//! ASCII output formatter with colors and box-drawing
//!
//! Provides rich terminal output with:
//! - Unicode box-drawing characters
//! - ANSI color codes
//! - Terminal width detection
//!
//! @module trace/output/ascii

use super::{
    DeadCodeResult, FlowResult, ImpactResult, ModuleResult, PatternResult, ReferenceKind,
    RefsResult, RiskLevel, ScopeResult, StatsResult, TraceFormatter, TraceResult,
};

// =============================================================================
// CONSTANTS
// =============================================================================

/// ANSI color codes
#[allow(dead_code)]
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";

    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
    pub const RED: &str = "\x1b[31m";

    pub const BG_RED: &str = "\x1b[41m";
    pub const BG_YELLOW: &str = "\x1b[43m";
}

/// Box-drawing characters
mod box_chars {
    pub const TOP_LEFT: char = '╔';
    pub const TOP_RIGHT: char = '╗';
    pub const BOTTOM_LEFT: char = '╚';
    pub const BOTTOM_RIGHT: char = '╝';
    pub const HORIZONTAL: char = '═';
    pub const VERTICAL: char = '║';
    pub const THIN_HORIZONTAL: char = '━';
    pub const ARROW_DOWN: &str = "│";
    pub const ARROW_RIGHT: &str = "→";
    pub const TARGET: &str = "←";
}

// =============================================================================
// FORMATTER IMPLEMENTATION
// =============================================================================

/// ASCII formatter with rich terminal output
pub struct AsciiFormatter {
    width: usize,
}

impl AsciiFormatter {
    /// Create a new ASCII formatter
    pub fn new() -> Self {
        Self {
            width: Self::detect_terminal_width(),
        }
    }

    /// Detect terminal width, defaulting to 80
    fn detect_terminal_width() -> usize {
        if let Ok(cols) = std::env::var("COLUMNS") {
            if let Ok(width) = cols.parse::<usize>() {
                return width.min(200).max(60);
            }
        }

        if let Ok(cols) = std::env::var("TERM_WIDTH") {
            if let Ok(width) = cols.parse::<usize>() {
                return width.min(200).max(60);
            }
        }

        80
    }

    /// Draw a header box
    fn draw_header_box<S: AsRef<str>>(&self, lines: &[S]) -> String {
        let inner_width = self.width - 4;
        let mut output = String::new();

        output.push(box_chars::TOP_LEFT);
        for _ in 0..inner_width + 2 {
            output.push(box_chars::HORIZONTAL);
        }
        output.push(box_chars::TOP_RIGHT);
        output.push('\n');

        for line in lines {
            output.push(box_chars::VERTICAL);
            output.push_str("  ");
            let display_line = self.truncate_or_pad(line.as_ref(), inner_width);
            output.push_str(&display_line);
            output.push_str("  ");
            output.push(box_chars::VERTICAL);
            output.push('\n');
        }

        output.push(box_chars::BOTTOM_LEFT);
        for _ in 0..inner_width + 2 {
            output.push(box_chars::HORIZONTAL);
        }
        output.push(box_chars::BOTTOM_RIGHT);
        output.push('\n');

        output
    }

    /// Draw a separator line
    fn draw_separator(&self, left_text: &str, right_text: &str) -> String {
        let inner_width = self.width - 2;
        let left_len = self.visible_len(left_text);
        let right_len = self.visible_len(right_text);
        let sep_len = inner_width.saturating_sub(left_len + right_len + 2);

        let mut output = String::new();
        for _ in 0..inner_width {
            output.push(box_chars::THIN_HORIZONTAL);
        }
        output.push('\n');
        output.push_str(left_text);
        for _ in 0..sep_len {
            output.push(' ');
        }
        output.push_str(right_text);
        output.push('\n');
        for _ in 0..inner_width {
            output.push(box_chars::THIN_HORIZONTAL);
        }
        output.push('\n');

        output
    }

    /// Truncate or pad a string to fit width
    fn truncate_or_pad(&self, s: &str, width: usize) -> String {
        let visible_len = self.visible_len(s);
        if visible_len > width {
            let mut result = String::new();
            let mut visible_count = 0;
            let mut chars = s.chars().peekable();

            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    result.push(c);
                    while let Some(&next) = chars.peek() {
                        result.push(chars.next().unwrap());
                        if next == 'm' {
                            break;
                        }
                    }
                } else {
                    if visible_count >= width - 3 {
                        result.push_str("...");
                        result.push_str(colors::RESET);
                        break;
                    }
                    result.push(c);
                    visible_count += 1;
                }
            }
            result
        } else {
            let padding = width - visible_len;
            format!("{}{}", s, " ".repeat(padding))
        }
    }

    /// Calculate visible length (excluding ANSI codes)
    fn visible_len(&self, s: &str) -> usize {
        let mut len = 0;
        let mut in_escape = false;

        for c in s.chars() {
            if c == '\x1b' {
                in_escape = true;
            } else if in_escape {
                if c == 'm' {
                    in_escape = false;
                }
            } else {
                len += 1;
            }
        }

        len
    }

    /// Color a string based on reference kind
    fn color_ref_kind(&self, kind: ReferenceKind) -> &'static str {
        match kind {
            ReferenceKind::Read => colors::CYAN,
            ReferenceKind::Write => colors::YELLOW,
            ReferenceKind::Call => colors::GREEN,
            ReferenceKind::TypeAnnotation => colors::MAGENTA,
            ReferenceKind::Import => colors::BLUE,
            ReferenceKind::Export => colors::BLUE,
        }
    }

    /// Color a string based on risk level
    fn color_risk(&self, risk: RiskLevel) -> &'static str {
        match risk {
            RiskLevel::Low => colors::GREEN,
            RiskLevel::Medium => colors::YELLOW,
            RiskLevel::High => colors::RED,
            RiskLevel::Critical => colors::BG_RED,
        }
    }
}

impl Default for AsciiFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceFormatter for AsciiFormatter {
    fn format_trace(&self, result: &TraceResult) -> String {
        let mut output = String::new();

        let defined_at = result.defined_at.as_deref().unwrap_or("unknown");
        let header_lines = [
            &format!(
                "{}{}TRACE:{} {}",
                colors::BOLD,
                colors::CYAN,
                colors::RESET,
                result.symbol
            ),
            &format!("{}Defined:{} {}", colors::DIM, colors::RESET, defined_at),
            &format!(
                "{}Found:{} {} invocation paths from {} entry points",
                colors::DIM,
                colors::RESET,
                result.total_paths,
                result.entry_points
            ),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        for (i, path) in result.invocation_paths.iter().enumerate() {
            let path_header = format!(
                "{}{}Path {}/{}{}",
                colors::BOLD,
                colors::WHITE,
                i + 1,
                result.total_paths,
                colors::RESET
            );
            let entry_info = format!("{}{}{}", colors::GREEN, path.entry_point, colors::RESET);
            output.push_str(&self.draw_separator(&path_header, &entry_info));
            output.push('\n');

            let max_file_width = path
                .chain
                .iter()
                .map(|s| s.file.len() + format!(":{}", s.line).len())
                .max()
                .unwrap_or(20);

            for (j, step) in path.chain.iter().enumerate() {
                let is_target = j == path.chain.len() - 1;
                let location = format!("{}:{}", step.file, step.line);
                let padding = max_file_width.saturating_sub(location.len()) + 2;

                if is_target {
                    output.push_str(&format!(
                        "  {}{:<width$}{}  {}  {}{}{}  {}{} TARGET{}",
                        colors::DIM,
                        location,
                        colors::RESET,
                        box_chars::ARROW_RIGHT,
                        colors::BOLD,
                        colors::GREEN,
                        step.symbol,
                        colors::YELLOW,
                        box_chars::TARGET,
                        colors::RESET,
                        width = max_file_width + padding
                    ));
                } else {
                    output.push_str(&format!(
                        "  {}{:<width$}{}  {}  {}{}{}",
                        colors::DIM,
                        location,
                        colors::RESET,
                        box_chars::ARROW_RIGHT,
                        colors::CYAN,
                        step.symbol,
                        colors::RESET,
                        width = max_file_width + padding
                    ));
                }
                output.push('\n');

                // Show context if available
                if let Some(ref ctx) = step.context {
                    for line in ctx.lines() {
                        output.push_str(&format!(
                            "      {}{}{}\n",
                            colors::DIM,
                            line,
                            colors::RESET
                        ));
                    }
                }

                if !is_target {
                    output.push_str(&format!(
                        "  {}{:<width$}{}  {}",
                        colors::DIM,
                        "",
                        colors::RESET,
                        box_chars::ARROW_DOWN,
                        width = max_file_width + padding
                    ));
                    output.push('\n');
                }
            }
            output.push('\n');
        }

        output
    }

    fn format_refs(&self, result: &RefsResult) -> String {
        let mut output = String::new();

        let defined_at = result.defined_at.as_deref().unwrap_or("unknown");
        let header_lines = [
            &format!(
                "{}{}REFS:{} {}",
                colors::BOLD,
                colors::CYAN,
                colors::RESET,
                result.symbol
            ),
            &format!("{}Defined:{} {}", colors::DIM, colors::RESET, defined_at),
            &format!(
                "{}Found:{} {} references",
                colors::DIM,
                colors::RESET,
                result.total_refs
            ),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        if !result.by_kind.is_empty() {
            output.push_str(&format!("{}By kind:{} ", colors::DIM, colors::RESET));
            let kinds: Vec<_> = result
                .by_kind
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            output.push_str(&kinds.join(", "));
            output.push_str("\n\n");
        }

        let mut by_file: std::collections::HashMap<&str, Vec<_>> = std::collections::HashMap::new();
        for r in &result.references {
            by_file.entry(&r.file).or_default().push(r);
        }

        for (file, refs) in by_file {
            output.push_str(&format!(
                "{}{}{}:{}\n",
                colors::BOLD,
                colors::WHITE,
                file,
                colors::RESET
            ));

            for r in refs {
                let kind_color = self.color_ref_kind(r.kind);
                output.push_str(&format!(
                    "  {}:{:<4}  {}{:<6}{}  ",
                    r.line,
                    r.column,
                    kind_color,
                    r.kind,
                    colors::RESET,
                ));

                // Handle multi-line context
                let context_lines: Vec<&str> = r.context.lines().collect();
                if context_lines.len() > 1 {
                    output.push('\n');
                    for line in &context_lines {
                        output.push_str(&format!("      {}\n", line));
                    }
                } else {
                    output.push_str(r.context.trim());
                    output.push('\n');
                }

                if let Some(ref enclosing) = r.enclosing_symbol {
                    output.push_str(&format!(
                        "      {}(in {}){}",
                        colors::DIM,
                        enclosing,
                        colors::RESET
                    ));
                    output.push('\n');
                }
            }
            output.push('\n');
        }

        output
    }

    fn format_dead_code(&self, result: &DeadCodeResult) -> String {
        let mut output = String::new();

        let header_lines = [
            &format!(
                "{}{}DEAD CODE ANALYSIS{}",
                colors::BOLD,
                colors::YELLOW,
                colors::RESET
            ),
            &format!(
                "{}Found:{} {} unused symbols",
                colors::DIM,
                colors::RESET,
                result.total_dead
            ),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        if !result.by_kind.is_empty() {
            output.push_str(&format!("{}By kind:{} ", colors::DIM, colors::RESET));
            let kinds: Vec<_> = result
                .by_kind
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            output.push_str(&kinds.join(", "));
            output.push_str("\n\n");
        }

        for sym in &result.symbols {
            output.push_str(&format!(
                "  {}{}{}  {}{}:{}{}  {}\n",
                colors::YELLOW,
                sym.name,
                colors::RESET,
                colors::DIM,
                sym.file,
                sym.line,
                colors::RESET,
                sym.reason
            ));

            // Show potential callers if cross-referencing is enabled
            if !sym.potential_callers.is_empty() {
                output.push_str(&format!(
                    "      {}Potential callers:{}\n",
                    colors::DIM,
                    colors::RESET
                ));
                for caller in &sym.potential_callers {
                    output.push_str(&format!(
                        "        {}→{} {}  {}{}:{}{}  {}{}{}\n",
                        colors::GREEN,
                        colors::RESET,
                        caller.name,
                        colors::DIM,
                        caller.file,
                        caller.line,
                        colors::RESET,
                        colors::DIM,
                        caller.reason,
                        colors::RESET
                    ));
                }
            }
        }

        output
    }

    fn format_flow(&self, result: &FlowResult) -> String {
        let mut output = String::new();

        let header_lines = [
            &format!(
                "{}{}DATA FLOW:{} {}",
                colors::BOLD,
                colors::MAGENTA,
                colors::RESET,
                result.symbol
            ),
            &format!(
                "{}Paths:{} {}",
                colors::DIM,
                colors::RESET,
                result.flow_paths.len()
            ),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        for (i, path) in result.flow_paths.iter().enumerate() {
            output.push_str(&format!(
                "{}{}Flow Path {}{}:\n",
                colors::BOLD,
                colors::WHITE,
                i + 1,
                colors::RESET
            ));

            for step in path {
                let action_color = match step.action {
                    super::FlowAction::Define | super::FlowAction::Assign => colors::GREEN,
                    super::FlowAction::Read => colors::CYAN,
                    super::FlowAction::PassToFunction => colors::YELLOW,
                    super::FlowAction::ReturnFrom => colors::MAGENTA,
                    super::FlowAction::Mutate => colors::RED,
                };

                output.push_str(&format!(
                    "  {}:{:<4}  {}{:<8}{}  {}\n",
                    step.file,
                    step.line,
                    action_color,
                    step.action,
                    colors::RESET,
                    step.expression.trim()
                ));
            }
            output.push('\n');
        }

        output
    }

    fn format_impact(&self, result: &ImpactResult) -> String {
        let mut output = String::new();

        let risk_color = self.color_risk(result.risk_level);
        let header_lines = [
            &format!(
                "{}{}IMPACT ANALYSIS:{} {}",
                colors::BOLD,
                colors::RED,
                colors::RESET,
                result.symbol
            ),
            &format!("{}File:{} {}", colors::DIM, colors::RESET, result.file),
            &format!(
                "{}Risk Level:{} {}{}{}{}",
                colors::DIM,
                colors::RESET,
                colors::BOLD,
                risk_color,
                result.risk_level,
                colors::RESET
            ),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        output.push_str(&format!(
            "{}Direct callers ({}):{}\n",
            colors::BOLD,
            result.direct_caller_count,
            colors::RESET
        ));
        for caller in &result.direct_callers {
            output.push_str(&format!("  {} {}\n", box_chars::ARROW_RIGHT, caller));
        }
        output.push('\n');

        if !result.transitive_callers.is_empty() {
            output.push_str(&format!(
                "{}Transitive callers ({}):{}\n",
                colors::BOLD,
                result.transitive_caller_count,
                colors::RESET
            ));
            for caller in result.transitive_callers.iter().take(10) {
                output.push_str(&format!(
                    "  {} {}{}{}\n",
                    box_chars::ARROW_RIGHT,
                    colors::DIM,
                    caller,
                    colors::RESET
                ));
            }
            if result.transitive_callers.len() > 10 {
                output.push_str(&format!(
                    "  {}... and {} more{}\n",
                    colors::DIM,
                    result.transitive_callers.len() - 10,
                    colors::RESET
                ));
            }
            output.push('\n');
        }

        output.push_str(&format!(
            "{}Affected entry points ({}):{}\n",
            colors::BOLD,
            result.affected_entry_points.len(),
            colors::RESET
        ));
        for ep in &result.affected_entry_points {
            output.push_str(&format!(
                "  {} {}{}{}\n",
                box_chars::ARROW_RIGHT,
                colors::GREEN,
                ep,
                colors::RESET
            ));
        }

        output.push_str(&format!(
            "\n{}Files affected:{} {}\n",
            colors::DIM,
            colors::RESET,
            result.files_affected.len()
        ));

        output
    }

    fn format_module(&self, result: &ModuleResult) -> String {
        let mut output = String::new();

        let header_lines = [
            &format!(
                "{}{}MODULE:{} {}",
                colors::BOLD,
                colors::BLUE,
                colors::RESET,
                result.module
            ),
            &format!("{}Path:{} {}", colors::DIM, colors::RESET, result.file_path),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        if !result.exports.is_empty() {
            output.push_str(&format!(
                "{}Exports ({}):{}\n",
                colors::BOLD,
                result.exports.len(),
                colors::RESET
            ));
            for export in &result.exports {
                output.push_str(&format!(
                    "  {} {}{}{}\n",
                    box_chars::ARROW_RIGHT,
                    colors::GREEN,
                    export,
                    colors::RESET
                ));
            }
            output.push('\n');
        }

        if !result.imported_by.is_empty() {
            output.push_str(&format!(
                "{}Imported by ({}):{}\n",
                colors::BOLD,
                result.imported_by.len(),
                colors::RESET
            ));
            for importer in &result.imported_by {
                output.push_str(&format!("  {} {}\n", box_chars::ARROW_RIGHT, importer));
            }
            output.push('\n');
        }

        if !result.dependencies.is_empty() {
            output.push_str(&format!(
                "{}Dependencies ({}):{}\n",
                colors::BOLD,
                result.dependencies.len(),
                colors::RESET
            ));
            for dep in &result.dependencies {
                output.push_str(&format!(
                    "  {} {}{}{}\n",
                    box_chars::ARROW_RIGHT,
                    colors::CYAN,
                    dep,
                    colors::RESET
                ));
            }
            output.push('\n');
        }

        if !result.circular_deps.is_empty() {
            output.push_str(&format!(
                "{}{}CIRCULAR DEPENDENCIES ({}):{}\n",
                colors::BOLD,
                colors::RED,
                result.circular_deps.len(),
                colors::RESET
            ));
            for cycle in &result.circular_deps {
                output.push_str(&format!(
                    "  {}⚠ {}{}\n",
                    colors::YELLOW,
                    cycle,
                    colors::RESET
                ));
            }
        }

        output
    }

    fn format_pattern(&self, result: &PatternResult) -> String {
        let mut output = String::new();

        let header_lines = [
            &format!(
                "{}{}PATTERN:{} {}",
                colors::BOLD,
                colors::MAGENTA,
                colors::RESET,
                result.pattern
            ),
            &format!(
                "{}Found:{} {} matches in {} files",
                colors::DIM,
                colors::RESET,
                result.total_matches,
                result.by_file.len()
            ),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        let mut by_file: std::collections::HashMap<&str, Vec<_>> = std::collections::HashMap::new();
        for m in &result.matches {
            by_file.entry(&m.file).or_default().push(m);
        }

        for (file, matches) in by_file {
            output.push_str(&format!(
                "{}{}{}:{}\n",
                colors::BOLD,
                colors::WHITE,
                file,
                colors::RESET
            ));

            for m in matches {
                // Handle multi-line context
                let context_lines: Vec<&str> = m.context.lines().collect();
                if context_lines.len() > 1 {
                    for line in &context_lines {
                        output.push_str(&format!("  {}\n", line));
                    }
                } else {
                    output.push_str(&format!(
                        "  {}:{:<4}  {}\n",
                        m.line,
                        m.column,
                        m.context.trim()
                    ));
                }

                if let Some(ref enclosing) = m.enclosing_symbol {
                    output.push_str(&format!(
                        "      {}(in {}){}",
                        colors::DIM,
                        enclosing,
                        colors::RESET
                    ));
                    output.push('\n');
                }
            }
            output.push('\n');
        }

        output
    }

    fn format_scope(&self, result: &ScopeResult) -> String {
        let mut output = String::new();

        let scope_name = result.enclosing_scope.as_deref().unwrap_or("<global>");
        let header_lines = [
            &format!(
                "{}{}SCOPE AT:{} {}:{}",
                colors::BOLD,
                colors::CYAN,
                colors::RESET,
                result.file,
                result.line
            ),
            &format!("{}Enclosing:{} {}", colors::DIM, colors::RESET, scope_name),
        ];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        if !result.local_variables.is_empty() {
            output.push_str(&format!(
                "{}Local Variables ({}):{}\n",
                colors::BOLD,
                result.local_variables.len(),
                colors::RESET
            ));
            for var in &result.local_variables {
                output.push_str(&format!(
                    "  {}{}{}: {} {}(line {}){}",
                    colors::CYAN,
                    var.name,
                    colors::RESET,
                    var.kind,
                    colors::DIM,
                    var.defined_at,
                    colors::RESET
                ));
                output.push('\n');
            }
            output.push('\n');
        }

        if !result.parameters.is_empty() {
            output.push_str(&format!(
                "{}Parameters ({}):{}\n",
                colors::BOLD,
                result.parameters.len(),
                colors::RESET
            ));
            for param in &result.parameters {
                output.push_str(&format!(
                    "  {}{}{}: {}\n",
                    colors::YELLOW,
                    param.name,
                    colors::RESET,
                    param.kind
                ));
            }
            output.push('\n');
        }

        if !result.imports.is_empty() {
            output.push_str(&format!(
                "{}Imports ({}):{}\n",
                colors::BOLD,
                result.imports.len(),
                colors::RESET
            ));
            for import in &result.imports {
                output.push_str(&format!("  {}{}{}\n", colors::BLUE, import, colors::RESET));
            }
        }

        output
    }

    fn format_stats(&self, result: &StatsResult) -> String {
        let mut output = String::new();

        let header_lines = [&format!(
            "{}{}CODEBASE STATISTICS{}",
            colors::BOLD,
            colors::GREEN,
            colors::RESET
        )];
        output.push_str(&self.draw_header_box(&header_lines));
        output.push('\n');

        // Overview
        output.push_str(&format!("{}Overview:{}\n", colors::BOLD, colors::RESET));
        output.push_str(&format!("  Files:        {}\n", result.total_files));
        output.push_str(&format!("  Symbols:      {}\n", result.total_symbols));
        output.push_str(&format!("  Tokens:       {}\n", result.total_tokens));
        output.push_str(&format!("  References:   {}\n", result.total_references));
        output.push_str(&format!("  Call Edges:   {}\n", result.total_edges));
        output.push_str(&format!("  Entry Points: {}\n", result.total_entry_points));
        output.push('\n');

        // Files by extension
        output.push_str(&format!(
            "{}Files by extension:{}\n",
            colors::BOLD,
            colors::RESET
        ));
        let mut exts: Vec<_> = result.files_by_extension.iter().collect();
        exts.sort_by(|a, b| b.1.cmp(a.1));
        for (ext, count) in exts.iter().take(10) {
            output.push_str(&format!("  .{}: {}\n", ext, count));
        }
        output.push('\n');

        // Symbols by kind
        output.push_str(&format!(
            "{}Symbols by kind:{}\n",
            colors::BOLD,
            colors::RESET
        ));
        let mut kinds: Vec<_> = result.symbols_by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1));
        for (kind, count) in &kinds {
            output.push_str(&format!("  {}: {}\n", kind, count));
        }
        output.push('\n');

        // Call graph
        output.push_str(&format!("{}Call Graph:{}\n", colors::BOLD, colors::RESET));
        output.push_str(&format!("  Max Call Depth: {}\n", result.max_call_depth));
        output.push_str(&format!("  Avg Call Depth: {:.1}\n", result.avg_call_depth));
        output.push('\n');

        // Most referenced
        if !result.most_referenced.is_empty() {
            output.push_str(&format!(
                "{}Most Referenced Symbols:{}\n",
                colors::BOLD,
                colors::RESET
            ));
            for (name, count) in result.most_referenced.iter().take(10) {
                output.push_str(&format!(
                    "  {}{}{}: {} refs\n",
                    colors::CYAN,
                    name,
                    colors::RESET,
                    count
                ));
            }
            output.push('\n');
        }

        // Largest files
        if !result.largest_files.is_empty() {
            output.push_str(&format!(
                "{}Largest Files (by symbols):{}\n",
                colors::BOLD,
                colors::RESET
            ));
            for (file, count) in result.largest_files.iter().take(10) {
                output.push_str(&format!("  {}: {} symbols\n", file, count));
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_len() {
        let formatter = AsciiFormatter::new();
        assert_eq!(formatter.visible_len("hello"), 5);
        assert_eq!(formatter.visible_len("\x1b[32mhello\x1b[0m"), 5);
        assert_eq!(formatter.visible_len("\x1b[1m\x1b[32mtest\x1b[0m"), 4);
    }

    #[test]
    fn test_format_trace_basic() {
        let formatter = AsciiFormatter::new();
        let result = TraceResult {
            symbol: "validateUser".to_string(),
            defined_at: Some("utils/validation.ts:8".to_string()),
            kind: "function".to_string(),
            invocation_paths: vec![],
            total_paths: 0,
            entry_points: 0,
        };
        let output = formatter.format_trace(&result);
        assert!(output.contains("validateUser"));
        assert!(output.contains("TRACE"));
    }
}
