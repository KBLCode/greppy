//! Search command implementation

use crate::ai::embedding::Embedder;
use crate::cli::SearchArgs;
use crate::core::error::Result;
use crate::core::project::Project;
use crate::index::TantivyIndex;
use crate::output::format_results;
use crate::search::SearchQuery;
use std::env;
use tracing::info;

/// Run the search command
pub fn run(args: SearchArgs) -> Result<()> {
    // Determine project path
    let project_path = args
        .project
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    // Detect project
    let project = Project::detect(&project_path)?;
    info!(project = %project.name, root = %project.root.display(), "Detected project");

    // Open index
    let index = TantivyIndex::open(&project.root)?;

    // Build query
    let mut query = SearchQuery::new(&args.query)
        .with_limit(args.limit)
        .with_path_filters(args.paths)
        .with_tests(args.include_tests);

    // Generate embedding if possible (for hybrid search)
    // Note: This adds latency (model load + inference), so we might want to make it optional flag
    // or only do it if the query looks like natural language.
    // For now, let's try to do it always to demonstrate the capability,
    // but print a message so user knows why it's slow the first time.
    // Actually, loading the model every time is slow (~1-2s).
    // In a real CLI, we might want a daemon or a faster model load.
    // Let's try to load it.

    // Only attempt embedding if the query has spaces (likely natural language)
    if args.query.contains(' ') {
        // info!("Generating embedding for semantic search...");
        if let Ok(embedder) = Embedder::new() {
            if let Ok(embedding) = embedder.embed(&args.query) {
                query = query.with_embedding(embedding);
            }
        }
    }

    let results = query.execute(&index)?;

    // Format and print results
    let output = format_results(&results, args.format);
    print!("{}", output);

    Ok(())
}
