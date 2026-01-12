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
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;
use tracing::{info, warn};

/// Batch size for sending chunks through channel
const CHUNK_BATCH_SIZE: usize = 200;

/// Run the index command
pub async fn run(args: IndexArgs) -> Result<()> {
    let project_path = match args.project.clone() {
        Some(p) => p,
        None => env::current_dir().map_err(|e| Error::IoError {
            message: format!("Failed to get current directory: {}", e),
        })?,
    };

    let project = Project::detect(&project_path)?;
    info!(project = %project.name, root = %project.root.display(), "Indexing project");

    // If daemon is running, delegate to it
    if let Ok(true) = client::is_running() {
        println!("Delegating indexing to daemon...");
        match client::index(&project.root, args.force).await {
            Ok((file_count, chunk_count, elapsed_ms)) => {
                println!(
                    "Indexed {} chunks from {} files in {:.2}s",
                    chunk_count,
                    file_count,
                    elapsed_ms / 1000.0
                );
                return Ok(());
            }
            Err(e) => {
                warn!("Daemon indexing failed: {}. Falling back to local.", e);
            }
        }
    }

    let start = Instant::now();
    let config = Config::load()?;

    let index = if args.force {
        TantivyIndex::delete(&project.root)?;
        TantivyIndex::open_or_create(&project.root)?
    } else {
        TantivyIndex::open_or_create(&project.root)?
    };

    // ============ PHASE 1: Fast Keyword Index ============
    println!("Phase 1: Building keyword index...");

    let mut writer = IndexWriter::new(&index)?;

    // Parallel file count
    let file_count = Arc::new(AtomicUsize::new(0));
    {
        let fc = Arc::clone(&file_count);
        let root = project.root.clone();
        let cfg = config.clone();
        WalkBuilder::new(&root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .max_filesize(Some(cfg.index.max_file_size))
            .threads(8)
            .build_parallel()
            .run(|| {
                let counter = Arc::clone(&fc);
                Box::new(move |entry| {
                    if let Ok(e) = entry {
                        if e.path().is_file() && is_code_file(e.path()) {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    ignore::WalkState::Continue
                })
            });
    }
    let total_files = file_count.load(Ordering::Relaxed);
    println!("  Found {} code files", total_files);

    // High-throughput channels - send batches not individual chunks
    let (path_tx, path_rx): (Sender<PathBuf>, Receiver<PathBuf>) = bounded(5000);
    let (doc_tx, doc_rx): (Sender<Vec<Chunk>>, Receiver<Vec<Chunk>>) = bounded(2000);

    let files_processed = Arc::new(AtomicUsize::new(0));

    // Parallel walker
    let walker_root = project.root.clone();
    let walker_config = config.clone();
    let walker_tx = path_tx.clone();
    let walker_handle: JoinHandle<()> = thread::spawn(move || {
        WalkBuilder::new(&walker_root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .max_filesize(Some(walker_config.index.max_file_size))
            .threads(8)
            .build_parallel()
            .run(|| {
                let tx = walker_tx.clone();
                Box::new(move |entry| {
                    if let Ok(e) = entry {
                        let path = e.path();
                        if path.is_file() && is_code_file(path) {
                            let _ = tx.send(path.to_path_buf());
                        }
                    }
                    ignore::WalkState::Continue
                })
            });
    });

    // Parser threads - more workers, batch output
    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .max(8);

    let mut worker_handles: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers);

    for _ in 0..num_workers {
        let p_rx = path_rx.clone();
        let d_tx = doc_tx.clone();
        let files_done = Arc::clone(&files_processed);

        let handle = thread::spawn(move || {
            let mut batch = Vec::with_capacity(CHUNK_BATCH_SIZE);

            for path in p_rx {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => {
                        files_done.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                let mut parser = get_parser(&path);
                let chunks = match parser.chunk(&path.to_string_lossy(), &content) {
                    Ok(c) => c,
                    Err(_) => {
                        files_done.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                batch.extend(chunks);

                if batch.len() >= CHUNK_BATCH_SIZE {
                    if d_tx.send(std::mem::take(&mut batch)).is_err() {
                        return;
                    }
                    batch = Vec::with_capacity(CHUNK_BATCH_SIZE);
                }

                files_done.fetch_add(1, Ordering::Relaxed);
            }

            // Flush remaining
            if !batch.is_empty() {
                let _ = d_tx.send(batch);
            }
        });
        worker_handles.push(handle);
    }

    drop(path_tx);
    drop(doc_tx);

    // Progress bar
    let pb = ProgressBar::new(total_files as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({per_sec})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut chunk_count = 0;
    let mut last_files = 0;

    // Receive batches
    for batch in doc_rx {
        for chunk in batch {
            writer.add_chunk(&chunk, None)?;
            chunk_count += 1;
        }

        let current_files = files_processed.load(Ordering::Relaxed);
        if current_files > last_files {
            pb.set_position(current_files as u64);
            last_files = current_files;
        }
    }

    pb.finish_and_clear();

    let _ = walker_handle.join();
    for handle in worker_handles {
        let _ = handle.join();
    }

    writer.commit()?;

    let phase1_elapsed = start.elapsed();
    let rate = chunk_count as f64 / phase1_elapsed.as_secs_f64();

    println!(
        "  Indexed {} chunks in {:.2}s ({:.0} chunks/sec)",
        chunk_count,
        phase1_elapsed.as_secs_f64(),
        rate
    );
    println!("  ✓ Keyword search ready!");

    // ============ PHASE 2: Background Embeddings ============
    if args.fast {
        println!("\nFast mode: skipping embeddings.");
        return Ok(());
    }

    println!("\nPhase 2: Generating embeddings (background)...");
    println!("  You can search now. Semantic search improves as embeddings complete.");

    let project_root = project.root.clone();
    let config_clone = config.clone();

    thread::spawn(move || {
        if let Err(e) = generate_embeddings_background(&project_root, &config_clone, chunk_count) {
            eprintln!("Background embedding failed: {}", e);
        }
    });

    Ok(())
}

/// Generate embeddings in background
fn generate_embeddings_background(
    project_root: &Path,
    config: &Config,
    total_chunks: usize,
) -> Result<()> {
    let start = Instant::now();

    println!("  Loading embedding model...");
    let embedder = match Embedder::new() {
        Ok(e) => Arc::new(e),
        Err(e) => {
            eprintln!("  Failed to load embedding model: {}", e);
            return Ok(());
        }
    };
    println!("  Model loaded.");

    let index = TantivyIndex::open_or_create(project_root)?;

    let (path_tx, path_rx): (Sender<PathBuf>, Receiver<PathBuf>) = bounded(1000);
    type ChunkEmbed = (Chunk, Vec<f32>);
    let (doc_tx, doc_rx): (Sender<Vec<ChunkEmbed>>, Receiver<Vec<ChunkEmbed>>) = bounded(500);

    // Walker
    let walker_root = project_root.to_path_buf();
    let walker_config = config.clone();
    thread::spawn(move || {
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
            let _ = path_tx.send(path.to_path_buf());
        }
    });

    // Embedding workers
    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(2);

    for _ in 0..num_workers {
        let p_rx = path_rx.clone();
        let d_tx = doc_tx.clone();
        let emb = Arc::clone(&embedder);

        thread::spawn(move || {
            const BATCH_SIZE: usize = 128;

            for path in p_rx {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let mut parser = get_parser(&path);
                let chunks = match parser.chunk(&path.to_string_lossy(), &content) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let mut batch_chunks = Vec::with_capacity(BATCH_SIZE);
                let mut batch_texts = Vec::with_capacity(BATCH_SIZE);

                for chunk in chunks {
                    let text = format!(
                        "{}: {}",
                        chunk.symbol_name.as_deref().unwrap_or("code"),
                        chunk.content
                    );
                    batch_chunks.push(chunk);
                    batch_texts.push(text);

                    if batch_chunks.len() >= BATCH_SIZE {
                        if let Ok(embeddings) = emb.embed_batch(std::mem::take(&mut batch_texts)) {
                            let batch: Vec<ChunkEmbed> =
                                batch_chunks.drain(..).zip(embeddings).collect();
                            let _ = d_tx.send(batch);
                        }
                    }
                }

                if !batch_chunks.is_empty() {
                    if let Ok(embeddings) = emb.embed_batch(batch_texts) {
                        let batch: Vec<ChunkEmbed> =
                            batch_chunks.into_iter().zip(embeddings).collect();
                        let _ = d_tx.send(batch);
                    }
                }
            }
        });
    }

    drop(path_rx);
    drop(doc_tx);

    let mut writer = IndexWriter::new(&index)?;
    let mut count = 0;
    let mut last_report = Instant::now();

    for batch in doc_rx {
        for (chunk, embedding) in batch {
            writer.add_chunk(&chunk, Some(&embedding))?;
            count += 1;
        }

        if last_report.elapsed().as_secs() >= 5 {
            let pct = (count as f64 / total_chunks as f64 * 100.0).min(100.0);
            let rate = count as f64 / start.elapsed().as_secs_f64();
            eprintln!(
                "  Embeddings: {:.0}% ({} chunks, {:.0}/sec)",
                pct, count, rate
            );
            last_report = Instant::now();
        }
    }

    writer.commit()?;

    let elapsed = start.elapsed();
    let rate = count as f64 / elapsed.as_secs_f64();
    eprintln!(
        "  ✓ Embeddings complete: {} chunks in {:.1}s ({:.0}/sec)",
        count,
        elapsed.as_secs_f64(),
        rate
    );

    Ok(())
}
