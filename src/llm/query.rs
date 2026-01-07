//! Query enhancement using Claude Haiku
//!
//! Analyzes natural language queries and expands them for better BM25 matching.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::client::ClaudeClient;

/// System prompt for query enhancement
const SYSTEM_PROMPT: &str = r#"You are a code search query optimizer. Given a natural language query about code, extract the intent and expand it into search terms that will match code effectively.

Respond with JSON only, no markdown formatting:
{
  "intent": "find_definition|find_usage|find_implementation|understand_flow|find_error_handling|general",
  "entity_type": "function|class|method|variable|type|module|interface|trait|struct|enum|null",
  "expanded_query": "space-separated search terms including synonyms and related terms",
  "filters": {
    "symbol_types": ["function", "class"] or null,
    "exclude_tests": true or false,
    "file_patterns": ["*.rs", "*.ts"] or null
  }
}

Examples:
- "how does authentication work" -> {"intent":"understand_flow","entity_type":null,"expanded_query":"auth authenticate login session token verify credentials user password jwt oauth","filters":{"symbol_types":null,"exclude_tests":true,"file_patterns":null}}
- "find the User class" -> {"intent":"find_definition","entity_type":"class","expanded_query":"User class struct type definition","filters":{"symbol_types":["class","struct"],"exclude_tests":true,"file_patterns":null}}
- "error handling in api routes" -> {"intent":"find_error_handling","entity_type":"function","expanded_query":"error handle catch try except Result Err Error api route endpoint handler","filters":{"symbol_types":["function","method"],"exclude_tests":true,"file_patterns":null}}"#;

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
pub async fn enhance_query(query: &str) -> Result<QueryEnhancement> {
    let client = ClaudeClient::new();
    
    info!("Enhancing query with LLM: {}", query);
    
    let response = client
        .send_message(SYSTEM_PROMPT, query)
        .await
        .context("Failed to get LLM response")?;
    
    // Parse JSON response
    let enhancement: QueryEnhancement = serde_json::from_str(&response)
        .context("Failed to parse LLM response as JSON")?;
    
    debug!(
        "Query enhanced: intent={}, expanded='{}'",
        enhancement.intent,
        enhancement.expanded_query
    );
    
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
}
