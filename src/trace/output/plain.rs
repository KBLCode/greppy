//! Plain text output formatters
//!
//! Provides simple text output without ANSI codes for:
//! - Piping to other tools
//! - Log files
//! - Environments without color support
//!
//! Also includes CSV, DOT, and Markdown formatters.
//!
//! @module trace/output/plain

use super::{
    DeadCodeResult, FlowResult, ImpactResult, ModuleResult, PatternResult, RefsResult, ScopeResult,
    StatsResult, TraceFormatter, TraceResult,
};

// =============================================================================
// PLAIN TEXT FORMATTER
// =============================================================================

/// Plain text formatter (no ANSI codes)
pub struct PlainFormatter;

impl PlainFormatter {
    /// Create a new plain text formatter
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlainFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceFormatter for PlainFormatter {
    fn format_trace(&self, result: &TraceResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("TRACE: {}\n", result.symbol));
        if let Some(ref defined_at) = result.defined_at {
            output.push_str(&format!("Defined: {}\n", defined_at));
        }
        output.push_str(&format!(
            "Found: {} invocation paths from {} entry points\n",
            result.total_paths, result.entry_points
        ));
        output.push_str(&"-".repeat(60));
        output.push('\n');

        for (i, path) in result.invocation_paths.iter().enumerate() {
            output.push_str(&format!(
                "\nPath {}/{} (entry: {})\n",
                i + 1,
                result.total_paths,
                path.entry_point
            ));

            for (j, step) in path.chain.iter().enumerate() {
                let prefix = if j == path.chain.len() - 1 {
                    "  -> "
                } else {
                    "     "
                };
                output.push_str(&format!(
                    "{}{}:{} - {}\n",
                    prefix, step.file, step.line, step.symbol
                ));

                // Show context if available
                if let Some(ref ctx) = step.context {
                    for line in ctx.lines() {
                        output.push_str(&format!("       {}\n", line));
                    }
                }
            }
        }

        output
    }

    fn format_refs(&self, result: &RefsResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("REFS: {}\n", result.symbol));
        if let Some(ref defined_at) = result.defined_at {
            output.push_str(&format!("Defined: {}\n", defined_at));
        }
        output.push_str(&format!("Found: {} references\n", result.total_refs));

        if !result.by_kind.is_empty() {
            output.push_str("By kind: ");
            let kinds: Vec<_> = result
                .by_kind
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            output.push_str(&kinds.join(", "));
            output.push('\n');
        }
        output.push_str(&"-".repeat(60));
        output.push('\n');

        let mut by_file: std::collections::HashMap<&str, Vec<_>> = std::collections::HashMap::new();
        for r in &result.references {
            by_file.entry(&r.file).or_default().push(r);
        }

        for (file, refs) in by_file {
            output.push_str(&format!("\n{}:\n", file));
            for r in refs {
                // Handle multi-line context
                let context_lines: Vec<&str> = r.context.lines().collect();
                if context_lines.len() > 1 {
                    output.push_str(&format!("  {}:{} [{}]\n", r.line, r.column, r.kind));
                    for line in &context_lines {
                        output.push_str(&format!("    {}\n", line));
                    }
                } else {
                    output.push_str(&format!(
                        "  {}:{} [{}] {}",
                        r.line,
                        r.column,
                        r.kind,
                        r.context.trim()
                    ));
                    if let Some(ref enclosing) = r.enclosing_symbol {
                        output.push_str(&format!(" (in {})", enclosing));
                    }
                    output.push('\n');
                }
            }
        }

        output
    }

    fn format_dead_code(&self, result: &DeadCodeResult) -> String {
        let mut output = String::new();

        output.push_str("DEAD CODE ANALYSIS\n");
        output.push_str(&format!("Found: {} unused symbols\n", result.total_dead));

        if !result.by_kind.is_empty() {
            output.push_str("By kind: ");
            let kinds: Vec<_> = result
                .by_kind
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            output.push_str(&kinds.join(", "));
            output.push('\n');
        }
        output.push_str(&"-".repeat(60));
        output.push('\n');

        for sym in &result.symbols {
            output.push_str(&format!(
                "{}  {}:{}  {} - {}\n",
                sym.kind, sym.file, sym.line, sym.name, sym.reason
            ));
        }

        output
    }

    fn format_flow(&self, result: &FlowResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("DATA FLOW: {}\n", result.symbol));
        output.push_str(&format!("Paths: {}\n", result.flow_paths.len()));
        output.push_str(&"-".repeat(60));
        output.push('\n');

        for (i, path) in result.flow_paths.iter().enumerate() {
            output.push_str(&format!("\nFlow Path {}:\n", i + 1));

            for step in path {
                output.push_str(&format!(
                    "  {}:{} [{}] {}\n",
                    step.file,
                    step.line,
                    step.action,
                    step.expression.trim()
                ));
            }
        }

        output
    }

    fn format_impact(&self, result: &ImpactResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("IMPACT ANALYSIS: {}\n", result.symbol));
        output.push_str(&format!("File: {}\n", result.file));
        output.push_str(&format!("Risk Level: {}\n", result.risk_level));
        output.push_str(&"-".repeat(60));
        output.push('\n');

        output.push_str(&format!(
            "\nDirect callers ({}):\n",
            result.direct_caller_count
        ));
        for caller in &result.direct_callers {
            output.push_str(&format!("  {}\n", caller));
        }

        if !result.transitive_callers.is_empty() {
            output.push_str(&format!(
                "\nTransitive callers ({}):\n",
                result.transitive_caller_count
            ));
            for caller in &result.transitive_callers {
                output.push_str(&format!("  {}\n", caller));
            }
        }

        output.push_str(&format!(
            "\nAffected entry points ({}):\n",
            result.affected_entry_points.len()
        ));
        for ep in &result.affected_entry_points {
            output.push_str(&format!("  {}\n", ep));
        }

        output.push_str(&format!(
            "\nFiles affected: {}\n",
            result.files_affected.len()
        ));

        output
    }

    fn format_module(&self, result: &ModuleResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("MODULE: {}\n", result.module));
        output.push_str(&format!("Path: {}\n", result.file_path));
        output.push_str(&"-".repeat(60));
        output.push('\n');

        if !result.exports.is_empty() {
            output.push_str(&format!("\nExports ({}):\n", result.exports.len()));
            for export in &result.exports {
                output.push_str(&format!("  {}\n", export));
            }
        }

        if !result.imported_by.is_empty() {
            output.push_str(&format!("\nImported by ({}):\n", result.imported_by.len()));
            for importer in &result.imported_by {
                output.push_str(&format!("  {}\n", importer));
            }
        }

        if !result.dependencies.is_empty() {
            output.push_str(&format!(
                "\nDependencies ({}):\n",
                result.dependencies.len()
            ));
            for dep in &result.dependencies {
                output.push_str(&format!("  {}\n", dep));
            }
        }

        if !result.circular_deps.is_empty() {
            output.push_str(&format!(
                "\nCIRCULAR DEPENDENCIES ({}):\n",
                result.circular_deps.len()
            ));
            for cycle in &result.circular_deps {
                output.push_str(&format!("  WARNING: {}\n", cycle));
            }
        }

        output
    }

    fn format_pattern(&self, result: &PatternResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("PATTERN: {}\n", result.pattern));
        output.push_str(&format!(
            "Found: {} matches in {} files\n",
            result.total_matches,
            result.by_file.len()
        ));
        output.push_str(&"-".repeat(60));
        output.push('\n');

        let mut by_file: std::collections::HashMap<&str, Vec<_>> = std::collections::HashMap::new();
        for m in &result.matches {
            by_file.entry(&m.file).or_default().push(m);
        }

        for (file, matches) in by_file {
            output.push_str(&format!("\n{}:\n", file));
            for m in matches {
                // Handle multi-line context
                let context_lines: Vec<&str> = m.context.lines().collect();
                if context_lines.len() > 1 {
                    output.push_str(&format!("  {}:{}:\n", m.line, m.column));
                    for line in &context_lines {
                        output.push_str(&format!("    {}\n", line));
                    }
                } else {
                    output.push_str(&format!(
                        "  {}:{}: {}\n",
                        m.line,
                        m.column,
                        m.context.trim()
                    ));
                }
            }
        }

        output
    }

    fn format_scope(&self, result: &ScopeResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("SCOPE AT: {}:{}\n", result.file, result.line));
        if let Some(ref scope) = result.enclosing_scope {
            output.push_str(&format!("Enclosing: {}\n", scope));
        }
        output.push_str(&"-".repeat(60));
        output.push('\n');

        if !result.local_variables.is_empty() {
            output.push_str(&format!(
                "\nLocal Variables ({}):\n",
                result.local_variables.len()
            ));
            for var in &result.local_variables {
                output.push_str(&format!(
                    "  {}: {} (line {})\n",
                    var.name, var.kind, var.defined_at
                ));
            }
        }

        if !result.parameters.is_empty() {
            output.push_str(&format!("\nParameters ({}):\n", result.parameters.len()));
            for param in &result.parameters {
                output.push_str(&format!("  {}: {}\n", param.name, param.kind));
            }
        }

        if !result.imports.is_empty() {
            output.push_str(&format!("\nImports ({}):\n", result.imports.len()));
            for import in &result.imports {
                output.push_str(&format!("  {}\n", import));
            }
        }

        output
    }

    fn format_stats(&self, result: &StatsResult) -> String {
        let mut output = String::new();

        output.push_str("CODEBASE STATISTICS\n");
        output.push_str(&"-".repeat(60));
        output.push('\n');

        output.push_str(&format!("\nFiles:        {}\n", result.total_files));
        output.push_str(&format!("Symbols:      {}\n", result.total_symbols));
        output.push_str(&format!("Tokens:       {}\n", result.total_tokens));
        output.push_str(&format!("References:   {}\n", result.total_references));
        output.push_str(&format!("Call Edges:   {}\n", result.total_edges));
        output.push_str(&format!("Entry Points: {}\n", result.total_entry_points));

        output.push_str("\nSymbols by kind:\n");
        let mut kinds: Vec<_> = result.symbols_by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in &kinds {
            output.push_str(&format!("  {}: {}\n", kind, count));
        }

        output.push_str("\nCall Graph:\n");
        output.push_str(&format!("  Max Depth: {}\n", result.max_call_depth));
        output.push_str(&format!("  Avg Depth: {:.1}\n", result.avg_call_depth));

        output
    }
}

// =============================================================================
// CSV FORMATTER
// =============================================================================

/// CSV formatter for spreadsheet export
pub struct CsvFormatter;

impl CsvFormatter {
    pub fn new() -> Self {
        Self
    }

    fn escape_csv(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }
}

impl Default for CsvFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceFormatter for CsvFormatter {
    fn format_trace(&self, result: &TraceResult) -> String {
        let mut output = String::from("path_num,entry_point,entry_kind,step,symbol,file,line\n");

        for (i, path) in result.invocation_paths.iter().enumerate() {
            for (j, step) in path.chain.iter().enumerate() {
                output.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    i + 1,
                    Self::escape_csv(&path.entry_point),
                    Self::escape_csv(&path.entry_kind),
                    j + 1,
                    Self::escape_csv(&step.symbol),
                    Self::escape_csv(&step.file),
                    step.line
                ));
            }
        }

        output
    }

    fn format_refs(&self, result: &RefsResult) -> String {
        let mut output = String::from("file,line,column,kind,context,enclosing_symbol\n");

        for r in &result.references {
            let context_single = r.context.lines().next().unwrap_or("").trim();
            output.push_str(&format!(
                "{},{},{},{},{},{}\n",
                Self::escape_csv(&r.file),
                r.line,
                r.column,
                r.kind,
                Self::escape_csv(context_single),
                Self::escape_csv(r.enclosing_symbol.as_deref().unwrap_or(""))
            ));
        }

        output
    }

    fn format_dead_code(&self, result: &DeadCodeResult) -> String {
        let mut output = String::from("name,kind,file,line,reason\n");

        for sym in &result.symbols {
            output.push_str(&format!(
                "{},{},{},{},{}\n",
                Self::escape_csv(&sym.name),
                Self::escape_csv(&sym.kind),
                Self::escape_csv(&sym.file),
                sym.line,
                Self::escape_csv(&sym.reason)
            ));
        }

        output
    }

    fn format_flow(&self, result: &FlowResult) -> String {
        let mut output = String::from("path,step,variable,action,file,line,expression\n");

        for (i, path) in result.flow_paths.iter().enumerate() {
            for (j, step) in path.iter().enumerate() {
                output.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    i + 1,
                    j + 1,
                    Self::escape_csv(&step.variable),
                    step.action,
                    Self::escape_csv(&step.file),
                    step.line,
                    Self::escape_csv(step.expression.trim())
                ));
            }
        }

        output
    }

    fn format_impact(&self, result: &ImpactResult) -> String {
        let mut output = String::from("type,value\n");

        output.push_str(&format!("symbol,{}\n", Self::escape_csv(&result.symbol)));
        output.push_str(&format!("file,{}\n", Self::escape_csv(&result.file)));
        output.push_str(&format!("risk_level,{}\n", result.risk_level));
        output.push_str(&format!(
            "direct_caller_count,{}\n",
            result.direct_caller_count
        ));
        output.push_str(&format!(
            "transitive_caller_count,{}\n",
            result.transitive_caller_count
        ));
        output.push_str(&format!(
            "affected_entry_points,{}\n",
            result.affected_entry_points.len()
        ));
        output.push_str(&format!("files_affected,{}\n", result.files_affected.len()));

        output
    }

    fn format_module(&self, result: &ModuleResult) -> String {
        let mut output = String::from("type,value\n");

        output.push_str(&format!("module,{}\n", Self::escape_csv(&result.module)));
        output.push_str(&format!("path,{}\n", Self::escape_csv(&result.file_path)));
        output.push_str(&format!("exports,{}\n", result.exports.len()));
        output.push_str(&format!("imported_by,{}\n", result.imported_by.len()));
        output.push_str(&format!("dependencies,{}\n", result.dependencies.len()));
        output.push_str(&format!("circular_deps,{}\n", result.circular_deps.len()));

        output
    }

    fn format_pattern(&self, result: &PatternResult) -> String {
        let mut output = String::from("file,line,column,matched_text,context\n");

        for m in &result.matches {
            let context_single = m.context.lines().next().unwrap_or("").trim();
            output.push_str(&format!(
                "{},{},{},{},{}\n",
                Self::escape_csv(&m.file),
                m.line,
                m.column,
                Self::escape_csv(&m.matched_text),
                Self::escape_csv(context_single)
            ));
        }

        output
    }

    fn format_scope(&self, result: &ScopeResult) -> String {
        let mut output = String::from("type,name,kind,defined_at\n");

        for var in &result.local_variables {
            output.push_str(&format!(
                "local,{},{},{}\n",
                Self::escape_csv(&var.name),
                Self::escape_csv(&var.kind),
                var.defined_at
            ));
        }

        for param in &result.parameters {
            output.push_str(&format!(
                "param,{},{},\n",
                Self::escape_csv(&param.name),
                Self::escape_csv(&param.kind)
            ));
        }

        for import in &result.imports {
            output.push_str(&format!("import,{},module,\n", Self::escape_csv(import)));
        }

        output
    }

    fn format_stats(&self, result: &StatsResult) -> String {
        let mut output = String::from("metric,value\n");

        output.push_str(&format!("total_files,{}\n", result.total_files));
        output.push_str(&format!("total_symbols,{}\n", result.total_symbols));
        output.push_str(&format!("total_tokens,{}\n", result.total_tokens));
        output.push_str(&format!("total_references,{}\n", result.total_references));
        output.push_str(&format!("total_edges,{}\n", result.total_edges));
        output.push_str(&format!(
            "total_entry_points,{}\n",
            result.total_entry_points
        ));
        output.push_str(&format!("max_call_depth,{}\n", result.max_call_depth));
        output.push_str(&format!("avg_call_depth,{:.2}\n", result.avg_call_depth));

        output
    }
}

// =============================================================================
// DOT FORMATTER (Graph Visualization)
// =============================================================================

/// DOT formatter for graph visualization
pub struct DotFormatter;

impl DotFormatter {
    pub fn new() -> Self {
        Self
    }

    fn escape_dot(s: &str) -> String {
        s.replace('"', "\\\"").replace('\n', "\\n")
    }
}

impl Default for DotFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceFormatter for DotFormatter {
    fn format_trace(&self, result: &TraceResult) -> String {
        let mut output = String::from("digraph trace {\n");
        output.push_str("  rankdir=LR;\n");
        output.push_str("  node [shape=box];\n\n");

        let mut nodes = std::collections::HashSet::new();
        let mut edges = Vec::new();

        for path in &result.invocation_paths {
            for (i, step) in path.chain.iter().enumerate() {
                let node_id = format!("{}_{}", step.symbol.replace(['.', '/'], "_"), step.line);
                if !nodes.contains(&node_id) {
                    nodes.insert(node_id.clone());
                    output.push_str(&format!(
                        "  {} [label=\"{}\\n{}:{}\"];\n",
                        node_id,
                        Self::escape_dot(&step.symbol),
                        Self::escape_dot(&step.file),
                        step.line
                    ));
                }

                if i > 0 {
                    let prev = &path.chain[i - 1];
                    let prev_id = format!("{}_{}", prev.symbol.replace(['.', '/'], "_"), prev.line);
                    let edge = (prev_id.clone(), node_id.clone());
                    if !edges.contains(&edge) {
                        edges.push(edge.clone());
                        output.push_str(&format!("  {} -> {};\n", edge.0, edge.1));
                    }
                }
            }
        }

        output.push_str("}\n");
        output
    }

    fn format_refs(&self, result: &RefsResult) -> String {
        let mut output = String::from("digraph refs {\n");
        output.push_str("  rankdir=TB;\n");
        output.push_str(&format!(
            "  center [label=\"{}\" shape=ellipse style=filled fillcolor=yellow];\n",
            Self::escape_dot(&result.symbol)
        ));

        for (i, r) in result.references.iter().enumerate() {
            let node_id = format!("ref_{}", i);
            let label = format!("{}:{}", r.file, r.line);
            output.push_str(&format!(
                "  {} [label=\"{}\" shape=box];\n",
                node_id,
                Self::escape_dot(&label)
            ));
            output.push_str(&format!(
                "  center -> {} [label=\"{}\"];\n",
                node_id, r.kind
            ));
        }

        output.push_str("}\n");
        output
    }

    fn format_dead_code(&self, result: &DeadCodeResult) -> String {
        let mut output = String::from("digraph dead_code {\n");
        output.push_str("  node [shape=box style=filled fillcolor=lightgray];\n");

        for (i, sym) in result.symbols.iter().enumerate() {
            output.push_str(&format!(
                "  dead_{} [label=\"{}\\n{}:{}\"];\n",
                i,
                Self::escape_dot(&sym.name),
                Self::escape_dot(&sym.file),
                sym.line
            ));
        }

        output.push_str("}\n");
        output
    }

    fn format_flow(&self, result: &FlowResult) -> String {
        let mut output = String::from("digraph flow {\n");
        output.push_str("  rankdir=TB;\n");

        for (path_idx, path) in result.flow_paths.iter().enumerate() {
            for (i, step) in path.iter().enumerate() {
                let node_id = format!("p{}_s{}", path_idx, i);
                output.push_str(&format!(
                    "  {} [label=\"[{}] {}\\n{}:{}\"];\n",
                    node_id,
                    step.action,
                    Self::escape_dot(&step.variable),
                    Self::escape_dot(&step.file),
                    step.line
                ));

                if i > 0 {
                    let prev_id = format!("p{}_s{}", path_idx, i - 1);
                    output.push_str(&format!("  {} -> {};\n", prev_id, node_id));
                }
            }
        }

        output.push_str("}\n");
        output
    }

    fn format_impact(&self, result: &ImpactResult) -> String {
        let mut output = String::from("digraph impact {\n");
        output.push_str("  rankdir=BT;\n");
        output.push_str(&format!(
            "  target [label=\"{}\" shape=ellipse style=filled fillcolor=red fontcolor=white];\n",
            Self::escape_dot(&result.symbol)
        ));

        for (i, caller) in result.direct_callers.iter().enumerate() {
            output.push_str(&format!(
                "  direct_{} [label=\"{}\"];\n",
                i,
                Self::escape_dot(caller)
            ));
            output.push_str(&format!("  direct_{} -> target [color=red];\n", i));
        }

        output.push_str("}\n");
        output
    }

    fn format_module(&self, result: &ModuleResult) -> String {
        let mut output = String::from("digraph module {\n");
        output.push_str("  rankdir=LR;\n");
        output.push_str(&format!(
            "  module [label=\"{}\" shape=box style=filled fillcolor=lightblue];\n",
            Self::escape_dot(&result.module)
        ));

        for (i, dep) in result.dependencies.iter().enumerate() {
            output.push_str(&format!(
                "  dep_{} [label=\"{}\"];\n",
                i,
                Self::escape_dot(dep)
            ));
            output.push_str(&format!("  module -> dep_{};\n", i));
        }

        for (i, importer) in result.imported_by.iter().enumerate() {
            output.push_str(&format!(
                "  importer_{} [label=\"{}\"];\n",
                i,
                Self::escape_dot(importer)
            ));
            output.push_str(&format!("  importer_{} -> module;\n", i));
        }

        output.push_str("}\n");
        output
    }

    fn format_pattern(&self, _result: &PatternResult) -> String {
        String::from("// Pattern results not suitable for DOT format\n")
    }

    fn format_scope(&self, _result: &ScopeResult) -> String {
        String::from("// Scope results not suitable for DOT format\n")
    }

    fn format_stats(&self, _result: &StatsResult) -> String {
        String::from("// Stats not suitable for DOT format\n")
    }
}

// =============================================================================
// MARKDOWN FORMATTER
// =============================================================================

/// Markdown formatter for documentation
pub struct MarkdownFormatter;

impl MarkdownFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceFormatter for MarkdownFormatter {
    fn format_trace(&self, result: &TraceResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("# Trace: {}\n\n", result.symbol));

        if let Some(ref defined_at) = result.defined_at {
            output.push_str(&format!("**Defined at:** `{}`\n\n", defined_at));
        }

        output.push_str(&format!(
            "**Found:** {} invocation paths from {} entry points\n\n",
            result.total_paths, result.entry_points
        ));

        for (i, path) in result.invocation_paths.iter().enumerate() {
            output.push_str(&format!("## Path {}/{}\n\n", i + 1, result.total_paths));
            output.push_str(&format!(
                "**Entry:** {} ({})\n\n",
                path.entry_point, path.entry_kind
            ));

            output.push_str("| Step | Symbol | Location |\n");
            output.push_str("|------|--------|----------|\n");

            for (j, step) in path.chain.iter().enumerate() {
                output.push_str(&format!(
                    "| {} | `{}` | `{}:{}` |\n",
                    j + 1,
                    step.symbol,
                    step.file,
                    step.line
                ));
            }
            output.push('\n');
        }

        output
    }

    fn format_refs(&self, result: &RefsResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("# References: {}\n\n", result.symbol));

        if let Some(ref defined_at) = result.defined_at {
            output.push_str(&format!("**Defined at:** `{}`\n\n", defined_at));
        }

        output.push_str(&format!("**Total:** {} references\n\n", result.total_refs));

        if !result.by_kind.is_empty() {
            output.push_str("### By Kind\n\n");
            for (kind, count) in &result.by_kind {
                output.push_str(&format!("- **{}:** {}\n", kind, count));
            }
            output.push('\n');
        }

        output.push_str("### References\n\n");
        output.push_str("| File | Line | Kind | Context |\n");
        output.push_str("|------|------|------|----------|\n");

        for r in &result.references {
            let context_short = r.context.lines().next().unwrap_or("").trim();
            let context_escaped = context_short.replace('|', "\\|");
            output.push_str(&format!(
                "| `{}` | {} | {} | `{}` |\n",
                r.file, r.line, r.kind, context_escaped
            ));
        }

        output
    }

    fn format_dead_code(&self, result: &DeadCodeResult) -> String {
        let mut output = String::new();

        output.push_str("# Dead Code Analysis\n\n");
        output.push_str(&format!(
            "**Found:** {} unused symbols\n\n",
            result.total_dead
        ));

        if !result.by_kind.is_empty() {
            output.push_str("### By Kind\n\n");
            for (kind, count) in &result.by_kind {
                output.push_str(&format!("- **{}:** {}\n", kind, count));
            }
            output.push('\n');
        }

        output.push_str("### Unused Symbols\n\n");
        output.push_str("| Name | Kind | File | Line |\n");
        output.push_str("|------|------|------|------|\n");

        for sym in &result.symbols {
            output.push_str(&format!(
                "| `{}` | {} | `{}` | {} |\n",
                sym.name, sym.kind, sym.file, sym.line
            ));
        }

        output
    }

    fn format_flow(&self, result: &FlowResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("# Data Flow: {}\n\n", result.symbol));

        for (i, path) in result.flow_paths.iter().enumerate() {
            output.push_str(&format!("## Flow Path {}\n\n", i + 1));

            for step in path {
                output.push_str(&format!(
                    "1. **[{}]** `{}:{}` - `{}`\n",
                    step.action,
                    step.file,
                    step.line,
                    step.expression.trim()
                ));
            }
            output.push('\n');
        }

        output
    }

    fn format_impact(&self, result: &ImpactResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("# Impact Analysis: {}\n\n", result.symbol));
        output.push_str(&format!("**File:** `{}`\n\n", result.file));
        output.push_str(&format!("**Risk Level:** {}\n\n", result.risk_level));

        output.push_str(&format!(
            "## Direct Callers ({})\n\n",
            result.direct_caller_count
        ));
        for caller in &result.direct_callers {
            output.push_str(&format!("- `{}`\n", caller));
        }
        output.push('\n');

        if !result.transitive_callers.is_empty() {
            output.push_str(&format!(
                "## Transitive Callers ({})\n\n",
                result.transitive_caller_count
            ));
            for caller in result.transitive_callers.iter().take(20) {
                output.push_str(&format!("- `{}`\n", caller));
            }
            output.push('\n');
        }

        output.push_str(&format!(
            "## Affected Entry Points ({})\n\n",
            result.affected_entry_points.len()
        ));
        for ep in &result.affected_entry_points {
            output.push_str(&format!("- `{}`\n", ep));
        }

        output
    }

    fn format_module(&self, result: &ModuleResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("# Module: {}\n\n", result.module));
        output.push_str(&format!("**Path:** `{}`\n\n", result.file_path));

        if !result.exports.is_empty() {
            output.push_str(&format!("## Exports ({})\n\n", result.exports.len()));
            for export in &result.exports {
                output.push_str(&format!("- `{}`\n", export));
            }
            output.push('\n');
        }

        if !result.imported_by.is_empty() {
            output.push_str(&format!(
                "## Imported By ({})\n\n",
                result.imported_by.len()
            ));
            for importer in &result.imported_by {
                output.push_str(&format!("- `{}`\n", importer));
            }
            output.push('\n');
        }

        if !result.dependencies.is_empty() {
            output.push_str(&format!(
                "## Dependencies ({})\n\n",
                result.dependencies.len()
            ));
            for dep in &result.dependencies {
                output.push_str(&format!("- `{}`\n", dep));
            }
            output.push('\n');
        }

        if !result.circular_deps.is_empty() {
            output.push_str(&format!(
                "## ⚠️ Circular Dependencies ({})\n\n",
                result.circular_deps.len()
            ));
            for cycle in &result.circular_deps {
                output.push_str(&format!("- `{}`\n", cycle));
            }
        }

        output
    }

    fn format_pattern(&self, result: &PatternResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("# Pattern: `{}`\n\n", result.pattern));
        output.push_str(&format!(
            "**Found:** {} matches in {} files\n\n",
            result.total_matches,
            result.by_file.len()
        ));

        output.push_str("## Matches\n\n");
        output.push_str("| File | Line | Match |\n");
        output.push_str("|------|------|-------|\n");

        for m in &result.matches {
            let match_escaped = m.matched_text.replace('|', "\\|");
            output.push_str(&format!(
                "| `{}` | {} | `{}` |\n",
                m.file, m.line, match_escaped
            ));
        }

        output
    }

    fn format_scope(&self, result: &ScopeResult) -> String {
        let mut output = String::new();

        output.push_str(&format!("# Scope at `{}:{}`\n\n", result.file, result.line));

        if let Some(ref scope) = result.enclosing_scope {
            output.push_str(&format!("**Enclosing:** `{}`\n\n", scope));
        }

        if !result.local_variables.is_empty() {
            output.push_str(&format!(
                "## Local Variables ({})\n\n",
                result.local_variables.len()
            ));
            output.push_str("| Name | Type | Defined At |\n");
            output.push_str("|------|------|------------|\n");
            for var in &result.local_variables {
                output.push_str(&format!(
                    "| `{}` | {} | line {} |\n",
                    var.name, var.kind, var.defined_at
                ));
            }
            output.push('\n');
        }

        if !result.imports.is_empty() {
            output.push_str(&format!("## Imports ({})\n\n", result.imports.len()));
            for import in &result.imports {
                output.push_str(&format!("- `{}`\n", import));
            }
        }

        output
    }

    fn format_stats(&self, result: &StatsResult) -> String {
        let mut output = String::new();

        output.push_str("# Codebase Statistics\n\n");

        output.push_str("## Overview\n\n");
        output.push_str("| Metric | Value |\n");
        output.push_str("|--------|-------|\n");
        output.push_str(&format!("| Files | {} |\n", result.total_files));
        output.push_str(&format!("| Symbols | {} |\n", result.total_symbols));
        output.push_str(&format!("| Tokens | {} |\n", result.total_tokens));
        output.push_str(&format!("| References | {} |\n", result.total_references));
        output.push_str(&format!("| Call Edges | {} |\n", result.total_edges));
        output.push_str(&format!(
            "| Entry Points | {} |\n",
            result.total_entry_points
        ));
        output.push('\n');

        output.push_str("## Symbols by Kind\n\n");
        output.push_str("| Kind | Count |\n");
        output.push_str("|------|-------|\n");
        let mut kinds: Vec<_> = result.symbols_by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in &kinds {
            output.push_str(&format!("| {} | {} |\n", kind, count));
        }
        output.push('\n');

        output.push_str("## Call Graph\n\n");
        output.push_str(&format!("- **Max Depth:** {}\n", result.max_call_depth));
        output.push_str(&format!("- **Avg Depth:** {:.1}\n", result.avg_call_depth));

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::output::{ChainStep, InvocationPath};

    #[test]
    fn test_format_trace_plain() {
        let formatter = PlainFormatter::new();
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
                        symbol: "validateUser".to_string(),
                        file: "validation.ts".to_string(),
                        line: 8,
                        column: Some(10),
                        context: None,
                    },
                ],
            }],
            total_paths: 1,
            entry_points: 1,
        };

        let output = formatter.format_trace(&result);
        assert!(output.contains("TRACE: validateUser"));
        assert!(output.contains("Defined: utils/validation.ts:8"));
        assert!(output.contains("POST /api/auth/login"));
        assert!(output.contains("auth.controller.ts:8"));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn test_format_refs_plain() {
        let formatter = PlainFormatter::new();
        let result = RefsResult {
            symbol: "userId".to_string(),
            defined_at: Some("types.ts:5".to_string()),
            symbol_kind: None,
            references: vec![],
            total_refs: 0,
            by_kind: std::collections::HashMap::new(),
            by_file: std::collections::HashMap::new(),
        };

        let output = formatter.format_refs(&result);
        assert!(output.contains("REFS: userId"));
        assert!(!output.contains("\x1b["));
    }
}
