//! Query enhancement using Claude Haiku
//!
//! Analyzes natural language queries and expands them for better BM25 matching.
//! Includes caching to avoid repeated API calls for similar queries.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::cache::LlmCache;
use super::client::ClaudeClient;

/// System prompt for query enhancement
pub const SYSTEM_PROMPT: &str = r#"Expand code search query into related terms. JSON only:
{"intent":"general","entity_type":null,"expanded_query":"term1 term2 term3","filters":{"symbol_types":null,"exclude_tests":true,"file_patterns":null}}

intent: find_definition|find_usage|understand_flow|find_error_handling|general
entity_type: function|class|method|struct|null
expanded_query: 8-15 space-separated code terms, synonyms, related concepts"#;

/// Enhanced query result from LLM analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEnhancement {
    /// Detected intent of the query
    pub intent: String,
    
    /// Type of code entity being searched for
    pub entity_type: Option<String>,
    
    /// Expanded query with synonyms and related terms
    pub expanded_query: String,
    
    /// Suggested filters for the search
    pub filters: QueryFilters,
}

/// Filters suggested by the LLM
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryFilters {
    /// Symbol types to filter by
    #[serde(default)]
    pub symbol_types: Option<Vec<String>>,
    
    /// Whether to exclude test files
    #[serde(default)]
    pub exclude_tests: bool,
    
    /// File patterns to filter by
    #[serde(default)]
    pub file_patterns: Option<Vec<String>>,
}

/// Enhance a search query using Claude Haiku
///
/// Returns the enhanced query or falls back to the original on error.
/// Uses caching to avoid repeated API calls for similar queries.
pub async fn enhance_query(query: &str) -> Result<QueryEnhancement> {
    // Check cache first
    let cache = LlmCache::load();
    if let Some(cached) = cache.get(query) {
        info!("Using cached query enhancement");
        return Ok(cached);
    }

    let client = ClaudeClient::new();
    
    info!("Enhancing query with LLM: {}", query);
    
    let response = client
        .send_message(SYSTEM_PROMPT, query)
        .await
        .context("Failed to get LLM response")?;
    
    // Extract JSON from response (handle markdown code blocks)
    let json_str = extract_json(&response);
    
    // Parse JSON response
    let enhancement: QueryEnhancement = serde_json::from_str(json_str)
        .with_context(|| format!("Failed to parse LLM response as JSON: {}", json_str))?;
    
    debug!(
        "Query enhanced: intent={}, expanded='{}'",
        enhancement.intent,
        enhancement.expanded_query
    );
    
    // Cache the result
    cache.set(query, enhancement.clone());
    
    Ok(enhancement)
}

/// Try to enhance a query, falling back to original on any error
pub async fn try_enhance_query(query: &str) -> QueryEnhancement {
    match enhance_query(query).await {
        Ok(enhancement) => enhancement,
        Err(e) => {
            warn!("Query enhancement failed, using original: {}", e);
            QueryEnhancement {
                intent: "general".to_string(),
                entity_type: None,
                expanded_query: query.to_string(),
                filters: QueryFilters::default(),
            }
        }
    }
}

/// Extract JSON from LLM response, handling markdown code blocks
fn extract_json(response: &str) -> &str {
    let trimmed = response.trim();
    
    // Try to find JSON in markdown code block
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    
    // Try generic code block
    if let Some(start) = trimmed.find("```") {
        let after_backticks = start + 3;
        // Skip optional language identifier on same line
        let json_start = trimmed[after_backticks..]
            .find('\n')
            .map(|n| after_backticks + n + 1)
            .unwrap_or(after_backticks);
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    
    // Try to find raw JSON object
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return &trimmed[start..=end];
            }
        }
    }
    
    // Return as-is
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_filters_default() {
        let filters = QueryFilters::default();
        assert!(filters.symbol_types.is_none());
        assert!(!filters.exclude_tests);
        assert!(filters.file_patterns.is_none());
    }

    #[test]
    fn test_parse_enhancement() {
        let json = r#"{
            "intent": "find_definition",
            "entity_type": "function",
            "expanded_query": "auth authenticate login",
            "filters": {
                "symbol_types": ["function"],
                "exclude_tests": true,
                "file_patterns": null
            }
        }"#;
        
        let enhancement: QueryEnhancement = serde_json::from_str(json).unwrap();
        assert_eq!(enhancement.intent, "find_definition");
        assert_eq!(enhancement.entity_type, Some("function".to_string()));
        assert_eq!(enhancement.expanded_query, "auth authenticate login");
        assert!(enhancement.filters.exclude_tests);
    }

    #[test]
    fn test_extract_json_raw() {
        let input = r#"{"intent": "general"}"#;
        assert_eq!(extract_json(input), r#"{"intent": "general"}"#);
    }

    #[test]
    fn test_extract_json_markdown() {
        let input = "```json\n{\"intent\": \"general\"}\n```";
        assert_eq!(extract_json(input), r#"{"intent": "general"}"#);
    }

    #[test]
    fn test_extract_json_with_text() {
        let input = "Here is the JSON:\n{\"intent\": \"general\"}";
        assert_eq!(extract_json(input), r#"{"intent": "general"}"#);
    }
}
