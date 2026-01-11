//! JSON output formatting

use crate::search::SearchResults;

/// Format results as JSON
pub fn format(results: &SearchResults) -> String {
    serde_json::to_string_pretty(results).unwrap_or_else(|e| {
        format!(r#"{{"error": "Failed to serialize results: {}"}}"#, e)
    })
}
