//! Search command implementation

use crate::ai::{claude::ClaudeClient, gemini::GeminiClient};
use crate::auth::{self, Provider};
use crate::cli::{OutputFormat, SearchArgs};
use crate::core::error::Result;
use crate::core::project::Project;
use crate::daemon::client;
use crate::index::TantivyIndex;
use crate::output::format_results;
use crate::search::SearchQuery;
use std::env;
use tracing::debug;

/// Run the search command
pub async fn run(args: SearchArgs) -> Result<()> {
    let project_path = args
        .project
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    let project = Project::detect(&project_path)?;
    let format = if args.json {
        OutputFormat::Json
    } else {
        OutputFormat::Human
    };

    // Direct mode: BM25 only
    if args.direct {
        return run_direct_search(&args, &project, format).await;
    }

    // Semantic mode: check OAuth, search, then AI
    run_semantic_search(&args, &project, format).await
}

/// Direct BM25 search (no AI)
async fn run_direct_search(
    args: &SearchArgs,
    project: &Project,
    format: OutputFormat,
) -> Result<()> {
    // Try daemon first
    if let Ok(true) = client::is_running() {
        debug!("Using daemon for direct search");
        if let Ok(results) = client::search(&args.query, &project.root, args.limit).await {
            print!("{}", format_results(&results, format));
            return Ok(());
        }
        debug!("Daemon search failed, falling back to direct");
    }

    // Direct mode (blocking, but fine for CLI)
    let index = TantivyIndex::open(&project.root)?;
    let query = SearchQuery::new(&args.query).with_limit(args.limit);
    let results = query.execute(&index)?;
    print!("{}", format_results(&results, format));

    Ok(())
}

/// Semantic search (BM25 + AI reranking)
async fn run_semantic_search(
    args: &SearchArgs,
    project: &Project,
    format: OutputFormat,
) -> Result<()> {
    // Check which provider is authenticated
    let providers = auth::get_authenticated_providers();

    if providers.is_empty() {
        eprintln!("Not logged in. Run 'greppy login' to enable semantic search.");
        eprintln!("Using direct BM25 search instead.\n");
        return run_direct_search(args, project, format).await;
    }

    // Get BM25 results first (fetch more than needed for reranking)
    let fetch_limit = (args.limit * 2).min(20); // Fetch 2x for better reranking, max 20
    let mut results = if let Ok(true) = client::is_running() {
        debug!("Using daemon for search");
        client::search(&args.query, &project.root, fetch_limit).await?
    } else {
        let index = TantivyIndex::open(&project.root)?;
        let query = SearchQuery::new(&args.query).with_limit(fetch_limit);
        query.execute(&index)?
    };

    // If no results, nothing to rerank
    if results.results.is_empty() {
        println!("No results found for: {}", args.query);
        return Ok(());
    }

    // Build chunks for reranking
    let chunks: Vec<String> = results
        .results
        .iter()
        .map(|r| {
            format!(
                "// {}\n{}",
                r.path,
                r.content.chars().take(1500).collect::<String>()
            )
        })
        .collect();

    // Call AI to rerank
    let indices = if providers.contains(&Provider::Anthropic) {
        let token = auth::get_anthropic_token()?;
        let client = ClaudeClient::new(token);
        client.rerank(&args.query, &chunks).await?
    } else {
        let token = auth::get_google_token()?;
        let client = GeminiClient::new(token);
        client.rerank(&args.query, &chunks).await?
    };

    // Reorder results based on AI ranking
    let original_results = std::mem::take(&mut results.results);
    for &idx in indices.iter().take(args.limit) {
        if idx < original_results.len() {
            results.results.push(original_results[idx].clone());
        }
    }

    // If AI returned fewer indices than requested, fill with remaining
    if results.results.len() < args.limit {
        for (i, result) in original_results.into_iter().enumerate() {
            if !indices.contains(&i) && results.results.len() < args.limit {
                results.results.push(result);
            }
        }
    }

    // Output same format as direct search
    print!("{}", format_results(&results, format));

    Ok(())
}
