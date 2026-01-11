//! Search command implementation

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

    // Build and execute query
    let query = SearchQuery::new(&args.query)
        .with_limit(args.limit)
        .with_path_filters(args.paths)
        .with_tests(args.include_tests);

    let results = query.execute(&index)?;

    // Format and print results
    let output = format_results(&results, args.format);
    print!("{}", output);

    Ok(())
}
