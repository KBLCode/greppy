//! Human-readable output formatting

use crate::search::SearchResponse;

/// Format results for human consumption
pub fn format(results: &SearchResponse) -> String {
    let mut output = String::new();

    if results.results.is_empty() {
        output.push_str(&format!(
            "No results found for '{}' ({:.1}ms)\n",
            results.query, results.elapsed_ms
        ));
        return output;
    }

    output.push_str(&format!(
        "Found {} results for '{}' ({:.1}ms)\n\n",
        results.results.len(),
        results.query,
        results.elapsed_ms
    ));

    for (i, result) in results.results.iter().enumerate() {
        // Header: path:lines (score)
        output.push_str(&format!(
            "{}. {}:{}-{} ({:.2})\n",
            i + 1,
            result.path,
            result.start_line,
            result.end_line,
            result.score
        ));

        // Symbol info if available
        if let (Some(name), Some(stype)) = (&result.symbol_name, &result.symbol_type) {
            output.push_str(&format!("   {} {}\n", stype, name));
        }

        // Content preview (first 3 lines, truncated)
        let preview_lines: Vec<&str> = result.content.lines().take(3).collect();
        for line in preview_lines {
            let truncated: String = if line.len() > 80 {
                format!("{}...", &line[..77])
            } else {
                line.to_string()
            };
            output.push_str(&format!("   {}\n", truncated));
        }

        if result.content.lines().count() > 3 {
            output.push_str("   ...\n");
        }

        output.push('\n');
    }

    output
}
