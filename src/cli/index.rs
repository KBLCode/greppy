//! Index command implementation

use crate::cli::IndexArgs;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::project::Project;
use crate::index::{IndexWriter, TantivyIndex};
use crate::parse::Chunker;
use ignore::WalkBuilder;
use std::env;
use std::time::Instant;
use tracing::{debug, info, warn};

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
    let chunker = Chunker::new();

    // Walk the project directory
    let walker = WalkBuilder::new(&project.root)
        .hidden(true) // Respect hidden files
        .git_ignore(true) // Respect .gitignore
        .git_global(true)
        .git_exclude(true)
        .max_filesize(Some(config.index.max_file_size))
        .build();

    let mut file_count = 0;
    let mut chunk_count = 0;

    for entry in walker.flatten() {
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Skip non-code files
        if !is_code_file(path) {
            continue;
        }

        // Skip files matching global ignore patterns
        if should_ignore(path, &config.ignore.patterns) {
            debug!(path = %path.display(), "Skipping ignored file");
            continue;
        }

        // Read file content
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                debug!(path = %path.display(), error = %e, "Failed to read file");
                continue;
            }
        };

        // Chunk the file
        let chunks = match chunker.chunk_file(path, &content) {
            Ok(c) => c,
            Err(e) => {
                warn!(path = %path.display(), error = %e, "Failed to chunk file");
                continue;
            }
        };

        // Add chunks to index
        for chunk in &chunks {
            writer.add_chunk(chunk)?;
            chunk_count += 1;
        }

        file_count += 1;

        if file_count % 100 == 0 {
            debug!(files = file_count, chunks = chunk_count, "Indexing progress");
        }
    }

    // Commit the index
    writer.commit()?;

    let elapsed = start.elapsed();
    info!(
        files = file_count,
        chunks = chunk_count,
        elapsed_ms = elapsed.as_millis(),
        "Indexing complete"
    );

    println!(
        "Indexed {} files ({} chunks) in {:.2}s",
        file_count,
        chunk_count,
        elapsed.as_secs_f64()
    );

    // Watch mode
    if args.watch {
        println!("Watch mode not yet implemented. Use daemon mode for file watching.");
        // TODO: Implement watch mode
    }

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
