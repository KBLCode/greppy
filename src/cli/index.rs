//! Index command implementation
//!
//! Memory-safe parallel indexing:
//! - Phase 1: Collect file paths (small memory footprint)
//! - Phase 2: Parallel read + chunk with rayon (bounded by thread pool)
//! - Phase 3: Sequential write to Tantivy with periodic commits
//! - Phase 4: Build semantic trace index (symbols, calls, references)
//!
//! This avoids holding all file contents or chunks in memory at once.

use crate::cli::IndexArgs;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::project::Project;
use crate::index::{IndexWriter, TantivyIndex};
use crate::parse::{chunk_file, Chunk};
use crate::trace::{
    build_and_save_index, detect_language, find_dead_symbols, is_treesitter_supported, load_index,
    snapshots::create_snapshot, trace_index_path, SemanticIndex,
};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
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

    let tantivy_elapsed = start.elapsed();
    let final_file_count = file_count.load(Ordering::Relaxed);
    let final_chunk_count = chunk_count.load(Ordering::Relaxed);

    let chunks_per_sec = if tantivy_elapsed.as_secs_f64() > 0.0 {
        final_chunk_count as f64 / tantivy_elapsed.as_secs_f64()
    } else {
        0.0
    };

    info!(
        files = final_file_count,
        chunks = final_chunk_count,
        elapsed_ms = tantivy_elapsed.as_millis(),
        chunks_per_sec = chunks_per_sec as u64,
        "Text index complete"
    );

    println!(
        "Text index: {} files ({} chunks) in {:.2}s",
        final_file_count,
        final_chunk_count,
        tantivy_elapsed.as_secs_f64(),
    );

    // =========================================================================
    // PHASE 4: Build semantic trace index
    // =========================================================================
    let trace_start = Instant::now();
    info!("Building semantic trace index...");

    // Collect files that support tree-sitter for semantic indexing
    // We need to re-read files for semantic extraction (different from chunking)
    let semantic_files: Vec<(PathBuf, String)> = file_paths
        .par_iter()
        .filter_map(|path| {
            let lang = detect_language(path);
            if !is_treesitter_supported(lang) {
                return None;
            }
            match std::fs::read_to_string(path) {
                Ok(content) => Some((path.clone(), content)),
                Err(_) => None,
            }
        })
        .collect();

    let semantic_file_count = semantic_files.len();

    if semantic_file_count > 0 {
        match build_and_save_index(&project.root, &semantic_files) {
            Ok(stats) => {
                let trace_elapsed = trace_start.elapsed();
                info!(
                    files = stats.files,
                    symbols = stats.symbols,
                    tokens = stats.tokens,
                    edges = stats.edges,
                    elapsed_ms = trace_elapsed.as_millis(),
                    "Trace index complete"
                );
                println!(
                    "Trace index: {} files ({} symbols, {} edges) in {:.2}s",
                    stats.files,
                    stats.symbols,
                    stats.edges,
                    trace_elapsed.as_secs_f64(),
                );
            }
            Err(e) => {
                tracing::warn!("Failed to build trace index: {}", e);
                println!("Warning: Trace index build failed: {}", e);
            }
        }
    } else {
        println!("Trace index: skipped (no supported languages)");
    }

    let total_elapsed = start.elapsed();
    println!(
        "\nTotal: {:.2}s ({:.0} chunks/sec)",
        total_elapsed.as_secs_f64(),
        chunks_per_sec
    );

    // =========================================================================
    // PHASE 5: Create automatic snapshot
    // =========================================================================
    // Only create snapshot if trace index was successfully built
    if semantic_file_count > 0 {
        let trace_path = trace_index_path(&project.root);
        if trace_path.exists() {
            match load_index(&trace_path) {
                Ok(index) => {
                    let dead_symbols = find_dead_symbols(&index);
                    let cycles_count = count_cycles(&index) as u32;

                    match create_snapshot(
                        &index,
                        &project.root,
                        &project.name,
                        &dead_symbols.iter().map(|s| s.id).collect(),
                        cycles_count,
                        None, // Auto-generated, no custom name
                    ) {
                        Ok(_) => {
                            debug!("Auto-created snapshot after indexing");
                        }
                        Err(e) => {
                            debug!("Failed to create snapshot: {}", e);
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to load trace index for snapshot: {}", e);
                }
            }
        }
    }

    Ok(())
}

/// Count cycles using DFS (simplified version)
fn count_cycles(index: &SemanticIndex) -> usize {
    let mut graph: HashMap<u16, HashSet<u16>> = HashMap::new();

    for edge in &index.edges {
        if let (Some(from_sym), Some(to_sym)) =
            (index.symbol(edge.from_symbol), index.symbol(edge.to_symbol))
        {
            if from_sym.file_id != to_sym.file_id {
                graph
                    .entry(from_sym.file_id)
                    .or_default()
                    .insert(to_sym.file_id);
            }
        }
    }

    let mut cycles = 0;
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for &node in graph.keys() {
        if !visited.contains(&node) {
            cycles += count_cycles_dfs(node, &graph, &mut visited, &mut rec_stack);
        }
    }

    cycles
}

fn count_cycles_dfs(
    node: u16,
    graph: &HashMap<u16, HashSet<u16>>,
    visited: &mut HashSet<u16>,
    rec_stack: &mut HashSet<u16>,
) -> usize {
    visited.insert(node);
    rec_stack.insert(node);

    let mut cycles = 0;

    if let Some(neighbors) = graph.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                cycles += count_cycles_dfs(neighbor, graph, visited, rec_stack);
            } else if rec_stack.contains(&neighbor) {
                cycles += 1;
            }
        }
    }

    rec_stack.remove(&node);
    cycles
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
