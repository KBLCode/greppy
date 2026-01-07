use crate::daemon::{Response, ResponseData, ResponseResult};
use crate::search::SearchResponse;
use colored::Colorize;

/// Format output for human consumption
pub fn format_human(response: &Response) -> String {
    match &response.result {
        ResponseResult::Ok { data } => format_data_human(data),
        ResponseResult::Error { message } => format!("{} {}", "Error:".red().bold(), message),
    }
}

/// Format output as JSON
pub fn format_json(response: &Response) -> String {
    serde_json::to_string_pretty(response).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}

fn format_data_human(data: &ResponseData) -> String {
    match data {
        ResponseData::Search(search) => format_search_human(search),
        ResponseData::Index { project, files_indexed, chunks_indexed, elapsed_ms } => {
            format!(
                "{} Indexed {} files  {} chunks  {}  {}\n{}",
                "✓".bright_cyan().bold(),
                files_indexed.to_string().bright_cyan().bold(),
                chunks_indexed.to_string().bright_cyan(),
                format!("{:.1}ms", elapsed_ms).dimmed(),
                "─".repeat(40).dimmed(),
                format!("  {}", project).dimmed()
            )
        }
        ResponseData::Status { pid, uptime_secs, projects_indexed, cache_size } => {
            let uptime = format_uptime(*uptime_secs);
            format!(
                "{}  {}  {}  {}  {}",
                "Daemon".bright_cyan().bold(),
                format!("pid:{}", pid).dimmed(),
                format!("up:{}", uptime).bright_white(),
                format!("projects:{}", projects_indexed).bright_cyan(),
                format!("cache:{}", cache_size).bright_cyan()
            )
        }
        ResponseData::Projects { projects } => {
            if projects.is_empty() {
                return format!("{}", "No projects indexed".dimmed());
            }
            let mut output = format!("{} ({})  {}\n\n", 
                "Indexed Projects".bright_cyan().bold(), 
                projects.len(),
                "─".repeat(60).dimmed()
            );
            for p in projects {
                output.push_str(&format!(
                    "  {}  {} files  {}  {}\n",
                    p.name.bright_white().bold(),
                    p.files_indexed.to_string().bright_cyan(),
                    p.path.dimmed(),
                    format!("indexed:{}", p.last_indexed).dimmed()
                ));
            }
            output
        }
        ResponseData::Forgotten { project } => {
            format!("{} Removed: {}", "✓".bright_cyan().bold(), project.bright_white())
        }
        ResponseData::Pong => format!("{}", "pong".bright_cyan()),
        ResponseData::Shutdown => format!("{}", "Daemon shutting down...".bright_cyan()),
    }
}

fn format_search_human(search: &SearchResponse) -> String {
    if search.results.is_empty() {
        return format!(
            "{} for \"{}\" {}",
            "No results".dimmed(),
            search.query.bright_white(),
            format!("({:.2}ms{})", search.elapsed_ms, if search.cached { ", cached" } else { "" }).dimmed()
        );
    }

    let cached_indicator = if search.cached { " ⚡" } else { "" };

    let mut output = format!(
        "{} {} results for \"{}\"  {}{}",
        "Found".bright_cyan().bold(),
        search.results.len().to_string().bright_cyan().bold(),
        search.query.bright_white().bold(),
        format!("{:.2}ms", search.elapsed_ms).dimmed(),
        cached_indicator.bright_cyan()
    );
    output.push_str("\n");
    output.push_str(&format!("{}\n\n", "─".repeat(90).dimmed()));

    for (i, result) in search.results.iter().enumerate() {
        // Compact header: number, path, lines, symbol, score - all on one line
        let symbol_info = if let Some(ref symbol) = result.symbol_name {
            let symbol_type = result.symbol_type.as_deref().unwrap_or("sym");
            format!("  {} {}", symbol_type.bright_cyan(), symbol.bright_white())
        } else {
            String::new()
        };

        output.push_str(&format!(
            "{} {}  {}{}  {}\n",
            format!("[{}]", i + 1).bright_cyan().bold(),
            result.path.bright_white().bold(),
            format!("L{}-{}", result.start_line, result.end_line).dimmed(),
            symbol_info,
            format!("score:{:.1}", result.score).dimmed()
        ));

        // Content preview - wider lines (140 chars), show more horizontally
        let preview_lines: Vec<&str> = result.content.lines().take(4).collect();
        for line in preview_lines {
            let trimmed = truncate_str(line, 140);
            output.push_str(&format!("    {} {}\n", "│".bright_cyan(), trimmed));
        }
        if result.content.lines().count() > 4 {
            output.push_str(&format!("    {} {}\n", "│".bright_cyan(), format!("... +{} more lines", result.content.lines().count() - 4).dimmed()));
        }
        output.push('\n');
    }

    output
}

/// Safely truncate a string at a char boundary
fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}

/// Format search response for human consumption (standalone)
pub fn format_search_results(search: &SearchResponse) -> String {
    format_search_human(search)
}
