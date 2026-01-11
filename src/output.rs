use crate::search::SearchResponse;

/// Format results for human consumption
pub fn format_human(response: &SearchResponse) -> String {
    let mut out = String::new();

    if response.results.is_empty() {
        out.push_str(&format!(
            "No results for '{}' ({:.1}ms)\n",
            response.query, response.elapsed_ms
        ));
        return out;
    }

    out.push_str(&format!(
        "Found {} results for '{}' ({:.1}ms)\n\n",
        response.results.len(),
        response.query,
        response.elapsed_ms
    ));

    for (i, result) in response.results.iter().enumerate() {
        // Header
        out.push_str(&format!(
            "{}. {}:{}-{} ({:.2})\n",
            i + 1,
            result.path,
            result.start_line,
            result.end_line,
            result.score
        ));

        // Symbol info
        if let (Some(name), Some(stype)) = (&result.symbol_name, &result.symbol_type) {
            out.push_str(&format!("   {} {}\n", stype, name));
        }

        // Content preview (first 3 lines)
        for line in result.content.lines().take(3) {
            let display = if line.len() > 80 {
                format!("{}...", &line[..77])
            } else {
                line.to_string()
            };
            out.push_str(&format!("   {}\n", display));
        }

        if result.content.lines().count() > 3 {
            out.push_str("   ...\n");
        }

        out.push('\n');
    }

    out
}

/// Format results as JSON
pub fn format_json(response: &SearchResponse) -> String {
    serde_json::to_string_pretty(response).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}

/// Format status output
pub fn format_status(running: bool, pid: Option<u32>, projects: &[(String, usize, bool)]) -> String {
    let mut out = String::new();

    if running {
        out.push_str(&format!("Daemon: running (pid {})\n", pid.unwrap_or(0)));
    } else {
        out.push_str("Daemon: not running\n");
        return out;
    }

    out.push_str(&format!("Projects indexed: {}\n\n", projects.len()));

    for (path, chunks, watching) in projects {
        let watch_str = if *watching { " [watching]" } else { "" };
        out.push_str(&format!("  {} ({} chunks){}\n", path, chunks, watch_str));
    }

    out
}

/// Format list output
pub fn format_list(projects: &[(String, usize, bool)]) -> String {
    if projects.is_empty() {
        return "No indexed projects.\n".to_string();
    }

    let mut out = String::new();
    out.push_str("Indexed projects:\n\n");

    for (path, chunks, watching) in projects {
        let watch_str = if *watching { " [watching]" } else { "" };
        out.push_str(&format!("  {} ({} chunks){}\n", path, chunks, watch_str));
    }

    out
}
