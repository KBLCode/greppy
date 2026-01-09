use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A single search result - optimized with Arc for zero-copy cloning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Relative path to the file (Arc for zero-copy cloning)
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    pub path: Arc<str>,
    /// The matched content (Arc for zero-copy cloning)
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    pub content: Arc<str>,
    /// Symbol name if this chunk contains a symbol definition
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        serialize_with = "serialize_option_arc_str",
        deserialize_with = "deserialize_option_arc_str"
    )]
    pub symbol_name: Option<Arc<str>>,
    /// Symbol type (function, class, struct, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        serialize_with = "serialize_option_arc_str",
        deserialize_with = "deserialize_option_arc_str"
    )]
    pub symbol_type: Option<Arc<str>>,
    /// Start line number (1-indexed)
    pub start_line: usize,
    /// End line number (1-indexed)
    pub end_line: usize,
    /// Programming language
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    pub language: Arc<str>,
    /// BM25 relevance score
    pub score: f32,
    // New AST-aware fields
    /// Function/method signature
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        serialize_with = "serialize_option_arc_str",
        deserialize_with = "deserialize_option_arc_str"
    )]
    pub signature: Option<Arc<str>>,
    /// Parent symbol (for methods in classes)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        serialize_with = "serialize_option_arc_str",
        deserialize_with = "deserialize_option_arc_str"
    )]
    pub parent_symbol: Option<Arc<str>>,
    /// Documentation comment
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(
        serialize_with = "serialize_option_arc_str",
        deserialize_with = "deserialize_option_arc_str"
    )]
    pub doc_comment: Option<Arc<str>>,
    /// Whether this symbol is exported/public
    pub is_exported: bool,
    /// Whether this is a test function
    pub is_test: bool,
}

// Serde helpers for Arc<str>
fn serialize_arc_str<S>(arc: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(arc)
}

fn deserialize_arc_str<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(Arc::from(s.as_str()))
}

fn serialize_option_arc_str<S>(opt: &Option<Arc<str>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match opt {
        Some(arc) => serializer.serialize_some(arc.as_ref()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_option_arc_str<'de, D>(deserializer: D) -> Result<Option<Arc<str>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.map(|s| Arc::from(s.as_str())))
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
    pub fn new(
        query: String,
        project: String,
        results: Vec<SearchResult>,
        elapsed_ms: f64,
        cached: bool,
    ) -> Self {
        Self {
            query,
            project,
            results,
            elapsed_ms,
            cached,
        }
    }
}
