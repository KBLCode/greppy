//! Index command implementation

use crate::ai::embedding::Embedder;
use crate::cli::IndexArgs;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::project::Project;
use crate::index::{IndexWriter, TantivyIndex};
use crate::parse::{chunk_file, Chunk};
use crossbeam_channel::{bounded, Receiver, Sender};
use ignore::WalkBuilder;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tracing::{debug, info};

/// Run the index command
pub fn run(args: IndexArgs) -> Result<()> {
    // Determine project path
    let project_path = args
        .project
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    // Detect project
    let project = Project::detect(&project_path)?;
    info!(project = %project.name, root = %project.root.display(), "Indexing project");

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
    // Note: Embedder initialization can be slow, so we do it once.
    // We wrap it in Arc for sharing.
    println!("Initializing embedding model...");
    let embedder = Arc::new(Embedder::new()?);
    println!("Model initialized.");

    // Channels for pipeline
    // Walker -> [Path] -> Workers -> [(Chunk, Embedding)] -> Writer
    let (path_tx, path_rx): (Sender<PathBuf>, Receiver<PathBuf>) = bounded(1000);
    let (doc_tx, doc_rx): (Sender<(Chunk, Vec<f32>)>, Receiver<(Chunk, Vec<f32>)>) = bounded(1000);

    // Spawn Walker Thread
    let walker_root = project.root.clone();
    let walker_config = config.clone();
    let walker_tx = path_tx.clone(); // Clone for the walker thread
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
            if should_ignore(path, &walker_config.ignore.patterns) {
                continue;
            }
            let _ = walker_tx.send(path.to_path_buf());
        }
    });

    // Spawn Worker Threads (Embedding Generation)
    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    for _ in 0..num_workers {
        let p_rx = path_rx.clone();
        let d_tx = doc_tx.clone();
        let emb = Arc::clone(&embedder);

        thread::spawn(move || {
            for path in p_rx {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let chunks = chunk_file(&path, &content);
                for chunk in chunks {
                    // Generate embedding for chunk content
                    // We combine symbol name and content for better context
                    let text_to_embed = format!(
                        "{}: {}",
                        chunk.symbol_name.as_deref().unwrap_or("code"),
                        chunk.content
                    );

                    if let Ok(embedding) = emb.embed(&text_to_embed) {
                        let _ = d_tx.send((chunk, embedding));
                    }
                }
            }
        });
    }

    // Drop original senders to close channel when workers are done
    drop(path_tx);
    drop(doc_tx);

    // Main Thread: Write to Index
    let _file_count = 0; // Approximation
    let mut chunk_count = 0;

    for (chunk, embedding) in doc_rx {
        writer.add_chunk(&chunk, Some(&embedding))?;
        chunk_count += 1;

        if chunk_count % 100 == 0 {
            debug!(chunks = chunk_count, "Indexing progress");
        }
    }

    // Commit the index
    writer.commit()?;

    let elapsed = start.elapsed();
    info!(
        chunks = chunk_count,
        elapsed_ms = elapsed.as_millis(),
        "Indexing complete"
    );

    println!(
        "Indexed {} chunks in {:.2}s",
        chunk_count,
        elapsed.as_secs_f64()
    );

    Ok(())
}

/// Check if a file is a code file worth indexing
fn is_code_file(path: &std::path::Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(
        ext.as_str(),
        "ts" | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "py"
            | "pyi"
            | "rs"
            | "go"
            | "java"
            | "kt"
            | "kts"
            | "scala"
            | "rb"
            | "php"
            | "c"
            | "h"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "cs"
            | "swift"
            | "ex"
            | "exs"
            | "erl"
            | "hrl"
            | "hs"
            | "ml"
            | "mli"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "sql"
            | "vue"
            | "svelte"
    )
}

/// Check if a path matches any ignore pattern
fn should_ignore(path: &std::path::Path, patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in patterns {
        // Simple substring matching for now
        // TODO: Use proper glob matching
        if path_str.contains(pattern.trim_matches('*')) {
            return true;
        }
    }

    false
}
