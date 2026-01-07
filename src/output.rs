use crate::daemon::{Response, ResponseData, ResponseResult};
use crate::search::SearchResponse;

/// Format output for human consumption
pub fn format_human(response: &Response) -> String {
    match &response.result {
        ResponseResult::Ok { data } => format_data_human(data),
        ResponseResult::Error { message } => format!("Error: {}", message),
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
                "Indexed {} files ({} chunks) in {:.1}ms\nProject: {}",
                files_indexed, chunks_indexed, elapsed_ms, project
            )
        }
        ResponseData::Status { pid, uptime_secs, projects_indexed, cache_size } => {
            let uptime = format_uptime(*uptime_secs);
            format!(
                "Daemon Status:\n  PID: {}\n  Uptime: {}\n  Projects indexed: {}\n  Cache entries: {}",
                pid, uptime, projects_indexed, cache_size
            )
        }
        ResponseData::Projects { projects } => {
            if projects.is_empty() {
                return "No projects indexed".to_string();
            }
            let mut output = format!("Indexed Projects ({}):\n", projects.len());
            for p in projects {
                output.push_str(&format!(
                    "  {} ({} files)\n    {}\n    Last indexed: {}\n",
                    p.name, p.files_indexed, p.path, p.last_indexed
                ));
            }
            output
        }
        ResponseData::Forgotten { project } => {
            format!("Removed project from index: {}", project)
        }
        ResponseData::Pong => "pong".to_string(),
        ResponseData::Shutdown => "Daemon shutting down...".to_string(),
    }
}

fn format_search_human(search: &SearchResponse) -> String {
    if search.results.is_empty() {
        return format!(
            "No results for \"{}\" ({:.2}ms{})",
            search.query,
            search.elapsed_ms,
            if search.cached { ", cached" } else { "" }
        );
    }

    let mut output = format!(
        "Found {} results for \"{}\" ({:.2}ms{})\n\n",
        search.results.len(),
        search.query,
        search.elapsed_ms,
        if search.cached { ", cached" } else { "" }
    );

    for (i, result) in search.results.iter().enumerate() {
        // Header with file path and line numbers
        output.push_str(&format!(
            "{}. {} (lines {}-{})\n",
            i + 1,
            result.path,
            result.start_line,
            result.end_line
        ));

        // Symbol info if available
        if let Some(ref symbol) = result.symbol_name {
            let symbol_type = result.symbol_type.as_deref().unwrap_or("symbol");
            output.push_str(&format!("   {} {}\n", symbol_type, symbol));
        }

        // Score
        output.push_str(&format!("   Score: {:.2}\n", result.score));

        // Content preview (first few lines)
        let preview_lines: Vec<&str> = result.content.lines().take(5).collect();
        for line in preview_lines {
            let trimmed = if line.len() > 100 {
                format!("{}...", &line[..100])
            } else {
                line.to_string()
            };
            output.push_str(&format!("   │ {}\n", trimmed));
        }
        if result.content.lines().count() > 5 {
            output.push_str("   │ ...\n");
        }
        output.push('\n');
    }

    output
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
