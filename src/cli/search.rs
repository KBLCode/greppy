//! Search command implementation

use crate::ai::embedding::get_global_embedder;
use crate::cli::SearchArgs;
use crate::core::error::{Error, Result};
use crate::core::project::Project;
use crate::daemon::client;
use crate::index::TantivyIndex;
use crate::output::format_results;
use crate::search::SearchQuery;
use std::env;
use tracing::{debug, info, warn};

/// Run the search command
pub async fn run(args: SearchArgs) -> Result<()> {
    // Determine project path
    let project_path = match args.project.clone() {
        Some(p) => p,
        None => env::current_dir().map_err(|e| Error::IoError {
            message: format!("Failed to get current directory: {}", e),
        })?,
    };

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
                    warn!("Daemon search failed: {}. Falling back to local search.", e);
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
        // Use global embedder to avoid re-initialization on every search
        if let Some(embedder) = get_global_embedder() {
            match embedder.embed(&args.query) {
                Ok(embedding) => {
                    debug!("Generated embedding for semantic search");
                    query = query.with_embedding(embedding);
                }
                Err(e) => {
                    warn!(error = %e, "Failed to generate embedding, using keyword search only");
                }
            }
        } else {
            debug!("Embedder not available, using keyword search only");
        }
    }

    let results = query.execute(&index)?;

    // Format and print results
    let output = format_results(&results, args.format);
    print!("{}", output);

    Ok(())
}
