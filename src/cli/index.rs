//! Index command implementation
//!
//! Memory-safe parallel indexing:
//! - Phase 1: Collect file paths (small memory footprint)
//! - Phase 2: Parallel read + chunk with rayon (bounded by thread pool)
//! - Phase 3: Sequential write to Tantivy with periodic commits
//!
//! This avoids holding all file contents or chunks in memory at once.

use crate::cli::IndexArgs;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::project::Project;
use crate::index::{IndexWriter, TantivyIndex};
use crate::parse::{chunk_file, Chunk};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tracing::{debug, info};

/// Batch size for commits - prevents unbounded memory growth in Tantivy
const COMMIT_BATCH_SIZE: usize = 5000;

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

    // =========================================================================
    // PHASE 1: Collect file paths (memory-efficient - just PathBufs)
    // =========================================================================
    let walker = WalkBuilder::new(&project.root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_filesize(Some(config.index.max_file_size))
        .build();

    let ignore_patterns = config.ignore.patterns.clone();

    let file_paths: Vec<PathBuf> = walker
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return None;
            }
            if !is_code_file(path) {
                return None;
            }
            if should_ignore(path, &ignore_patterns) {
                debug!(path = %path.display(), "Skipping ignored file");
                return None;
            }
            Some(path.to_path_buf())
        })
        .collect();

    let total_files = file_paths.len();
    info!(files = total_files, "Found files to index");

    // =========================================================================
    // PHASE 2: Parallel read + chunk (rayon handles thread pool bounds)
    // Memory safety: Each file is read, chunked, and dropped before next batch
    // =========================================================================
    let file_count = AtomicUsize::new(0);
    let chunk_count = AtomicUsize::new(0);

    // Process in batches to control memory - don't load all files at once
    let batch_size = 500; // Process 500 files at a time
    let mut writer = IndexWriter::new(&index)?;
    let mut total_chunks_written = 0usize;

    for batch in file_paths.chunks(batch_size) {
        // Parallel: read and chunk files in this batch
        let batch_chunks: Vec<Chunk> = batch
            .par_iter()
            .filter_map(|path| {
                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => {
                        debug!(path = %path.display(), error = %e, "Failed to read file");
                        return None;
                    }
                };

                file_count.fetch_add(1, Ordering::Relaxed);
                let chunks = chunk_file(path, &content);
                chunk_count.fetch_add(chunks.len(), Ordering::Relaxed);

                // Return chunks, content is dropped here (memory freed)
                Some(chunks)
            })
            .flatten()
            .collect();

        // Sequential: write to Tantivy (thread-safe requirement)
        for chunk in &batch_chunks {
            writer.add_chunk(chunk)?;
            total_chunks_written += 1;

            // Periodic commit to prevent unbounded Tantivy buffer growth
            if total_chunks_written % COMMIT_BATCH_SIZE == 0 {
                debug!(chunks = total_chunks_written, "Intermediate commit");
                writer = writer.commit_and_reopen(&index)?;
            }
        }
        // batch_chunks dropped here - memory freed before next batch
    }

    // Final commit
    writer.commit()?;

    let elapsed = start.elapsed();
    let final_file_count = file_count.load(Ordering::Relaxed);
    let final_chunk_count = chunk_count.load(Ordering::Relaxed);

    let chunks_per_sec = if elapsed.as_secs_f64() > 0.0 {
        final_chunk_count as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    info!(
        files = final_file_count,
        chunks = final_chunk_count,
        elapsed_ms = elapsed.as_millis(),
        chunks_per_sec = chunks_per_sec as u64,
        "Indexing complete"
    );

    println!(
        "Indexed {} files ({} chunks) in {:.2}s ({:.0} chunks/sec)",
        final_file_count,
        final_chunk_count,
        elapsed.as_secs_f64(),
        chunks_per_sec
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
