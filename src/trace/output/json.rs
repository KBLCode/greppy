//! JSON output formatter
//!
//! Provides machine-readable JSON output for:
//! - Tooling integration
//! - Editor plugins
//! - Scripting and automation
//!
//! @module trace/output/json

use super::{
    DeadCodeResult, FlowResult, ImpactResult, ModuleResult, PatternResult, RefsResult, ScopeResult,
    StatsResult, TraceFormatter, TraceResult,
};

// =============================================================================
// FORMATTER IMPLEMENTATION
// =============================================================================

/// JSON formatter for machine-readable output
pub struct JsonFormatter {
    pretty: bool,
}

impl JsonFormatter {
    /// Create a new JSON formatter with pretty printing
    pub fn new() -> Self {
        Self { pretty: true }
    }

    /// Create a compact JSON formatter (no pretty printing)
    pub fn compact() -> Self {
        Self { pretty: false }
    }

    /// Serialize to JSON string
    fn to_json<T: serde::Serialize>(&self, value: &T) -> String {
        if self.pretty {
            serde_json::to_string_pretty(value)
                .unwrap_or_else(|e| format!(r#"{{"error": "JSON serialization failed: {}"}}"#, e))
        } else {
            serde_json::to_string(value)
                .unwrap_or_else(|e| format!(r#"{{"error": "JSON serialization failed: {}"}}"#, e))
        }
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceFormatter for JsonFormatter {
    fn format_trace(&self, result: &TraceResult) -> String {
        self.to_json(result)
    }

    fn format_refs(&self, result: &RefsResult) -> String {
        self.to_json(result)
    }

    fn format_dead_code(&self, result: &DeadCodeResult) -> String {
        self.to_json(result)
    }

    fn format_flow(&self, result: &FlowResult) -> String {
        self.to_json(result)
    }

    fn format_impact(&self, result: &ImpactResult) -> String {
        self.to_json(result)
    }

    fn format_module(&self, result: &ModuleResult) -> String {
        self.to_json(result)
    }

    fn format_pattern(&self, result: &PatternResult) -> String {
        self.to_json(result)
    }

    fn format_scope(&self, result: &ScopeResult) -> String {
        self.to_json(result)
    }

    fn format_stats(&self, result: &StatsResult) -> String {
        self.to_json(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::output::{ChainStep, InvocationPath, ReferenceInfo, ReferenceKind};

    #[test]
    fn test_format_trace_json() {
        let formatter = JsonFormatter::new();
        let result = TraceResult {
            symbol: "validateUser".to_string(),
            defined_at: Some("utils/validation.ts:8".to_string()),
            kind: "function".to_string(),
            invocation_paths: vec![InvocationPath {
                entry_point: "POST /api/auth/login".to_string(),
                entry_kind: "route".to_string(),
                chain: vec![
                    ChainStep {
                        symbol: "loginController.handle".to_string(),
                        file: "auth.controller.ts".to_string(),
                        line: 8,
                        column: Some(5),
                        context: None,
                    },
                    ChainStep {
                        symbol: "authService.login".to_string(),
                        file: "auth.service.ts".to_string(),
                        line: 42,
                        column: Some(10),
                        context: None,
                    },
                    ChainStep {
                        symbol: "validateUser".to_string(),
                        file: "validation.ts".to_string(),
                        line: 8,
                        column: Some(3),
                        context: None,
                    },
                ],
            }],
            total_paths: 47,
            entry_points: 12,
        };

        let output = formatter.format_trace(&result);

        // Verify it's valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("Should be valid JSON");

        assert_eq!(parsed["symbol"], "validateUser");
        assert_eq!(parsed["defined_at"], "utils/validation.ts:8");
        assert_eq!(parsed["total_paths"], 47);
        assert_eq!(parsed["entry_points"], 12);
        assert_eq!(
            parsed["invocation_paths"][0]["entry_point"],
            "POST /api/auth/login"
        );
        assert_eq!(
            parsed["invocation_paths"][0]["chain"][0]["symbol"],
            "loginController.handle"
        );
    }

    #[test]
    fn test_format_refs_json() {
        let formatter = JsonFormatter::new();
        let mut by_kind = std::collections::HashMap::new();
        by_kind.insert("read".to_string(), 5);
        by_kind.insert("write".to_string(), 2);

        let result = RefsResult {
            symbol: "userId".to_string(),
            defined_at: Some("types.ts:5".to_string()),
            symbol_kind: Some("variable".to_string()),
            references: vec![ReferenceInfo {
                file: "handler.ts".to_string(),
                line: 10,
                column: 15,
                kind: ReferenceKind::Read,
                context: "const id = userId;".to_string(),
                enclosing_symbol: Some("handleRequest".to_string()),
            }],
            total_refs: 7,
            by_kind,
            by_file: std::collections::HashMap::new(),
        };

        let output = formatter.format_refs(&result);

        // Verify it's valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("Should be valid JSON");

        assert_eq!(parsed["symbol"], "userId");
        assert_eq!(parsed["total_refs"], 7);
        assert_eq!(parsed["references"][0]["kind"], "read");
        assert_eq!(parsed["references"][0]["line"], 10);
    }

    #[test]
    fn test_compact_json() {
        let formatter = JsonFormatter::compact();
        let result = TraceResult {
            symbol: "test".to_string(),
            defined_at: None,
            kind: "function".to_string(),
            invocation_paths: vec![],
            total_paths: 0,
            entry_points: 0,
        };

        let output = formatter.format_trace(&result);

        // Compact JSON should not contain newlines (except in strings)
        assert!(!output.contains("\n  "));
    }

    #[test]
    fn test_json_special_characters() {
        let formatter = JsonFormatter::new();
        let result = TraceResult {
            symbol: "test\"with\\quotes".to_string(),
            defined_at: Some("path/with spaces/file.ts:1".to_string()),
            kind: "function".to_string(),
            invocation_paths: vec![],
            total_paths: 0,
            entry_points: 0,
        };

        let output = formatter.format_trace(&result);

        // Should be valid JSON even with special characters
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("Should be valid JSON");
        assert!(parsed["symbol"].as_str().unwrap().contains("quotes"));
    }
}
