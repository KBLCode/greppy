use crate::ai::embedding::Embedder;
use crate::cli::IndexArgs;
use crate::core::config::Config;
use crate::core::error::{Error, Result};
use crate::core::project::Project;
use crate::daemon::client;
use crate::index::{IndexWriter, TantivyIndex};
use crate::parse::{get_parser, is_code_file, Chunk};
use crossbeam_channel::{bounded, Receiver, Sender};
use ignore::WalkBuilder;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Run the index command
pub async fn run(args: IndexArgs) -> Result<()> {
    // Determine project path
    let project_path = match args.project.clone() {
        Some(p) => p,
        None => env::current_dir().map_err(|e| Error::IoError {
            message: format!("Failed to get current directory: {}", e),
        })?,
    };

    // Detect project
    let project = Project::detect(&project_path)?;
    info!(project = %project.name, root = %project.root.display(), "Indexing project");

    // Try daemon first
    if let Ok(true) = client::is_running() {
        println!("Delegating indexing to daemon...");
        println!("This may take a while for large projects. Please wait...");
        match client::index(&project.root, args.force).await {
            Ok((file_count, chunk_count, elapsed_ms)) => {
                println!(
                    "Indexed {} chunks from {} files in {:.2}s (via daemon)",
                    chunk_count,
                    file_count,
                    elapsed_ms / 1000.0
                );
                return Ok(());
            }
            Err(e) => {
                warn!(
                    "Daemon indexing failed: {}. Falling back to local indexing.",
                    e
                );
            }
        }
    }

    let start = Instant::now();

    // Load config for ignore patterns
    let config = Config::load()?;

    // Create or open index
    let index = if args.force {
        TantivyIndex::delete(&project.root)?;
        TantivyIndex::open_or_create(&project.root)?
    } else {
        TantivyIndex::open_or_create(&project.root)?
    };

    let mut writer = IndexWriter::new(&index)?;

    // Initialize Embedder (shared across threads)
    println!("Initializing embedding model...");
    let embedder = Arc::new(Embedder::new()?);
    println!("Model initialized.");

    // Channels for pipeline
    type ChunkWithEmbedding = (Chunk, Option<Vec<f32>>);
    let (path_tx, path_rx): (Sender<PathBuf>, Receiver<PathBuf>) = bounded(1000);
    let (doc_tx, doc_rx): (Sender<ChunkWithEmbedding>, Receiver<ChunkWithEmbedding>) =
        bounded(1000);

    // Track embedding failures
    let embedding_failure_count = Arc::new(AtomicUsize::new(0));

    // Spawn Walker Thread and store handle
    let walker_root = project.root.clone();
    let walker_config = config.clone();
    let walker_tx = path_tx.clone();
    let walker_handle: JoinHandle<()> = thread::spawn(move || {
        let walker = WalkBuilder::new(&walker_root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .max_filesize(Some(walker_config.index.max_file_size))
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_dir() || !is_code_file(path) {
                continue;
            }
            if walker_tx.send(path.to_path_buf()).is_err() {
                // Channel closed, stop walking
                break;
            }
        }
    });

    // Spawn Worker Threads and store handles
    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut worker_handles: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers);

    for worker_id in 0..num_workers {
        let p_rx = path_rx.clone();
        let d_tx = doc_tx.clone();
        let emb = Arc::clone(&embedder);
        let failure_count = Arc::clone(&embedding_failure_count);

        let handle = thread::spawn(move || {
            for path in p_rx {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        debug!(path = %path.display(), error = %e, "Failed to read file");
                        continue;
                    }
                };

                // Use the factory to get the correct parser
                let mut parser = get_parser(&path);
                let chunks = match parser.chunk(&path.to_string_lossy(), &content) {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "Failed to chunk file");
                        continue;
                    }
                };

                let mut batch_chunks = Vec::with_capacity(64);
                let mut batch_texts = Vec::with_capacity(64);

                for chunk in chunks {
                    let text_to_embed = format!(
                        "{}: {}",
                        chunk.symbol_name.as_deref().unwrap_or("code"),
                        chunk.content
                    );

                    batch_chunks.push(chunk);
                    batch_texts.push(text_to_embed);

                    if batch_chunks.len() >= 64 {
                        // Take ownership instead of cloning
                        let texts_to_process = std::mem::take(&mut batch_texts);
                        match emb.embed_batch(texts_to_process) {
                            Ok(embeddings) => {
                                for (c, e) in batch_chunks.drain(..).zip(embeddings) {
                                    if d_tx.send((c, Some(e))).is_err() {
                                        return; // Channel closed
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    worker_id,
                                    error = %e,
                                    batch_size = batch_chunks.len(),
                                    "Embedding batch failed, indexing without embeddings"
                                );
                                failure_count.fetch_add(batch_chunks.len(), Ordering::Relaxed);
                                for c in batch_chunks.drain(..) {
                                    if d_tx.send((c, None)).is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }

                // Flush remaining
                if !batch_chunks.is_empty() {
                    let texts_to_process = std::mem::take(&mut batch_texts);
                    match emb.embed_batch(texts_to_process) {
                        Ok(embeddings) => {
                            for (c, e) in batch_chunks.into_iter().zip(embeddings) {
                                if d_tx.send((c, Some(e))).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                worker_id,
                                error = %e,
                                batch_size = batch_chunks.len(),
                                "Embedding batch failed, indexing without embeddings"
                            );
                            failure_count.fetch_add(batch_chunks.len(), Ordering::Relaxed);
                            for c in batch_chunks {
                                if d_tx.send((c, None)).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        });
        worker_handles.push(handle);
    }

    // Drop original senders
    drop(path_tx);
    drop(doc_tx);

    // Main Thread: Write to Index with progress reporting
    let mut chunk_count = 0;
    let mut last_report = Instant::now();
    let report_interval = std::time::Duration::from_secs(2);

    println!("Indexing...");

    for (chunk, embedding) in doc_rx {
        writer.add_chunk(&chunk, embedding.as_deref())?;
        chunk_count += 1;

        // Report progress every 2 seconds
        if last_report.elapsed() >= report_interval {
            let elapsed = start.elapsed().as_secs_f64();
            let rate = chunk_count as f64 / elapsed;
            print!(
                "\r  {} chunks indexed ({:.0} chunks/sec)...",
                chunk_count, rate
            );
            std::io::Write::flush(&mut std::io::stdout()).ok();
            last_report = Instant::now();
        }
    }

    // Clear the progress line
    print!("\r                                                    \r");
    std::io::Write::flush(&mut std::io::stdout()).ok();

    // Wait for all threads to complete
    let _ = walker_handle.join();
    for handle in worker_handles {
        let _ = handle.join();
    }

    let failures = embedding_failure_count.load(Ordering::Relaxed);

    // Commit the index
    writer.commit()?;

    let elapsed = start.elapsed();
    info!(
        chunks = chunk_count,
        elapsed_ms = elapsed.as_millis(),
        "Indexing complete"
    );

    // Final summary
    let rate = chunk_count as f64 / elapsed.as_secs_f64();
    println!(
        "Indexed {} chunks in {:.2}s ({:.0} chunks/sec)",
        chunk_count,
        elapsed.as_secs_f64(),
        rate
    );

    if failures > 0 {
        println!(
            "  Warning: {} chunks indexed without embeddings due to errors",
            failures
        );
    }

    Ok(())
}
