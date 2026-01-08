//! LLM cache pre-warming during indexing
//!
//! Generates common query patterns from indexed symbols and pre-caches
//! LLM responses so searches are instant. Uses parallel requests for speed.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use futures::future::join_all;

use crate::index::IndexSearcher;
use super::cache::LlmCache;
use super::client::ClaudeClient;
use super::query::QueryEnhancement;

/// Common query templates to pre-warm
const QUERY_TEMPLATES: &[&str] = &[
    "how does {} work",
    "find {}",
    "where is {} used",
];

/// Max concurrent LLM requests
const MAX_CONCURRENT: usize = 5;

/// Pre-warm the LLM cache with common queries based on indexed symbols
/// Runs in parallel for speed
pub async fn warmup_cache(project_path: &Path) -> Result<usize, crate::error::GreppyError> {
    info!("Pre-warming LLM cache for {:?}", project_path);
    
    let searcher = IndexSearcher::open(project_path)?;
    let cache = Arc::new(Mutex::new(LlmCache::load()));
    
    // Extract top symbols from index
    let top_symbols = extract_top_symbols(&searcher)?;
    info!("Found {} key symbols to pre-warm", top_symbols.len());
    
    // Generate all queries
    let mut queries: Vec<String> = Vec::new();
    {
        let cache_guard = cache.lock().await;
        for symbol in &top_symbols {
            for template in QUERY_TEMPLATES {
                let query = template.replace("{}", symbol);
                // Skip if already cached
                if cache_guard.get(&query).is_none() {
                    queries.push(query);
                }
            }
        }
    }
    
    if queries.is_empty() {
        info!("All queries already cached");
        return Ok(0);
    }
    
    info!("Warming {} queries in parallel...", queries.len());
    
    // Process in batches
    let warmed = Arc::new(Mutex::new(0usize));
    
    for chunk in queries.chunks(MAX_CONCURRENT) {
        let futures: Vec<_> = chunk.iter().map(|query| {
            let query = query.clone();
            let cache = Arc::clone(&cache);
            let warmed = Arc::clone(&warmed);
            
            async move {
                let client = ClaudeClient::new();
                let system_prompt = super::query::SYSTEM_PROMPT;
                
                match client.send_message(system_prompt, &query).await {
                    Ok(response) => {
                        if let Ok(enhancement) = serde_json::from_str::<QueryEnhancement>(&response) {
                            let cache_guard = cache.lock().await;
                            cache_guard.set(&query, enhancement);
                            let mut w = warmed.lock().await;
                            *w += 1;
                            debug!("Cached: {}", query);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to warm '{}': {}", query, e);
                    }
                }
            }
        }).collect();
        
        join_all(futures).await;
    }
    
    let total = *warmed.lock().await;
    info!("Pre-warmed {} queries", total);
    Ok(total)
}

/// Extract top symbols from index for pre-warming
fn extract_top_symbols(searcher: &IndexSearcher) -> Result<Vec<String>, crate::error::GreppyError> {
    let mut symbols: HashSet<String> = HashSet::new();
    
    // Search for common patterns to find key symbols
    let searches = ["", "main", "new", "get", "set", "create", "handle", "process"];
    
    for query in searches {
        if let Ok(results) = searcher.search(query, 50) {
            for result in results {
                if let Some(ref name) = result.symbol_name {
                    // Only include meaningful symbol names
                    if name.len() >= 4 && !is_common_word(name) {
                        symbols.insert(name.to_lowercase());
                    }
                }
            }
        }
    }
    
    // Limit to top 20 most important symbols
    let mut symbols: Vec<String> = symbols.into_iter().collect();
    symbols.truncate(20);
    
    Ok(symbols)
}

/// Check if a word is too common to be useful
fn is_common_word(word: &str) -> bool {
    matches!(word.to_lowercase().as_str(), 
        "self" | "this" | "that" | "from" | "into" | "with" | "test" | "tests" |
        "main" | "new" | "get" | "set" | "run" | "init" | "data" | "value" |
        "result" | "error" | "option" | "some" | "none" | "true" | "false"
    )
}


