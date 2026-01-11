//! Output formatting

pub mod human;
pub mod json;

use crate::cli::OutputFormat;
use crate::search::SearchResponse;

pub fn format_results(results: &SearchResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => human::format(results),
        OutputFormat::Json => json::format(results),
    }
}
