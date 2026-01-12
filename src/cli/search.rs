//! Search command implementation

use crate::ai::embedding::Embedder;
use crate::cli::SearchArgs;
use crate::core::error::Result;
use crate::core::project::Project;
use crate::daemon::client;
use crate::index::TantivyIndex;
use crate::output::format_results;
use crate::search::SearchQuery;
use std::env;
use tracing::info;

/// Run the search command
pub async fn run(args: SearchArgs) -> Result<()> {
    // Determine project path
    let project_path = args
        .project
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    // Detect project
    let project = Project::detect(&project_path)?;
    info!(project = %project.name, root = %project.root.display(), "Detected project");

    // Try daemon first if requested
    if args.use_daemon {
        if let Ok(true) = client::is_running() {
            match client::search(&args.query, &project.root, args.limit).await {
                Ok(results) => {
                    let output = format_results(&results, args.format);
                    print!("{}", output);
                    return Ok(());
                }
                Err(e) => {
                    // Fallback to local search if daemon fails
                    info!("Daemon search failed: {}. Falling back to local search.", e);
                }
            }
        }
    }

    // Open index
    let index = TantivyIndex::open(&project.root)?;

    // Build query
    let mut query = SearchQuery::new(&args.query)
        .with_limit(args.limit)
        .with_path_filters(args.paths)
        .with_tests(args.include_tests);

    // Generate embedding if possible (for hybrid search)
    // Only attempt embedding if the query has spaces (likely natural language)
    if args.query.contains(' ') {
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
