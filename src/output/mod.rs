//! Output formatting

pub mod human;
pub mod json;

use crate::cli::OutputFormat;
use crate::search::SearchResults;

/// Format search results for output
pub fn format_results(results: &SearchResults, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => human::format(results),
        OutputFormat::Json => json::format(results),
    }
}
