//! JSON output formatting

use crate::search::SearchResponse;

/// Format results as JSON
pub fn format(results: &SearchResponse) -> String {
    serde_json::to_string_pretty(results)
        .unwrap_or_else(|e| format!(r#"{{"error": "Failed to serialize results: {}"}}"#, e))
}
