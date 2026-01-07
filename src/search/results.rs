use serde::{Deserialize, Serialize};

/// A single search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Relative path to the file
    pub path: String,
    /// The matched content
    pub content: String,
    /// Symbol name if this chunk contains a symbol definition
    pub symbol_name: Option<String>,
    /// Symbol type (function, class, struct, etc.)
    pub symbol_type: Option<String>,
    /// Start line number (1-indexed)
    pub start_line: usize,
    /// End line number (1-indexed)
    pub end_line: usize,
    /// Programming language
    pub language: String,
    /// BM25 relevance score
    pub score: f32,
}

/// Response containing search results and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// The search query
    pub query: String,
    /// Project path that was searched
    pub project: String,
    /// Search results
    pub results: Vec<SearchResult>,
    /// Time taken in milliseconds
    pub elapsed_ms: f64,
    /// Whether results came from cache
    pub cached: bool,
}

impl SearchResponse {
    pub fn new(query: String, project: String, results: Vec<SearchResult>, elapsed_ms: f64, cached: bool) -> Self {
        Self {
            query,
            project,
            results,
            elapsed_ms,
            cached,
        }
    }
}
