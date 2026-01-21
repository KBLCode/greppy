//! Trace command implementation
//!
//! Provides the `greppy trace` CLI command for tracing symbol invocations,
//! references, data flow, and dead code analysis.
//!
//! @module cli/trace

use crate::ai::trace_prompts::is_natural_language_query;
use crate::ai::{claude::ClaudeClient, gemini::GeminiClient};
use crate::auth::{self, Provider};
use crate::core::error::{Error, Result};
use crate::core::project::Project;
use crate::trace::context::FileCache;
use crate::trace::output::{
    create_formatter, ChainStep, DeadCodeResult, DeadSymbol, FlowAction, FlowResult, FlowStep,
    ImpactResult, InvocationPath, ModuleResult, OutputFormat, PatternMatch, PatternResult,
    PotentialCaller, ReferenceInfo, ReferenceKind, RefsResult, RiskLevel, ScopeResult,
    ScopeVariable, StatsResult, TraceResult,
};
use crate::trace::{
    find_dead_symbols, find_refs, load_index, trace_index_exists, trace_index_path,
    trace_symbol_by_name, RefKind, SemanticIndex, SymbolKind,
};
use clap::Args;
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Combined results for multi-operation JSON output
#[derive(Debug, Default, Serialize)]
pub struct CombinedResults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<TraceResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refs: Option<RefsResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callers: Option<TraceResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callees: Option<TraceResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_usage: Option<RefsResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<ModuleResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<FlowResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact: Option<ImpactResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead_code: Option<DeadCodeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<StatsResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycles: Option<ModuleResult>,
}

// =============================================================================
// HELPERS
// =============================================================================

/// Convert SymbolKind to string
fn symbol_kind_str(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Interface => "interface",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::Constant => "constant",
        SymbolKind::Variable => "variable",
        SymbolKind::Module => "module",
        SymbolKind::Unknown => "unknown",
    }
}

/// Convert output ReferenceKind to string
fn reference_kind_str(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::Read => "read",
        ReferenceKind::Write => "write",
        ReferenceKind::Call => "call",
        ReferenceKind::TypeAnnotation => "type",
        ReferenceKind::Import => "import",
        ReferenceKind::Export => "export",
    }
}

// =============================================================================
// ARGS
// =============================================================================

/// Arguments for the trace command
#[derive(Args, Debug)]
#[command(after_help = "EXAMPLES:
    greppy trace validateUser              Trace invocation paths
    greppy trace -d validateUser           Direct mode (no AI reranking)
    greppy trace --refs userId             Find all references
    greppy trace --refs userId -c 2        Find refs with 2 lines context
    greppy trace --refs userId --in src/   Limit to src/ directory
    greppy trace --reads userId            Find reads only
    greppy trace --writes userId           Find writes only
    greppy trace --callers fetchData       Show what calls this
    greppy trace --callees fetchData       Show what this calls
    greppy trace --type UserProfile        Trace type usage
    greppy trace --module @/lib/auth       Trace module imports/exports
    greppy trace --pattern \"TODO:.*\"       Find pattern occurrences
    greppy trace --flow userInput          Trace data flow
    greppy trace --impact login            Analyze change impact
    greppy trace --scope src/api.ts:42     Show scope at location
    greppy trace --dead                    Find unused code
    greppy trace --dead --xref             Dead code with potential callers
    greppy trace --stats                   Show codebase statistics
    greppy trace --cycles                  Find circular dependencies

COMPOSABLE FLAGS (run multiple operations at once):
    greppy trace --dead --stats            Dead code + statistics
    greppy trace --dead --stats --in src/  Filtered to src/ directory
    greppy trace --dead --stats --summary  Condensed one-line summaries
    greppy trace --refs foo --impact foo   References + impact analysis
    greppy trace --dead --cycles           Dead code + circular deps

FILTERING:
    greppy trace --dead --in src/auth      Filter to path
    greppy trace --dead --symbol-type fn   Filter by type (fn, struct, etc)
    greppy trace --dead --name \"test.*\"    Filter by name pattern

OUTPUT FORMATS:
    greppy trace --refs userId --json      JSON output for tooling
    greppy trace --dead --stats --json     Combined JSON for multi-op
    greppy trace --refs userId --plain     Plain text (no colors)
    greppy trace --refs userId --csv       CSV output
    greppy trace --refs userId --dot       DOT graph format
    greppy trace --refs userId --markdown  Markdown output")]
pub struct TraceArgs {
    /// Symbol to trace (function, class, method, variable)
    pub symbol: Option<String>,

    /// Direct mode (no AI reranking)
    #[arg(short = 'd', long)]
    pub direct: bool,

    /// Trace all references to symbol
    #[arg(long, value_name = "SYMBOL")]
    pub refs: Option<String>,

    /// Trace reads only
    #[arg(long, value_name = "SYMBOL")]
    pub reads: Option<String>,

    /// Trace writes only
    #[arg(long, value_name = "SYMBOL")]
    pub writes: Option<String>,

    /// Show what calls this symbol
    #[arg(long, value_name = "SYMBOL")]
    pub callers: Option<String>,

    /// Show what this symbol calls
    #[arg(long, value_name = "SYMBOL")]
    pub callees: Option<String>,

    /// Trace type usage
    #[arg(long = "type", value_name = "TYPE")]
    pub type_name: Option<String>,

    /// Trace module imports/exports
    #[arg(long, value_name = "MODULE")]
    pub module: Option<String>,

    /// Trace pattern occurrences (regex)
    #[arg(long, value_name = "REGEX")]
    pub pattern: Option<String>,

    /// Trace data flow
    #[arg(long, value_name = "SYMBOL")]
    pub flow: Option<String>,

    /// Analyze impact of changing symbol
    #[arg(long, value_name = "SYMBOL")]
    pub impact: Option<String>,

    /// Show scope at location (file:line)
    #[arg(long, value_name = "LOCATION")]
    pub scope: Option<String>,

    /// Find dead/unused code
    #[arg(long)]
    pub dead: bool,

    /// Cross-reference dead code (show potential callers)
    #[arg(long)]
    pub xref: bool,

    /// Show codebase statistics
    #[arg(long)]
    pub stats: bool,

    /// Find circular dependencies
    #[arg(long)]
    pub cycles: bool,

    /// Filter by reference kind (read, write, call, type, import, export)
    #[arg(long, value_name = "KIND")]
    pub kind: Option<String>,

    /// Limit search to path/directory
    #[arg(long, value_name = "PATH")]
    pub r#in: Option<PathBuf>,

    /// Filter by symbol type (function, method, class, variable, type, interface)
    #[arg(long, value_name = "TYPE")]
    pub symbol_type: Option<String>,

    /// Filter by name pattern (regex)
    #[arg(long, value_name = "PATTERN")]
    pub name: Option<String>,

    /// Group results by (file, kind, scope)
    #[arg(long, value_name = "GROUP")]
    pub group_by: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Output as plain text (no colors)
    #[arg(long)]
    pub plain: bool,

    /// Output as CSV
    #[arg(long)]
    pub csv: bool,

    /// Output as DOT graph
    #[arg(long)]
    pub dot: bool,

    /// Output as Markdown
    #[arg(long)]
    pub markdown: bool,

    /// Interactive TUI mode
    #[arg(long)]
    pub tui: bool,

    /// Maximum trace depth
    #[arg(long, default_value = "10")]
    pub max_depth: usize,

    /// Lines of code context to show (before and after)
    #[arg(long, short = 'c', default_value = "0")]
    pub context: u32,

    /// Maximum number of results to show
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show only counts, not full results
    #[arg(long)]
    pub count: bool,

    /// Summary mode: condensed output for multi-op commands
    #[arg(long)]
    pub summary: bool,

    /// Project path (default: current directory)
    #[arg(short, long)]
    pub project: Option<PathBuf>,
}

impl TraceArgs {
    /// Determine the output format from args
    fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else if self.csv {
            OutputFormat::Csv
        } else if self.dot {
            OutputFormat::Dot
        } else if self.markdown {
            OutputFormat::Markdown
        } else if self.plain {
            OutputFormat::Plain
        } else {
            OutputFormat::Ascii
        }
    }

    /// Get all operations to perform (supports composable flags)
    fn operations(&self) -> Vec<TraceOperation> {
        let mut ops = Vec::new();

        // Boolean flags (can combine)
        if self.dead {
            ops.push(TraceOperation::DeadCode);
        }
        if self.stats {
            ops.push(TraceOperation::Stats);
        }
        if self.cycles {
            ops.push(TraceOperation::Cycles);
        }

        // Symbol-based operations (can combine multiple)
        if let Some(ref loc) = self.scope {
            ops.push(TraceOperation::Scope(loc.clone()));
        }
        if let Some(ref sym) = self.impact {
            ops.push(TraceOperation::Impact(sym.clone()));
        }
        if let Some(ref sym) = self.flow {
            ops.push(TraceOperation::Flow(sym.clone()));
        }
        if let Some(ref pattern) = self.pattern {
            ops.push(TraceOperation::Pattern(pattern.clone()));
        }
        if let Some(ref module) = self.module {
            ops.push(TraceOperation::Module(module.clone()));
        }
        if let Some(ref type_name) = self.type_name {
            ops.push(TraceOperation::Type(type_name.clone()));
        }
        if let Some(ref sym) = self.callers {
            ops.push(TraceOperation::Callers(sym.clone()));
        }
        if let Some(ref sym) = self.callees {
            ops.push(TraceOperation::Callees(sym.clone()));
        }
        if let Some(ref sym) = self.reads {
            ops.push(TraceOperation::Refs {
                symbol: sym.clone(),
                kind: Some(ReferenceKind::Read),
            });
        }
        if let Some(ref sym) = self.writes {
            ops.push(TraceOperation::Refs {
                symbol: sym.clone(),
                kind: Some(ReferenceKind::Write),
            });
        }
        if let Some(ref sym) = self.refs {
            ops.push(TraceOperation::Refs {
                symbol: sym.clone(),
                kind: self.parse_kind_filter(),
            });
        }
        if let Some(ref sym) = self.symbol {
            ops.push(TraceOperation::Trace(sym.clone()));
        }

        ops
    }

    /// Parse --kind filter into ReferenceKind
    fn parse_kind_filter(&self) -> Option<ReferenceKind> {
        self.kind
            .as_ref()
            .and_then(|k| match k.to_lowercase().as_str() {
                "read" => Some(ReferenceKind::Read),
                "write" => Some(ReferenceKind::Write),
                "call" => Some(ReferenceKind::Call),
                "type" => Some(ReferenceKind::TypeAnnotation),
                "import" => Some(ReferenceKind::Import),
                "export" => Some(ReferenceKind::Export),
                _ => None,
            })
    }
}

/// The trace operation to perform
#[derive(Debug)]
enum TraceOperation {
    Trace(String),
    Refs {
        symbol: String,
        kind: Option<ReferenceKind>,
    },
    Callers(String),
    Callees(String),
    Type(String),
    Module(String),
    Pattern(String),
    Flow(String),
    Impact(String),
    Scope(String),
    DeadCode,
    Stats,
    Cycles,
}

/// Universal filter for trace operations
#[derive(Debug, Clone, Default)]
pub struct TraceFilter {
    /// Filter by file/folder path (contains match)
    pub path: Option<String>,
    /// Filter by symbol type (function, method, class, etc.)
    pub symbol_type: Option<String>,
    /// Filter by name pattern (regex)
    pub name_pattern: Option<regex::Regex>,
}

impl TraceFilter {
    /// Check if a symbol passes the filter
    pub fn matches_symbol(&self, name: &str, kind: &str, file_path: &str) -> bool {
        // Path filter
        if let Some(ref path) = self.path {
            if !file_path.contains(path) {
                return false;
            }
        }
        // Symbol type filter
        if let Some(ref stype) = self.symbol_type {
            if !kind.to_lowercase().contains(&stype.to_lowercase()) {
                return false;
            }
        }
        // Name pattern filter
        if let Some(ref pattern) = self.name_pattern {
            if !pattern.is_match(name) {
                return false;
            }
        }
        true
    }

    /// Check if a file path passes the filter
    pub fn matches_path(&self, file_path: &str) -> bool {
        if let Some(ref path) = self.path {
            return file_path.contains(path);
        }
        true
    }
}

impl TraceArgs {
    /// Build a universal filter from args
    pub fn build_filter(&self) -> TraceFilter {
        TraceFilter {
            path: self.r#in.as_ref().map(|p| p.to_string_lossy().to_string()),
            symbol_type: self.symbol_type.clone(),
            name_pattern: self.name.as_ref().and_then(|p| regex::Regex::new(p).ok()),
        }
    }
}

// =============================================================================
// COMMAND EXECUTION
// =============================================================================

/// Run the trace command
pub async fn run(args: TraceArgs) -> Result<()> {
    let project_path = args
        .project
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    let project = Project::detect(&project_path)?;
    let format = args.output_format();
    let formatter = create_formatter(format);

    // Check for TUI mode
    if args.tui {
        return run_tui(&args, &project).await;
    }

    // Get all operations (composable flags)
    let operations = args.operations();
    debug!(?operations, "Trace operations");

    // Build universal filter from args
    let filter = args.build_filter();

    // No operations specified
    if operations.is_empty() {
        eprintln!("Usage: greppy trace <symbol>");
        eprintln!("       greppy trace --refs <symbol>");
        eprintln!("       greppy trace --dead");
        eprintln!("       greppy trace --stats");
        eprintln!("       greppy trace --dead --stats  (composable!)");
        eprintln!("Run 'greppy trace --help' for more options.");
        return Err(Error::SearchError {
            message: "No symbol or operation specified".to_string(),
        });
    }

    let multi_op = operations.len() > 1;
    let summary_mode = args.summary;
    let json_multi_op = args.json && multi_op;

    // For JSON multi-op mode, collect results into combined struct
    let mut combined = CombinedResults::default();

    // Execute each operation
    for (i, operation) in operations.iter().enumerate() {
        // Print section header for multi-operation mode or summary mode (not for JSON)
        if (multi_op || summary_mode) && !json_multi_op {
            if i > 0 {
                println!();
            }
            let header = operation_header(operation);
            println!("{}", "═".repeat(79));
            println!("{}", header);
            println!("{}", "═".repeat(79));
        }

        match operation {
            TraceOperation::Trace(symbol) => {
                info!(symbol = %symbol, "Tracing symbol invocations");
                let result =
                    trace_symbol_cmd(&project, symbol, args.max_depth, args.direct, &filter)
                        .await?;
                if json_multi_op {
                    combined.trace = Some(result);
                } else if summary_mode {
                    println!(
                        "  Paths: {}  Entry points: {}",
                        result.invocation_paths.len(),
                        result.entry_points
                    );
                } else {
                    println!("{}", formatter.format_trace(&result));
                }
            }
            TraceOperation::Refs { symbol, kind } => {
                info!(symbol = %symbol, ?kind, "Finding references");
                let result = find_refs_cmd(&project, symbol, *kind, &args, &filter).await?;
                if json_multi_op {
                    combined.refs = Some(result);
                } else if args.count || summary_mode {
                    println!(
                        "  References: {}  Files: {}",
                        result.total_refs,
                        result.by_file.len()
                    );
                } else {
                    println!("{}", formatter.format_refs(&result));
                }
            }
            TraceOperation::Callers(symbol) => {
                info!(symbol = %symbol, "Finding callers");
                let result = find_callers_cmd(&project, symbol, args.max_depth, &filter).await?;
                if json_multi_op {
                    combined.callers = Some(result);
                } else if summary_mode {
                    println!(
                        "  Callers: {}  Paths: {}",
                        result.entry_points,
                        result.invocation_paths.len()
                    );
                } else {
                    println!("{}", formatter.format_trace(&result));
                }
            }
            TraceOperation::Callees(symbol) => {
                info!(symbol = %symbol, "Finding callees");
                let result = find_callees_cmd(&project, symbol, args.max_depth, &filter).await?;
                if json_multi_op {
                    combined.callees = Some(result);
                } else if summary_mode {
                    println!("  Callees: {}", result.invocation_paths.len());
                } else {
                    println!("{}", formatter.format_trace(&result));
                }
            }
            TraceOperation::Type(type_name) => {
                info!(type_name = %type_name, "Tracing type usage");
                let result = find_refs_cmd(
                    &project,
                    type_name,
                    Some(ReferenceKind::TypeAnnotation),
                    &args,
                    &filter,
                )
                .await?;
                if json_multi_op {
                    combined.type_usage = Some(result);
                } else if summary_mode {
                    println!("  Type usages: {}", result.total_refs);
                } else {
                    println!("{}", formatter.format_refs(&result));
                }
            }
            TraceOperation::Module(module) => {
                info!(module = %module, "Tracing module");
                let result = trace_module_cmd(&project, module, &filter).await?;
                if json_multi_op {
                    combined.module = Some(result);
                } else if summary_mode {
                    println!(
                        "  Exports: {}  Imported by: {}  Deps: {}",
                        result.exports.len(),
                        result.imported_by.len(),
                        result.dependencies.len()
                    );
                } else {
                    println!("{}", formatter.format_module(&result));
                }
            }
            TraceOperation::Pattern(pattern) => {
                info!(pattern = %pattern, "Tracing pattern");
                let result = trace_pattern_cmd(&project, pattern, &args, &filter).await?;
                if json_multi_op {
                    combined.pattern = Some(result);
                } else if summary_mode {
                    println!(
                        "  Matches: {}  Files: {}",
                        result.total_matches,
                        result.by_file.len()
                    );
                } else {
                    println!("{}", formatter.format_pattern(&result));
                }
            }
            TraceOperation::Flow(symbol) => {
                info!(symbol = %symbol, "Tracing data flow");
                let result = trace_flow_cmd(&project, symbol, &args, &filter).await?;
                if json_multi_op {
                    combined.flow = Some(result);
                } else if summary_mode {
                    let total_steps: usize = result.flow_paths.iter().map(|p| p.len()).sum();
                    println!(
                        "  Flow paths: {}  Steps: {}",
                        result.flow_paths.len(),
                        total_steps
                    );
                } else {
                    println!("{}", formatter.format_flow(&result));
                }
            }
            TraceOperation::Impact(symbol) => {
                info!(symbol = %symbol, "Analyzing impact");
                let result = analyze_impact_cmd(&project, symbol, args.max_depth, &filter).await?;
                if json_multi_op {
                    combined.impact = Some(result);
                } else if summary_mode {
                    println!(
                        "  Direct callers: {}  Transitive: {}  Entry points: {}  Risk: {:?}",
                        result.direct_callers.len(),
                        result.transitive_callers.len(),
                        result.affected_entry_points.len(),
                        result.risk_level
                    );
                } else {
                    println!("{}", formatter.format_impact(&result));
                }
            }
            TraceOperation::Scope(location) => {
                info!(location = %location, "Analyzing scope");
                let result = analyze_scope_cmd(&project, location, &filter).await?;
                if json_multi_op {
                    combined.scope = Some(result);
                } else if summary_mode {
                    println!(
                        "  Scope: {}  Variables: {}  Imports: {}",
                        result.enclosing_scope.as_deref().unwrap_or("global"),
                        result.local_variables.len(),
                        result.imports.len()
                    );
                } else {
                    println!("{}", formatter.format_scope(&result));
                }
            }
            TraceOperation::DeadCode => {
                info!("Finding dead code");
                let result = find_dead_code_cmd(&project, args.limit, &filter, args.xref).await?;
                if json_multi_op {
                    combined.dead_code = Some(result);
                } else if args.count || summary_mode {
                    let kinds: Vec<_> = result
                        .by_kind
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect();
                    println!(
                        "  Dead symbols: {}  ({})",
                        result.total_dead,
                        kinds.join(", ")
                    );
                } else {
                    println!("{}", formatter.format_dead_code(&result));
                }
            }
            TraceOperation::Stats => {
                info!("Computing statistics");
                let result = compute_stats_cmd(&project, &filter).await?;
                if json_multi_op {
                    combined.stats = Some(result);
                } else if summary_mode {
                    println!(
                        "  Files: {}  Symbols: {}  Refs: {}  Edges: {}",
                        result.total_files,
                        result.total_symbols,
                        result.total_references,
                        result.total_edges
                    );
                } else {
                    println!("{}", formatter.format_stats(&result));
                }
            }
            TraceOperation::Cycles => {
                info!("Finding circular dependencies");
                let result = find_cycles_cmd(&project, &filter).await?;
                if json_multi_op {
                    combined.cycles = Some(result);
                } else if summary_mode {
                    println!("  Circular deps: {}", result.circular_deps.len());
                } else {
                    println!("{}", formatter.format_module(&result));
                }
            }
        }
    }

    // Output combined JSON for multi-op JSON mode
    if json_multi_op {
        println!(
            "{}",
            serde_json::to_string_pretty(&combined)
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        );
    }

    Ok(())
}

/// Generate header text for an operation in multi-op mode
fn operation_header(op: &TraceOperation) -> String {
    match op {
        TraceOperation::Trace(s) => format!("TRACE: {}", s),
        TraceOperation::Refs { symbol, kind } => {
            if let Some(k) = kind {
                format!("REFERENCES: {} ({:?})", symbol, k)
            } else {
                format!("REFERENCES: {}", symbol)
            }
        }
        TraceOperation::Callers(s) => format!("CALLERS: {}", s),
        TraceOperation::Callees(s) => format!("CALLEES: {}", s),
        TraceOperation::Type(s) => format!("TYPE USAGE: {}", s),
        TraceOperation::Module(s) => format!("MODULE: {}", s),
        TraceOperation::Pattern(s) => format!("PATTERN: {}", s),
        TraceOperation::Flow(s) => format!("DATA FLOW: {}", s),
        TraceOperation::Impact(s) => format!("IMPACT ANALYSIS: {}", s),
        TraceOperation::Scope(s) => format!("SCOPE: {}", s),
        TraceOperation::DeadCode => "DEAD CODE ANALYSIS".to_string(),
        TraceOperation::Stats => "CODEBASE STATISTICS".to_string(),
        TraceOperation::Cycles => "CIRCULAR DEPENDENCIES".to_string(),
    }
}

// =============================================================================
// INDEX LOADING
// =============================================================================

/// Load the semantic index for a project
fn load_semantic_index(project: &Project) -> Result<SemanticIndex> {
    let index_path = trace_index_path(&project.root);

    if !trace_index_exists(&project.root) {
        return Err(Error::IndexError {
            message: format!(
                "Trace index not found. Run 'greppy index' first.\nExpected at: {}",
                index_path.display()
            ),
        });
    }

    load_index(&index_path).map_err(|e| Error::IndexError {
        message: format!("Failed to load trace index: {}", e),
    })
}

// =============================================================================
// PHASE 1: CODE CONTEXT ENGINE
// =============================================================================

/// Get code context for a reference
fn get_code_context(cache: &mut FileCache, file: &Path, line: u32, context_lines: u32) -> String {
    if context_lines == 0 {
        // Just get the single line
        cache
            .get_line(file, line)
            .map(|l| l.trim().to_string())
            .unwrap_or_else(|| format!("// line {}", line))
    } else {
        // Get context with surrounding lines
        cache
            .get_context(file, line, context_lines, context_lines)
            .map(|ctx| ctx.format(false))
            .unwrap_or_else(|| format!("// line {}", line))
    }
}

// =============================================================================
// PHASE 2: COMPLETE REFERENCE SYSTEM
// =============================================================================

/// Find references to a symbol with full context
async fn find_refs_cmd(
    project: &Project,
    symbol: &str,
    kind_filter: Option<ReferenceKind>,
    args: &TraceArgs,
    filter: &TraceFilter,
) -> Result<RefsResult> {
    debug!(symbol = %symbol, ?kind_filter, ?filter, "find_refs");

    let index = load_semantic_index(project)?;
    let mut cache = FileCache::new(&project.root);

    let mut references = Vec::new();
    let mut by_kind: HashMap<String, usize> = HashMap::new();
    let mut by_file: HashMap<String, usize> = HashMap::new();

    // Find symbol IDs matching the name
    let symbol_ids = index.symbols_by_name(symbol).cloned().unwrap_or_default();

    // Get definition location from first matching symbol
    let defined_at = symbol_ids.first().and_then(|&id| {
        let sym = index.symbol(id)?;
        let file = index.file_path(sym.file_id)?;
        Some(format!("{}:{}", file.display(), sym.start_line))
    });

    // Get symbol kind
    let symbol_kind = symbol_ids
        .first()
        .and_then(|&id| index.symbol(id))
        .map(|s| symbol_kind_str(s.symbol_kind()).to_string());

    // Find all references to all matching symbols (via Reference table)
    for &sym_id in &symbol_ids {
        let refs = find_refs(&index, sym_id);

        for ref_ctx in refs {
            // Convert RefKind to ReferenceKind
            let kind = match ref_ctx.reference.ref_kind() {
                RefKind::Read => ReferenceKind::Read,
                RefKind::Write => ReferenceKind::Write,
                RefKind::Call => ReferenceKind::Call,
                RefKind::TypeAnnotation => ReferenceKind::TypeAnnotation,
                RefKind::Import => ReferenceKind::Import,
                RefKind::Export => ReferenceKind::Export,
                RefKind::Inheritance | RefKind::Decorator | RefKind::Unknown => ReferenceKind::Read,
            };

            // Apply kind filter
            if let Some(filter_kind) = kind_filter {
                if kind != filter_kind {
                    continue;
                }
            }

            // Get file path
            let file_path = index
                .file_path(ref_ctx.file_id)
                .map(|p| p.to_path_buf())
                .unwrap_or_default();
            let file = file_path.to_string_lossy().to_string();

            // Apply universal filter (path, type, name)
            if !filter.matches_path(&file) {
                continue;
            }

            // Find enclosing symbol
            let enclosing_symbol = find_enclosing_symbol(&index, ref_ctx.file_id, ref_ctx.line);

            // Get code context
            let context = get_code_context(&mut cache, &file_path, ref_ctx.line, args.context);

            // Count by kind and file
            *by_kind
                .entry(reference_kind_str(kind).to_string())
                .or_insert(0) += 1;
            *by_file.entry(file.clone()).or_insert(0) += 1;

            references.push(ReferenceInfo {
                file,
                line: ref_ctx.line,
                column: ref_ctx.column,
                kind,
                context,
                enclosing_symbol,
            });
        }
    }

    // ALWAYS search tokens by name (catches variables, params, field names)
    if let Some(token_ids) = index.tokens_by_name(symbol) {
        for &token_id in token_ids {
            if let Some(token) = index.token(token_id) {
                let file_path = index
                    .file_path(token.file_id)
                    .map(|p| p.to_path_buf())
                    .unwrap_or_default();
                let file = file_path.to_string_lossy().to_string();

                // Skip if we already have this location
                let already_have = references
                    .iter()
                    .any(|r| r.file == file && r.line == token.line && r.column == token.column);

                if already_have {
                    continue;
                }

                // Apply path filter
                if let Some(ref in_path) = args.r#in {
                    if !file.contains(&in_path.to_string_lossy().to_string()) {
                        continue;
                    }
                }

                let kind = match token.token_kind() {
                    crate::trace::TokenKind::Call => ReferenceKind::Call,
                    _ => ReferenceKind::Read,
                };

                // Apply kind filter
                if let Some(filter_kind) = kind_filter {
                    if kind != filter_kind {
                        continue;
                    }
                }

                let enclosing_symbol = find_enclosing_symbol(&index, token.file_id, token.line);
                let context = get_code_context(&mut cache, &file_path, token.line, args.context);

                *by_kind
                    .entry(reference_kind_str(kind).to_string())
                    .or_insert(0) += 1;
                *by_file.entry(file.clone()).or_insert(0) += 1;

                references.push(ReferenceInfo {
                    file,
                    line: token.line,
                    column: token.column,
                    kind,
                    context,
                    enclosing_symbol,
                });
            }
        }
    }

    // Sort by file and line
    references.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

    // Apply limit
    if let Some(limit) = args.limit {
        references.truncate(limit);
    }

    Ok(RefsResult {
        symbol: symbol.to_string(),
        defined_at,
        symbol_kind,
        total_refs: references.len(),
        references,
        by_kind,
        by_file,
    })
}

/// Find the enclosing symbol for a given location
fn find_enclosing_symbol(index: &SemanticIndex, file_id: u16, line: u32) -> Option<String> {
    let mut best: Option<(&crate::trace::Symbol, u32)> = None;

    for symbol in &index.symbols {
        if symbol.file_id == file_id && symbol.start_line <= line && symbol.end_line >= line {
            let size = symbol.end_line - symbol.start_line;
            match best {
                None => best = Some((symbol, size)),
                Some((_, best_size)) if size < best_size => best = Some((symbol, size)),
                _ => {}
            }
        }
    }

    best.and_then(|(sym, _)| index.symbol_name(sym).map(|s| s.to_string()))
}

// =============================================================================
// PHASE 3: CALL GRAPH WITH FULL PRECISION
// =============================================================================

/// Find what calls a symbol (callers/incoming)
async fn find_callers_cmd(
    project: &Project,
    symbol: &str,
    max_depth: usize,
    filter: &TraceFilter,
) -> Result<TraceResult> {
    let _ = filter; // TODO: apply filter to results
    debug!(symbol = %symbol, "find_callers");

    let index = load_semantic_index(project)?;

    let symbol_ids = index.symbols_by_name(symbol).cloned().unwrap_or_default();
    if symbol_ids.is_empty() {
        return Ok(TraceResult {
            symbol: symbol.to_string(),
            defined_at: None,
            kind: "unknown".to_string(),
            invocation_paths: Vec::new(),
            total_paths: 0,
            entry_points: 0,
        });
    }

    let mut paths = Vec::new();
    let mut visited = HashSet::new();

    for &sym_id in &symbol_ids {
        collect_callers_recursive(
            &index,
            sym_id,
            &mut paths,
            &mut visited,
            Vec::new(),
            max_depth,
        );
    }

    let defined_at = symbol_ids.first().and_then(|&id| {
        let sym = index.symbol(id)?;
        let file = index.file_path(sym.file_id)?;
        Some(format!("{}:{}", file.display(), sym.start_line))
    });

    let kind = symbol_ids
        .first()
        .and_then(|&id| index.symbol(id))
        .map(|s| symbol_kind_str(s.symbol_kind()).to_string())
        .unwrap_or_else(|| "function".to_string());

    let entry_points = paths
        .iter()
        .map(|p| &p.entry_point)
        .collect::<HashSet<_>>()
        .len();

    Ok(TraceResult {
        symbol: symbol.to_string(),
        defined_at,
        kind,
        invocation_paths: paths.clone(),
        total_paths: paths.len(),
        entry_points,
    })
}

fn collect_callers_recursive(
    index: &SemanticIndex,
    sym_id: u32,
    paths: &mut Vec<InvocationPath>,
    visited: &mut HashSet<u32>,
    current_chain: Vec<ChainStep>,
    max_depth: usize,
) {
    if current_chain.len() >= max_depth {
        return;
    }

    let callers = index.callers(sym_id);
    if callers.is_empty() && !current_chain.is_empty() {
        // End of chain - record path
        if let Some(sym) = index.symbol(sym_id) {
            let name = index.symbol_name(sym).unwrap_or("<unknown>");
            let file = index
                .file_path(sym.file_id)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let mut chain = current_chain.clone();
            chain.push(ChainStep {
                symbol: name.to_string(),
                file: file.clone(),
                line: sym.start_line,
                column: None,
                context: None,
            });

            paths.push(InvocationPath {
                entry_point: format!("{} ({})", name, file),
                entry_kind: symbol_kind_str(sym.symbol_kind()).to_string(),
                chain,
            });
        }
        return;
    }

    for &caller_id in callers {
        if visited.contains(&caller_id) {
            continue;
        }
        visited.insert(caller_id);

        if let Some(caller) = index.symbol(caller_id) {
            let name = index.symbol_name(caller).unwrap_or("<unknown>");
            let file = index
                .file_path(caller.file_id)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            // Find call line from edge
            let call_line = index
                .edges
                .iter()
                .find(|e| e.from_symbol == caller_id && e.to_symbol == sym_id)
                .map(|e| e.line)
                .unwrap_or(caller.start_line);

            let mut new_chain = current_chain.clone();
            new_chain.push(ChainStep {
                symbol: name.to_string(),
                file,
                line: call_line,
                column: None,
                context: None,
            });

            collect_callers_recursive(index, caller_id, paths, visited, new_chain, max_depth);
        }
    }
}

/// Find what a symbol calls (callees/outgoing)
async fn find_callees_cmd(
    project: &Project,
    symbol: &str,
    max_depth: usize,
    filter: &TraceFilter,
) -> Result<TraceResult> {
    let _ = filter; // TODO: apply filter to results
    debug!(symbol = %symbol, "find_callees");

    let index = load_semantic_index(project)?;

    let symbol_ids = index.symbols_by_name(symbol).cloned().unwrap_or_default();
    if symbol_ids.is_empty() {
        return Ok(TraceResult {
            symbol: symbol.to_string(),
            defined_at: None,
            kind: "unknown".to_string(),
            invocation_paths: Vec::new(),
            total_paths: 0,
            entry_points: 0,
        });
    }

    let mut paths = Vec::new();

    for &sym_id in &symbol_ids {
        let mut visited = HashSet::new();
        collect_callees_recursive(
            &index,
            sym_id,
            &mut paths,
            &mut visited,
            Vec::new(),
            max_depth,
        );
    }

    let defined_at = symbol_ids.first().and_then(|&id| {
        let sym = index.symbol(id)?;
        let file = index.file_path(sym.file_id)?;
        Some(format!("{}:{}", file.display(), sym.start_line))
    });

    let kind = symbol_ids
        .first()
        .and_then(|&id| index.symbol(id))
        .map(|s| symbol_kind_str(s.symbol_kind()).to_string())
        .unwrap_or_else(|| "function".to_string());

    Ok(TraceResult {
        symbol: symbol.to_string(),
        defined_at,
        kind,
        invocation_paths: paths.clone(),
        total_paths: paths.len(),
        entry_points: 1,
    })
}

fn collect_callees_recursive(
    index: &SemanticIndex,
    sym_id: u32,
    paths: &mut Vec<InvocationPath>,
    visited: &mut HashSet<u32>,
    current_chain: Vec<ChainStep>,
    max_depth: usize,
) {
    if current_chain.len() >= max_depth {
        return;
    }

    visited.insert(sym_id);

    let callees = index.callees(sym_id);

    // Add current symbol to chain
    let mut chain = current_chain.clone();
    if let Some(sym) = index.symbol(sym_id) {
        let name = index.symbol_name(sym).unwrap_or("<unknown>");
        let file = index
            .file_path(sym.file_id)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        chain.push(ChainStep {
            symbol: name.to_string(),
            file: file.clone(),
            line: sym.start_line,
            column: None,
            context: None,
        });

        if callees.is_empty() && !chain.is_empty() {
            // End of chain
            paths.push(InvocationPath {
                entry_point: chain.first().map(|c| c.symbol.clone()).unwrap_or_default(),
                entry_kind: symbol_kind_str(sym.symbol_kind()).to_string(),
                chain: chain.clone(),
            });
            return;
        }
    }

    for &callee_id in callees {
        if visited.contains(&callee_id) {
            continue;
        }
        collect_callees_recursive(index, callee_id, paths, visited, chain.clone(), max_depth);
    }
}

// =============================================================================
// PHASE 4: IMPACT ANALYSIS (REAL DATA)
// =============================================================================

/// Analyze impact of changing a symbol
async fn analyze_impact_cmd(
    project: &Project,
    symbol: &str,
    max_depth: usize,
    filter: &TraceFilter,
) -> Result<ImpactResult> {
    let _ = filter; // TODO: apply filter to results
    debug!(symbol = %symbol, "analyze_impact");

    let index = load_semantic_index(project)?;

    // Parse file:symbol format if present
    let sym_name = if symbol.contains(':') {
        symbol.splitn(2, ':').nth(1).unwrap_or(symbol)
    } else {
        symbol
    };

    let symbol_ids = index.symbols_by_name(sym_name).cloned().unwrap_or_default();

    if symbol_ids.is_empty() {
        return Ok(ImpactResult {
            symbol: symbol.to_string(),
            file: String::new(),
            defined_at: None,
            direct_callers: Vec::new(),
            direct_caller_count: 0,
            transitive_callers: Vec::new(),
            transitive_caller_count: 0,
            affected_entry_points: Vec::new(),
            files_affected: Vec::new(),
            risk_level: RiskLevel::Low,
        });
    }

    let first_id = symbol_ids[0];
    let defined_at = index.symbol(first_id).and_then(|sym| {
        let file = index.file_path(sym.file_id)?;
        Some(format!("{}:{}", file.display(), sym.start_line))
    });

    let file = index
        .symbol(first_id)
        .and_then(|s| index.file_path(s.file_id))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // Collect direct callers (deduplicated)
    let mut direct_callers_set = HashSet::new();
    let mut direct_caller_files = HashSet::new();

    for &sym_id in &symbol_ids {
        for &caller_id in index.callers(sym_id) {
            if let Some(caller) = index.symbol(caller_id) {
                let name = index.symbol_name(caller).unwrap_or("<unknown>");
                let caller_file = index
                    .file_path(caller.file_id)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let caller_str = format!("{} ({}:{})", name, caller_file, caller.start_line);
                direct_callers_set.insert(caller_str);
                direct_caller_files.insert(caller_file);
            }
        }
    }
    let direct_callers: Vec<_> = direct_callers_set.into_iter().collect();

    // Collect transitive callers via BFS (deduplicated)
    let mut transitive_callers_set = HashSet::new();
    let mut visited = HashSet::new();
    let mut queue: Vec<(u32, usize)> = symbol_ids.iter().map(|&id| (id, 0)).collect();
    let mut affected_entry_points_set = HashSet::new();
    let mut all_files = HashSet::new();

    while let Some((current, depth)) = queue.pop() {
        if depth > max_depth || visited.contains(&current) {
            continue;
        }
        visited.insert(current);

        if let Some(sym) = index.symbol(current) {
            let sym_file = index
                .file_path(sym.file_id)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            all_files.insert(sym_file.clone());

            if sym.is_entry_point() {
                let name = index.symbol_name(sym).unwrap_or("<unknown>");
                affected_entry_points_set.insert(format!("{} ({})", name, sym_file));
            }

            if depth > 1 {
                let name = index.symbol_name(sym).unwrap_or("<unknown>");
                transitive_callers_set.insert(format!("{} (depth {})", name, depth));
            }
        }

        for &caller_id in index.callers(current) {
            if !visited.contains(&caller_id) {
                queue.push((caller_id, depth + 1));
            }
        }
    }
    let transitive_callers: Vec<_> = transitive_callers_set.into_iter().collect();
    let affected_entry_points: Vec<_> = affected_entry_points_set.into_iter().collect();

    // Determine risk level
    let risk_level = if affected_entry_points.len() > 10 || all_files.len() > 50 {
        RiskLevel::Critical
    } else if affected_entry_points.len() > 5 || all_files.len() > 20 {
        RiskLevel::High
    } else if direct_callers.len() > 5 || all_files.len() > 5 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    Ok(ImpactResult {
        symbol: symbol.to_string(),
        file,
        defined_at,
        direct_callers: direct_callers.clone(),
        direct_caller_count: direct_callers.len(),
        transitive_callers: transitive_callers.clone(),
        transitive_caller_count: transitive_callers.len(),
        affected_entry_points,
        files_affected: all_files.into_iter().collect(),
        risk_level,
    })
}

// =============================================================================
// PHASE 5: TYPE TRACING (handled by refs with TypeAnnotation filter)
// =============================================================================

// Type tracing is implemented via find_refs_cmd with ReferenceKind::TypeAnnotation

// =============================================================================
// PHASE 6: MODULE TRACING
// =============================================================================

/// Trace module imports/exports
async fn trace_module_cmd(
    project: &Project,
    module: &str,
    filter: &TraceFilter,
) -> Result<ModuleResult> {
    let _ = filter; // TODO: apply filter to results
    debug!(module = %module, "trace_module");

    let index = load_semantic_index(project)?;

    // Find files that match the module pattern
    let module_files: Vec<_> = index
        .files
        .iter()
        .enumerate()
        .filter(|(_, path)| path.to_string_lossy().contains(module))
        .collect();

    let mut exports = Vec::new();
    let mut imported_by = Vec::new();
    let mut dependencies = Vec::new();

    for (file_id, _file_path) in &module_files {
        // Find exported symbols from this file
        for symbol in index.symbols_in_file(*file_id as u16) {
            if symbol.is_exported() {
                let name = index.symbol_name(symbol).unwrap_or("<unknown>");
                exports.push(format!(
                    "{} ({})",
                    name,
                    symbol_kind_str(symbol.symbol_kind())
                ));
            }
        }

        // Find imports of this module (tokens with Import kind referencing this file)
        // This is approximate - we look for symbols from this file being referenced elsewhere
        for symbol in index.symbols_in_file(*file_id as u16) {
            for reference in index.references_to(symbol.id) {
                if reference.ref_kind() == RefKind::Import {
                    if let Some(token) = index.token(reference.token_id) {
                        if token.file_id != *file_id as u16 {
                            let importer_file = index
                                .file_path(token.file_id)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let name = index.symbol_name(symbol).unwrap_or("<unknown>");
                            imported_by.push(format!("{} imports {}", importer_file, name));
                        }
                    }
                }
            }
        }
    }

    // Find what this module depends on (imports from other modules)
    for (file_id, _) in &module_files {
        for token in index.tokens_in_file(*file_id as u16) {
            if token.token_kind() == crate::trace::TokenKind::Import {
                let name = index.token_name(token).unwrap_or("<unknown>");
                if !dependencies.contains(&name.to_string()) {
                    dependencies.push(name.to_string());
                }
            }
        }
    }

    let file_path = module_files
        .first()
        .map(|(_, p)| p.to_string_lossy().to_string())
        .unwrap_or_else(|| module.to_string());

    Ok(ModuleResult {
        module: module.to_string(),
        file_path,
        exports,
        imported_by,
        dependencies,
        circular_deps: Vec::new(), // Populated by cycles command
    })
}

/// Find circular dependencies
async fn find_cycles_cmd(project: &Project, filter: &TraceFilter) -> Result<ModuleResult> {
    debug!("find_cycles filter={:?}", filter);

    let index = load_semantic_index(project)?;

    // Helper to check if file passes filter
    let file_passes = |file_id: u16| -> bool {
        if let Some(path) = index.file_path(file_id) {
            filter.matches_path(&path.to_string_lossy())
        } else {
            false
        }
    };

    // Build file dependency graph (filtered)
    let mut file_deps: HashMap<u16, HashSet<u16>> = HashMap::new();

    for edge in &index.edges {
        if let (Some(from_sym), Some(to_sym)) =
            (index.symbol(edge.from_symbol), index.symbol(edge.to_symbol))
        {
            if from_sym.file_id != to_sym.file_id {
                // Only include if at least one file passes the filter (or no filter)
                let from_passes = file_passes(from_sym.file_id);
                let to_passes = file_passes(to_sym.file_id);

                if from_passes || to_passes || filter.path.is_none() {
                    file_deps
                        .entry(from_sym.file_id)
                        .or_default()
                        .insert(to_sym.file_id);
                }
            }
        }
    }

    // Find cycles using DFS
    let mut cycles = Vec::new();
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();

    for &file_id in file_deps.keys() {
        find_cycles_dfs(
            file_id,
            &file_deps,
            &mut visited,
            &mut rec_stack,
            &mut path,
            &mut cycles,
            &index,
            filter,
        );
    }

    Ok(ModuleResult {
        module: "Circular Dependencies".to_string(),
        file_path: String::new(),
        exports: Vec::new(),
        imported_by: Vec::new(),
        dependencies: Vec::new(),
        circular_deps: cycles,
    })
}

fn find_cycles_dfs(
    node: u16,
    graph: &HashMap<u16, HashSet<u16>>,
    visited: &mut HashSet<u16>,
    rec_stack: &mut HashSet<u16>,
    path: &mut Vec<u16>,
    cycles: &mut Vec<String>,
    index: &SemanticIndex,
    filter: &TraceFilter,
) {
    visited.insert(node);
    rec_stack.insert(node);
    path.push(node);

    if let Some(neighbors) = graph.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                find_cycles_dfs(
                    neighbor, graph, visited, rec_stack, path, cycles, index, filter,
                );
            } else if rec_stack.contains(&neighbor) {
                // Found a cycle
                let cycle_start = path.iter().position(|&n| n == neighbor).unwrap_or(0);
                let cycle_path: Vec<_> = path[cycle_start..]
                    .iter()
                    .filter_map(|&fid| {
                        index
                            .file_path(fid)
                            .map(|p| p.to_string_lossy().to_string())
                    })
                    .collect();

                if !cycle_path.is_empty() {
                    // Only include cycle if at least one file in the cycle passes the filter
                    let cycle_passes =
                        filter.path.is_none() || cycle_path.iter().any(|p| filter.matches_path(p));

                    if cycle_passes {
                        cycles.push(cycle_path.join(" -> ") + " -> " + &cycle_path[0]);
                    }
                }
            }
        }
    }

    path.pop();
    rec_stack.remove(&node);
}

// =============================================================================
// PHASE 7: DATA FLOW TRACING
// =============================================================================

/// Trace data flow for a variable
async fn trace_flow_cmd(
    project: &Project,
    symbol: &str,
    args: &TraceArgs,
    filter: &TraceFilter,
) -> Result<FlowResult> {
    debug!(symbol = %symbol, "trace_flow filter={:?}", filter);

    let index = load_semantic_index(project)?;
    let mut cache = FileCache::new(&project.root);

    let mut flow_paths = Vec::new();
    let mut current_path = Vec::new();

    // Find all tokens with this name
    if let Some(token_ids) = index.tokens_by_name(symbol) {
        // Group by file and sort by line
        let mut by_file: HashMap<u16, Vec<&crate::trace::Token>> = HashMap::new();
        for &token_id in token_ids {
            if let Some(token) = index.token(token_id) {
                // Apply path filter
                if let Some(path) = index.file_path(token.file_id) {
                    if !filter.matches_path(&path.to_string_lossy()) {
                        continue;
                    }
                }
                by_file.entry(token.file_id).or_default().push(token);
            }
        }

        for (file_id, mut tokens) in by_file {
            tokens.sort_by_key(|t| (t.line, t.column));

            let file_path = index
                .file_path(file_id)
                .map(|p| p.to_path_buf())
                .unwrap_or_default();
            let file = file_path.to_string_lossy().to_string();

            for token in tokens {
                // Map token kinds to flow actions
                // Note: TokenKind doesn't have Assignment/Return, so we infer from context
                let action = match token.token_kind() {
                    crate::trace::TokenKind::Call => FlowAction::PassToFunction,
                    crate::trace::TokenKind::Property => FlowAction::Read,
                    crate::trace::TokenKind::Identifier => FlowAction::Read, // Could be assign or read
                    _ => FlowAction::Read,
                };

                // First occurrence is considered a definition
                let actual_action = if current_path.is_empty() {
                    FlowAction::Define
                } else {
                    action
                };

                let expression = get_code_context(&mut cache, &file_path, token.line, 0);

                current_path.push(FlowStep {
                    variable: symbol.to_string(),
                    action: actual_action,
                    file: file.clone(),
                    line: token.line,
                    expression,
                });
            }
        }
    }

    if !current_path.is_empty() {
        flow_paths.push(current_path);
    }

    // Apply limit
    if let Some(limit) = args.limit {
        for path in &mut flow_paths {
            path.truncate(limit);
        }
    }

    Ok(FlowResult {
        symbol: symbol.to_string(),
        flow_paths,
    })
}

// =============================================================================
// PHASE 8: PATTERN SEARCH
// =============================================================================

/// Search for regex pattern in codebase
async fn trace_pattern_cmd(
    project: &Project,
    pattern: &str,
    args: &TraceArgs,
    filter: &TraceFilter,
) -> Result<PatternResult> {
    debug!(pattern = %pattern, "trace_pattern filter={:?}", filter);

    let regex = Regex::new(pattern).map_err(|e| Error::SearchError {
        message: format!("Invalid regex pattern: {}", e),
    })?;

    let index = load_semantic_index(project)?;
    let mut cache = FileCache::new(&project.root);

    let mut matches = Vec::new();

    for (file_id, file_path) in index.files.iter().enumerate() {
        // Apply universal path filter
        let file_str = file_path.to_string_lossy();
        if !filter.matches_path(&file_str) {
            continue;
        }

        // Read file and search
        if let Some(line_count) = cache.line_count(file_path) {
            for line_num in 1..=line_count as u32 {
                if let Some(line_content) = cache.get_line(file_path, line_num) {
                    if let Some(mat) = regex.find(&line_content) {
                        let context = if args.context > 0 {
                            cache
                                .get_context(file_path, line_num, args.context, args.context)
                                .map(|ctx| ctx.format(false))
                                .unwrap_or_else(|| line_content.clone())
                        } else {
                            line_content.trim().to_string()
                        };

                        let enclosing = find_enclosing_symbol(&index, file_id as u16, line_num);

                        matches.push(PatternMatch {
                            file: file_path.to_string_lossy().to_string(),
                            line: line_num,
                            column: mat.start() as u16,
                            matched_text: mat.as_str().to_string(),
                            context,
                            enclosing_symbol: enclosing,
                        });

                        // Apply limit
                        if let Some(limit) = args.limit {
                            if matches.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Check limit
        if let Some(limit) = args.limit {
            if matches.len() >= limit {
                break;
            }
        }
    }

    // Count by file
    let mut by_file: HashMap<String, usize> = HashMap::new();
    for m in &matches {
        *by_file.entry(m.file.clone()).or_insert(0) += 1;
    }

    Ok(PatternResult {
        pattern: pattern.to_string(),
        total_matches: matches.len(),
        matches,
        by_file,
    })
}

// =============================================================================
// PHASE 9: SCOPE ANALYSIS
// =============================================================================

/// Analyze scope at a specific location
async fn analyze_scope_cmd(
    project: &Project,
    location: &str,
    filter: &TraceFilter,
) -> Result<ScopeResult> {
    debug!(location = %location, "analyze_scope");
    let _ = filter; // TODO: Apply filter to scope analysis

    // Parse file:line format
    let parts: Vec<&str> = location.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(Error::SearchError {
            message: "Location must be in format file:line".to_string(),
        });
    }

    let line: u32 = parts[0].parse().map_err(|_| Error::SearchError {
        message: "Invalid line number".to_string(),
    })?;
    let file_pattern = parts[1];

    let index = load_semantic_index(project)?;

    // Find the file
    let file_id = index
        .files
        .iter()
        .enumerate()
        .find(|(_, p)| p.to_string_lossy().contains(file_pattern))
        .map(|(id, _)| id as u16);

    let file_id = match file_id {
        Some(id) => id,
        None => {
            return Err(Error::SearchError {
                message: format!("File not found: {}", file_pattern),
            });
        }
    };

    let file_path = index
        .file_path(file_id)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // Find enclosing scope
    let enclosing_scope = find_enclosing_symbol(&index, file_id, line);

    // Find local variables (symbols in the same scope that are defined before this line)
    let mut local_variables = Vec::new();
    let mut parameters = Vec::new();
    let mut imports = Vec::new();

    for symbol in index.symbols_in_file(file_id) {
        let name = index.symbol_name(symbol).unwrap_or("<unknown>");
        let kind = symbol_kind_str(symbol.symbol_kind());

        // Check if this symbol is in scope at the given line
        if symbol.start_line <= line && symbol.end_line >= line {
            // This is the enclosing function/class
            continue;
        }

        if symbol.start_line < line && symbol.end_line < line {
            // Symbol defined before the line
            match symbol.symbol_kind() {
                SymbolKind::Variable | SymbolKind::Constant => {
                    local_variables.push(ScopeVariable {
                        name: name.to_string(),
                        kind: kind.to_string(),
                        defined_at: symbol.start_line,
                    });
                }
                SymbolKind::Function | SymbolKind::Method => {
                    // Could be a parameter if inside a function
                    if let Some(ref scope) = enclosing_scope {
                        if name != scope {
                            parameters.push(ScopeVariable {
                                name: name.to_string(),
                                kind: kind.to_string(),
                                defined_at: symbol.start_line,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Find imports (tokens with Import kind in this file)
    for token in index.tokens_in_file(file_id) {
        if token.token_kind() == crate::trace::TokenKind::Import {
            let name = index.token_name(token).unwrap_or("<unknown>");
            imports.push(name.to_string());
        }
    }

    Ok(ScopeResult {
        file: file_path,
        line,
        enclosing_scope,
        local_variables,
        parameters,
        imports,
    })
}

// =============================================================================
// PHASE 10: STATISTICS
// =============================================================================

/// Compute codebase statistics
async fn compute_stats_cmd(project: &Project, filter: &TraceFilter) -> Result<StatsResult> {
    debug!("compute_stats");

    let index = load_semantic_index(project)?;
    let stats = index.stats();

    // Helper to check if a file passes the filter
    let file_passes = |file_id: u16| -> bool {
        if let Some(path) = index.file_path(file_id) {
            filter.matches_path(&path.to_string_lossy())
        } else {
            false
        }
    };

    // Helper to check if a symbol passes the filter
    let symbol_passes = |symbol: &crate::trace::Symbol| -> bool {
        if let Some(path) = index.file_path(symbol.file_id) {
            let name = index.symbol_name(symbol).unwrap_or("");
            let kind = symbol_kind_str(symbol.symbol_kind());
            filter.matches_symbol(name, kind, &path.to_string_lossy())
        } else {
            false
        }
    };

    // Count symbols by kind (filtered)
    let mut symbols_by_kind: HashMap<String, usize> = HashMap::new();
    let mut filtered_symbol_count = 0;
    for symbol in &index.symbols {
        if symbol_passes(symbol) {
            filtered_symbol_count += 1;
            *symbols_by_kind
                .entry(symbol_kind_str(symbol.symbol_kind()).to_string())
                .or_insert(0) += 1;
        }
    }

    // Count files by extension (filtered)
    let mut files_by_extension: HashMap<String, usize> = HashMap::new();
    let mut filtered_file_count = 0;
    for (idx, file) in index.files.iter().enumerate() {
        if file_passes(idx as u16) {
            filtered_file_count += 1;
            let ext = file
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            *files_by_extension.entry(ext).or_insert(0) += 1;
        }
    }

    // Find most referenced symbols (aggregated by name, filtered)
    let mut symbol_ref_counts: HashMap<String, usize> = HashMap::new();
    for s in &index.symbols {
        if symbol_passes(s) {
            if let Some(name) = index.symbol_name(s) {
                let ref_count = index.references_to(s.id).count();
                if ref_count > 0 {
                    *symbol_ref_counts.entry(name.to_string()).or_insert(0) += ref_count;
                }
            }
        }
    }
    let mut sorted_refs: Vec<_> = symbol_ref_counts.into_iter().collect();
    sorted_refs.sort_by(|a, b| b.1.cmp(&a.1));
    let most_referenced: Vec<_> = sorted_refs.into_iter().take(10).collect();

    // Find largest files (by symbol count, filtered)
    let mut file_symbol_counts: HashMap<u16, usize> = HashMap::new();
    for symbol in &index.symbols {
        if file_passes(symbol.file_id) {
            *file_symbol_counts.entry(symbol.file_id).or_insert(0) += 1;
        }
    }
    let mut largest_files: Vec<_> = file_symbol_counts
        .into_iter()
        .filter_map(|(file_id, count)| {
            let path = index.file_path(file_id)?;
            Some((path.to_string_lossy().to_string(), count))
        })
        .collect();
    largest_files.sort_by(|a, b| b.1.cmp(&a.1));
    largest_files.truncate(10);

    // Use filtered counts if filter is active, otherwise use global stats
    let (total_files, total_symbols) =
        if filter.path.is_some() || filter.symbol_type.is_some() || filter.name_pattern.is_some() {
            (filtered_file_count, filtered_symbol_count)
        } else {
            (stats.files, stats.symbols)
        };

    // Calculate call graph stats
    let max_call_depth = calculate_max_call_depth(&index);
    let avg_call_depth = calculate_avg_call_depth(&index);

    Ok(StatsResult {
        total_files,
        total_symbols,
        total_tokens: stats.tokens, // Not filtered (token-level filtering is expensive)
        total_references: stats.references, // Not filtered
        total_edges: stats.edges,   // Not filtered
        total_entry_points: stats.entry_points,
        symbols_by_kind,
        files_by_extension,
        most_referenced,
        largest_files,
        max_call_depth,
        avg_call_depth,
    })
}

fn calculate_max_call_depth(index: &SemanticIndex) -> usize {
    let mut max_depth = 0;

    for &entry_id in &index.entry_points {
        let depth = calculate_depth_from(index, entry_id, &mut HashSet::new());
        max_depth = max_depth.max(depth);
    }

    max_depth
}

fn calculate_depth_from(index: &SemanticIndex, sym_id: u32, visited: &mut HashSet<u32>) -> usize {
    if visited.contains(&sym_id) {
        return 0;
    }
    visited.insert(sym_id);

    let callees = index.callees(sym_id);
    if callees.is_empty() {
        return 0;
    }

    let max_child_depth = callees
        .iter()
        .map(|&callee| calculate_depth_from(index, callee, visited))
        .max()
        .unwrap_or(0);

    max_child_depth + 1
}

fn calculate_avg_call_depth(index: &SemanticIndex) -> f32 {
    if index.entry_points.is_empty() {
        return 0.0;
    }

    let total_depth: usize = index
        .entry_points
        .iter()
        .map(|&id| calculate_depth_from(index, id, &mut HashSet::new()))
        .sum();

    total_depth as f32 / index.entry_points.len() as f32
}

// =============================================================================
// TRACE OPERATIONS
// =============================================================================

/// Trace symbol invocation paths
async fn trace_symbol_cmd(
    project: &Project,
    symbol: &str,
    max_depth: usize,
    direct: bool,
    filter: &TraceFilter,
) -> Result<TraceResult> {
    debug!(symbol = %symbol, max_depth, direct, ?filter, "trace_symbol");

    let index = load_semantic_index(project)?;

    // Determine symbols to search for
    let symbols_to_search = if direct {
        vec![symbol.to_string()]
    } else {
        expand_query_with_ai(symbol).await
    };

    debug!(symbols = ?symbols_to_search, "Searching for symbols");

    // Find and trace all matching symbols
    let mut all_trace_results = Vec::new();
    for sym_name in &symbols_to_search {
        let trace_results = trace_symbol_by_name(&index, sym_name, Some(max_depth));
        all_trace_results.extend(trace_results);
    }

    if all_trace_results.is_empty() {
        return Ok(TraceResult {
            symbol: symbol.to_string(),
            defined_at: None,
            kind: "unknown".to_string(),
            invocation_paths: Vec::new(),
            total_paths: 0,
            entry_points: 0,
        });
    }

    // Convert traverse results to output format
    let mut invocation_paths = Vec::new();
    let mut entry_points_set = std::collections::HashSet::new();

    for trace_result in &all_trace_results {
        for path in &trace_result.paths {
            let entry_symbol = index.symbol(path.entry_point);
            let entry_name = entry_symbol
                .and_then(|s| index.symbol_name(s))
                .unwrap_or("<unknown>");
            let entry_file = entry_symbol
                .and_then(|s| index.file_path(s.file_id))
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let entry_kind = entry_symbol
                .map(|s| symbol_kind_str(s.symbol_kind()))
                .unwrap_or("function");

            entry_points_set.insert(path.entry_point);

            let chain: Vec<ChainStep> = path
                .chain
                .iter()
                .enumerate()
                .filter_map(|(i, &sym_id)| {
                    let sym = index.symbol(sym_id)?;
                    let name = index.symbol_name(sym)?;
                    let file = index
                        .file_path(sym.file_id)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let line = if i > 0 {
                        path.call_lines
                            .get(i - 1)
                            .copied()
                            .unwrap_or(sym.start_line)
                    } else {
                        sym.start_line
                    };

                    Some(ChainStep {
                        symbol: name.to_string(),
                        file,
                        line,
                        column: None,
                        context: None,
                    })
                })
                .collect();

            if !chain.is_empty() {
                invocation_paths.push(InvocationPath {
                    entry_point: format!("{} ({})", entry_name, entry_file),
                    entry_kind: entry_kind.to_string(),
                    chain,
                });
            }
        }
    }

    // AI reranking when not in direct mode
    if !direct && invocation_paths.len() > 1 {
        invocation_paths = rerank_paths_with_ai(symbol, invocation_paths).await;
    }

    let first_target = all_trace_results.first().map(|r| r.target);
    let defined_at = first_target.and_then(|id| {
        let sym = index.symbol(id)?;
        let file = index.file_path(sym.file_id)?;
        Some(format!("{}:{}", file.display(), sym.start_line))
    });
    let kind = first_target
        .and_then(|id| index.symbol(id))
        .map(|s| symbol_kind_str(s.symbol_kind()).to_string())
        .unwrap_or_else(|| "function".to_string());

    Ok(TraceResult {
        symbol: symbol.to_string(),
        defined_at,
        kind,
        invocation_paths: invocation_paths.clone(),
        total_paths: invocation_paths.len(),
        entry_points: entry_points_set.len(),
    })
}

// =============================================================================
// AI ENHANCEMENT
// =============================================================================

/// Expand a query into related symbol names using AI
async fn expand_query_with_ai(query: &str) -> Vec<String> {
    let providers = auth::get_authenticated_providers();

    if providers.is_empty() {
        debug!("No AI provider authenticated, skipping query expansion");
        return vec![query.to_string()];
    }

    if !is_natural_language_query(query) && query.len() > 15 {
        return vec![query.to_string()];
    }

    let expanded = if providers.contains(&Provider::Anthropic) {
        match auth::get_anthropic_token() {
            Ok(token) => {
                let client = ClaudeClient::new(token);
                match client.expand_query(query).await {
                    Ok(symbols) => {
                        debug!(count = symbols.len(), "AI expanded query to symbols");
                        symbols
                    }
                    Err(e) => {
                        warn!("AI query expansion failed: {}", e);
                        vec![query.to_string()]
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get Anthropic token: {}", e);
                vec![query.to_string()]
            }
        }
    } else if providers.contains(&Provider::Google) {
        match auth::get_google_token() {
            Ok(token) => {
                let client = GeminiClient::new(token);
                match client.expand_query(query).await {
                    Ok(symbols) => {
                        debug!(count = symbols.len(), "AI expanded query to symbols");
                        symbols
                    }
                    Err(e) => {
                        warn!("AI query expansion failed: {}", e);
                        vec![query.to_string()]
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get Google token: {}", e);
                vec![query.to_string()]
            }
        }
    } else {
        vec![query.to_string()]
    };

    let mut result = expanded;
    if !result.iter().any(|s| s.eq_ignore_ascii_case(query)) {
        result.insert(0, query.to_string());
    }
    result.truncate(10);
    result
}

/// Rerank invocation paths by relevance using AI
async fn rerank_paths_with_ai(query: &str, mut paths: Vec<InvocationPath>) -> Vec<InvocationPath> {
    let providers = auth::get_authenticated_providers();

    if providers.is_empty() {
        debug!("No AI provider authenticated, skipping reranking");
        return paths;
    }

    if paths.len() <= 3 {
        return paths;
    }

    let path_descriptions: Vec<String> = paths
        .iter()
        .map(|p| {
            let chain_str: Vec<String> = p.chain.iter().map(|c| c.symbol.clone()).collect();
            format!(
                "Entry: {} ({})\nChain: {}",
                p.entry_point,
                p.entry_kind,
                chain_str.join(" -> ")
            )
        })
        .collect();

    let indices = if providers.contains(&Provider::Anthropic) {
        match auth::get_anthropic_token() {
            Ok(token) => {
                let client = ClaudeClient::new(token);
                match client.rerank_trace(query, &path_descriptions).await {
                    Ok(idx) => {
                        debug!(order = ?idx, "AI reranked trace paths");
                        idx
                    }
                    Err(e) => {
                        warn!("AI reranking failed: {}", e);
                        (0..paths.len()).collect()
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get Anthropic token: {}", e);
                (0..paths.len()).collect()
            }
        }
    } else if providers.contains(&Provider::Google) {
        match auth::get_google_token() {
            Ok(token) => {
                let client = GeminiClient::new(token);
                match client.rerank_trace(query, &path_descriptions).await {
                    Ok(idx) => {
                        debug!(order = ?idx, "AI reranked trace paths");
                        idx
                    }
                    Err(e) => {
                        warn!("AI reranking failed: {}", e);
                        (0..paths.len()).collect()
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get Google token: {}", e);
                (0..paths.len()).collect()
            }
        }
    } else {
        (0..paths.len()).collect()
    };

    let original_paths = std::mem::take(&mut paths);
    let mut reranked = Vec::with_capacity(original_paths.len());

    for &idx in &indices {
        if idx < original_paths.len() {
            reranked.push(original_paths[idx].clone());
        }
    }

    for (i, path) in original_paths.into_iter().enumerate() {
        if !indices.contains(&i) {
            reranked.push(path);
        }
    }

    reranked
}

// =============================================================================
// DEAD CODE
// =============================================================================

/// Find dead/unused code
async fn find_dead_code_cmd(
    project: &Project,
    limit: Option<usize>,
    filter: &TraceFilter,
    xref: bool,
) -> Result<DeadCodeResult> {
    debug!("find_dead_code filter={:?} xref={}", filter, xref);

    let index = load_semantic_index(project)?;

    let dead_symbols = find_dead_symbols(&index);

    let mut symbols = Vec::new();
    let mut by_kind: HashMap<String, usize> = HashMap::new();
    let mut by_file: HashMap<String, usize> = HashMap::new();

    for sym in dead_symbols {
        let file = index
            .file_path(sym.file_id)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());

        let name = index.symbol_name(sym).unwrap_or("<unknown>").to_string();
        let kind = symbol_kind_str(sym.symbol_kind()).to_string();

        // Apply universal filter
        if !filter.matches_symbol(&name, &kind, &file) {
            continue;
        }

        *by_kind.entry(kind.clone()).or_insert(0) += 1;
        *by_file.entry(file.clone()).or_insert(0) += 1;

        // Cross-reference: find potential callers if enabled
        let potential_callers = if xref {
            find_potential_callers(&index, sym, &name)
        } else {
            Vec::new()
        };

        symbols.push(DeadSymbol {
            name,
            kind,
            file,
            line: sym.start_line,
            reason: "No references or calls found".to_string(),
            potential_callers,
        });
    }

    // Sort by file and line
    symbols.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

    // Apply limit
    if let Some(limit) = limit {
        symbols.truncate(limit);
    }

    Ok(DeadCodeResult {
        total_dead: symbols.len(),
        symbols,
        by_kind,
        by_file,
    })
}

/// Find potential callers for a dead symbol (for cross-referencing)
fn find_potential_callers(
    index: &SemanticIndex,
    dead_sym: &crate::trace::Symbol,
    dead_name: &str,
) -> Vec<PotentialCaller> {
    let mut callers = Vec::new();

    // Strategy 1: Find functions in the same file that could call this
    let same_file_symbols: Vec<_> = index
        .symbols
        .iter()
        .filter(|s| {
            s.file_id == dead_sym.file_id
                && s.id != dead_sym.id
                && matches!(s.symbol_kind(), SymbolKind::Function | SymbolKind::Method)
        })
        .collect();

    for caller in same_file_symbols.iter().take(3) {
        if let Some(caller_name) = index.symbol_name(caller) {
            if let Some(path) = index.file_path(caller.file_id) {
                callers.push(PotentialCaller {
                    name: caller_name.to_string(),
                    file: path.to_string_lossy().to_string(),
                    line: caller.start_line,
                    reason: "Same file - could call this".to_string(),
                });
            }
        }
    }

    // Strategy 2: Find entry points that could use this
    for &entry_id in index.entry_points.iter().take(2) {
        if let Some(entry_sym) = index.symbols.iter().find(|s| s.id == entry_id) {
            if entry_sym.id != dead_sym.id {
                if let Some(entry_name) = index.symbol_name(entry_sym) {
                    if let Some(path) = index.file_path(entry_sym.file_id) {
                        callers.push(PotentialCaller {
                            name: entry_name.to_string(),
                            file: path.to_string_lossy().to_string(),
                            line: entry_sym.start_line,
                            reason: "Entry point - could reach this".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Strategy 3: Find token matches (name appears but not as actual reference)
    if let Some(token_ids) = index.tokens_by_name(dead_name) {
        for &token_id in token_ids.iter().take(2) {
            if let Some(token) = index.token(token_id) {
                // Skip if it's at the definition location
                if token.file_id != dead_sym.file_id || token.line != dead_sym.start_line {
                    if let Some(path) = index.file_path(token.file_id) {
                        callers.push(PotentialCaller {
                            name: dead_name.to_string(),
                            file: path.to_string_lossy().to_string(),
                            line: token.line,
                            reason: "Token match - name appears here".to_string(),
                        });
                    }
                }
            }
        }
    }

    callers
}

/// Run interactive TUI mode
async fn run_tui(_args: &TraceArgs, _project: &Project) -> Result<()> {
    eprintln!("TUI mode is not yet implemented.");
    eprintln!("Use --json or default ASCII output instead.");
    Err(Error::SearchError {
        message: "TUI mode not implemented".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_output_format() {
        let args = TraceArgs {
            symbol: Some("test".to_string()),
            direct: false,
            refs: None,
            reads: None,
            writes: None,
            callers: None,
            callees: None,
            type_name: None,
            module: None,
            pattern: None,
            flow: None,
            impact: None,
            scope: None,
            dead: false,
            stats: false,
            cycles: false,
            kind: None,
            r#in: None,
            symbol_type: None,
            name: None,
            group_by: None,
            json: true,
            plain: false,
            csv: false,
            dot: false,
            markdown: false,
            tui: false,
            max_depth: 10,
            context: 0,
            limit: None,
            count: false,
            summary: false,
            xref: false,
            project: None,
        };
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn test_args_operations() {
        let args = TraceArgs {
            symbol: Some("test".to_string()),
            direct: false,
            refs: None,
            reads: None,
            writes: None,
            callers: None,
            callees: None,
            type_name: None,
            module: None,
            pattern: None,
            flow: None,
            impact: None,
            scope: None,
            dead: false,
            stats: false,
            cycles: false,
            kind: None,
            r#in: None,
            symbol_type: None,
            name: None,
            group_by: None,
            json: false,
            plain: false,
            csv: false,
            dot: false,
            markdown: false,
            tui: false,
            max_depth: 10,
            context: 0,
            limit: None,
            count: false,
            summary: false,
            xref: false,
            project: None,
        };

        let ops = args.operations();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TraceOperation::Trace(sym) => assert_eq!(sym, "test"),
            _ => panic!("Expected Trace operation"),
        }
    }

    #[test]
    fn test_args_refs_operations() {
        let args = TraceArgs {
            symbol: None,
            direct: false,
            refs: Some("userId".to_string()),
            reads: None,
            writes: None,
            callers: None,
            callees: None,
            type_name: None,
            module: None,
            pattern: None,
            flow: None,
            impact: None,
            scope: None,
            dead: false,
            stats: false,
            cycles: false,
            kind: None,
            r#in: None,
            symbol_type: None,
            name: None,
            group_by: None,
            json: false,
            plain: false,
            csv: false,
            dot: false,
            markdown: false,
            tui: false,
            max_depth: 10,
            context: 0,
            limit: None,
            count: false,
            summary: false,
            xref: false,
            project: None,
        };

        let ops = args.operations();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TraceOperation::Refs { symbol, kind } => {
                assert_eq!(symbol, "userId");
                assert!(kind.is_none());
            }
            _ => panic!("Expected Refs operation"),
        }
    }

    #[test]
    fn test_args_composable_operations() {
        // Test that multiple flags result in multiple operations
        let args = TraceArgs {
            symbol: Some("test".to_string()),
            direct: false,
            refs: Some("other".to_string()),
            reads: None,
            writes: None,
            callers: None,
            callees: None,
            type_name: None,
            module: None,
            pattern: None,
            flow: None,
            impact: None,
            scope: None,
            dead: true,
            stats: true,
            cycles: false,
            kind: None,
            r#in: None,
            symbol_type: None,
            name: None,
            group_by: None,
            json: false,
            plain: false,
            csv: false,
            dot: false,
            markdown: false,
            tui: false,
            max_depth: 10,
            context: 0,
            limit: None,
            count: false,
            summary: false,
            xref: false,
            project: None,
        };

        let ops = args.operations();
        // Should have: DeadCode, Stats, Refs, Trace (4 operations)
        assert_eq!(ops.len(), 4, "Expected 4 operations, got {:?}", ops);

        // Verify all expected operations are present
        let has_dead = ops.iter().any(|op| matches!(op, TraceOperation::DeadCode));
        let has_stats = ops.iter().any(|op| matches!(op, TraceOperation::Stats));
        let has_refs = ops
            .iter()
            .any(|op| matches!(op, TraceOperation::Refs { .. }));
        let has_trace = ops.iter().any(|op| matches!(op, TraceOperation::Trace(_)));

        assert!(has_dead, "Missing DeadCode operation");
        assert!(has_stats, "Missing Stats operation");
        assert!(has_refs, "Missing Refs operation");
        assert!(has_trace, "Missing Trace operation");
    }
}

#[allow(dead_code)]
pub fn debug_index_stats(project: &Project) -> Result<()> {
    let index = load_semantic_index(project)?;

    println!("=== INDEX DEBUG ===");
    println!("Symbols: {}", index.symbols.len());
    println!("Tokens: {}", index.tokens.len());
    println!("symbol_by_name entries: {}", index.symbol_by_name.len());
    println!("token_by_name entries: {}", index.token_by_name.len());

    println!("\nSample token names:");
    for (name, ids) in index.token_by_name.iter().take(10) {
        println!("  '{}' -> {} occurrences", name, ids.len());
    }

    if let Some(ids) = index.tokens_by_name("userId") {
        println!("\n'userId' found: {} occurrences", ids.len());
    } else {
        println!("\n'userId' NOT FOUND in token_by_name");
        let matches = index.tokens_matching("userId");
        println!("Tokens containing 'userId': {}", matches.len());
    }

    Ok(())
}
