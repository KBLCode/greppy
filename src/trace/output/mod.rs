//! Output formatters for trace results
//!
//! Provides multiple output formats for trace results:
//! - ASCII: Rich box-drawing with ANSI colors (terminal)
//! - Plain: Simple text without colors (piping/logs)
//! - JSON: Machine-readable format (tooling integration)
//! - CSV: Spreadsheet-compatible format
//! - DOT: Graph visualization format
//! - Markdown: Documentation format
//!
//! @module trace/output

pub mod ascii;
pub mod json;
pub mod plain;

// =============================================================================
// TYPES
// =============================================================================

/// Output format selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Rich ASCII with colors and box-drawing
    #[default]
    Ascii,
    /// Plain text without ANSI codes
    Plain,
    /// JSON for machine consumption
    Json,
    /// CSV for spreadsheets
    Csv,
    /// DOT for graph visualization
    Dot,
    /// Markdown for documentation
    Markdown,
}

/// A single step in an invocation chain
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChainStep {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// A complete invocation path from entry point to target
#[derive(Debug, Clone, serde::Serialize)]
pub struct InvocationPath {
    pub entry_point: String,
    pub entry_kind: String,
    pub chain: Vec<ChainStep>,
}

/// Reference to a symbol with context
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReferenceInfo {
    pub file: String,
    pub line: u32,
    pub column: u16,
    pub kind: ReferenceKind,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol: Option<String>,
}

/// Kind of reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReferenceKind {
    Read,
    Write,
    Call,
    TypeAnnotation,
    Import,
    Export,
}

impl std::fmt::Display for ReferenceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferenceKind::Read => write!(f, "read"),
            ReferenceKind::Write => write!(f, "write"),
            ReferenceKind::Call => write!(f, "call"),
            ReferenceKind::TypeAnnotation => write!(f, "type"),
            ReferenceKind::Import => write!(f, "import"),
            ReferenceKind::Export => write!(f, "export"),
        }
    }
}

/// Symbol information for dead code analysis
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeadSymbol {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    pub reason: String,
    /// Cross-reference: potential callers that could use this symbol
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub potential_callers: Vec<PotentialCaller>,
}

/// A potential caller/reference for dead code cross-referencing
#[derive(Debug, Clone, serde::Serialize)]
pub struct PotentialCaller {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub reason: String,
}

/// Data flow step
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowStep {
    pub variable: String,
    pub action: FlowAction,
    pub file: String,
    pub line: u32,
    pub expression: String,
}

/// Flow action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FlowAction {
    Define,
    Assign,
    Read,
    PassToFunction,
    ReturnFrom,
    Mutate,
}

impl std::fmt::Display for FlowAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlowAction::Define => write!(f, "define"),
            FlowAction::Assign => write!(f, "assign"),
            FlowAction::Read => write!(f, "read"),
            FlowAction::PassToFunction => write!(f, "pass"),
            FlowAction::ReturnFrom => write!(f, "return"),
            FlowAction::Mutate => write!(f, "mutate"),
        }
    }
}

/// Impact analysis result
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImpactResult {
    pub symbol: String,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defined_at: Option<String>,
    pub direct_callers: Vec<String>,
    pub direct_caller_count: usize,
    pub transitive_callers: Vec<String>,
    pub transitive_caller_count: usize,
    pub affected_entry_points: Vec<String>,
    pub files_affected: Vec<String>,
    pub risk_level: RiskLevel,
}

/// Risk level for impact analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

/// Pattern match result
#[derive(Debug, Clone, serde::Serialize)]
pub struct PatternMatch {
    pub file: String,
    pub line: u32,
    pub column: u16,
    pub matched_text: String,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol: Option<String>,
}

/// Variable in scope
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScopeVariable {
    pub name: String,
    pub kind: String,
    pub defined_at: u32,
}

// =============================================================================
// TRACE RESULT TYPES
// =============================================================================

/// Result of a symbol trace operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceResult {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defined_at: Option<String>,
    pub kind: String,
    pub invocation_paths: Vec<InvocationPath>,
    pub total_paths: usize,
    pub entry_points: usize,
}

/// Result of a reference trace operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct RefsResult {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defined_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<String>,
    pub references: Vec<ReferenceInfo>,
    pub total_refs: usize,
    pub by_kind: std::collections::HashMap<String, usize>,
    pub by_file: std::collections::HashMap<String, usize>,
}

/// Result of dead code analysis
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeadCodeResult {
    pub symbols: Vec<DeadSymbol>,
    pub total_dead: usize,
    pub by_kind: std::collections::HashMap<String, usize>,
    pub by_file: std::collections::HashMap<String, usize>,
}

/// Result of data flow analysis
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowResult {
    pub symbol: String,
    pub flow_paths: Vec<Vec<FlowStep>>,
}

/// Result of module tracing
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleResult {
    pub module: String,
    pub file_path: String,
    pub exports: Vec<String>,
    pub imported_by: Vec<String>,
    pub dependencies: Vec<String>,
    pub circular_deps: Vec<String>,
}

/// Result of pattern search
#[derive(Debug, Clone, serde::Serialize)]
pub struct PatternResult {
    pub pattern: String,
    pub total_matches: usize,
    pub matches: Vec<PatternMatch>,
    pub by_file: std::collections::HashMap<String, usize>,
}

/// Result of scope analysis
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScopeResult {
    pub file: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_scope: Option<String>,
    pub local_variables: Vec<ScopeVariable>,
    pub parameters: Vec<ScopeVariable>,
    pub imports: Vec<String>,
}

/// Result of statistics computation
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatsResult {
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_tokens: usize,
    pub total_references: usize,
    pub total_edges: usize,
    pub total_entry_points: usize,
    pub symbols_by_kind: std::collections::HashMap<String, usize>,
    pub files_by_extension: std::collections::HashMap<String, usize>,
    pub most_referenced: Vec<(String, usize)>,
    pub largest_files: Vec<(String, usize)>,
    pub max_call_depth: usize,
    pub avg_call_depth: f32,
}

// =============================================================================
// FORMATTER TRAIT
// =============================================================================

/// Trait for formatting trace output
pub trait TraceFormatter {
    /// Format invocation paths for a symbol
    fn format_trace(&self, result: &TraceResult) -> String;

    /// Format references to a symbol
    fn format_refs(&self, result: &RefsResult) -> String;

    /// Format dead code analysis results
    fn format_dead_code(&self, result: &DeadCodeResult) -> String;

    /// Format data flow analysis results
    fn format_flow(&self, result: &FlowResult) -> String;

    /// Format impact analysis results
    fn format_impact(&self, result: &ImpactResult) -> String;

    /// Format module tracing results
    fn format_module(&self, result: &ModuleResult) -> String;

    /// Format pattern search results
    fn format_pattern(&self, result: &PatternResult) -> String;

    /// Format scope analysis results
    fn format_scope(&self, result: &ScopeResult) -> String;

    /// Format statistics results
    fn format_stats(&self, result: &StatsResult) -> String;
}

// =============================================================================
// FACTORY FUNCTION
// =============================================================================

/// Create a formatter for the given output format
pub fn create_formatter(format: OutputFormat) -> Box<dyn TraceFormatter> {
    match format {
        OutputFormat::Ascii => Box::new(ascii::AsciiFormatter::new()),
        OutputFormat::Plain => Box::new(plain::PlainFormatter::new()),
        OutputFormat::Json => Box::new(json::JsonFormatter::new()),
        OutputFormat::Csv => Box::new(plain::CsvFormatter::new()),
        OutputFormat::Dot => Box::new(plain::DotFormatter::new()),
        OutputFormat::Markdown => Box::new(plain::MarkdownFormatter::new()),
    }
}

// =============================================================================
// RE-EXPORTS
// =============================================================================

pub use ascii::AsciiFormatter;
pub use json::JsonFormatter;
pub use plain::{CsvFormatter, DotFormatter, MarkdownFormatter, PlainFormatter};
