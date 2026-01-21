use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_type: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub score: f32,
}

impl SearchResult {
    /// Check if this result overlaps with another (same file, overlapping lines)
    fn overlaps(&self, other: &SearchResult) -> bool {
        self.path == other.path
            && self.start_line <= other.end_line
            && other.start_line <= self.end_line
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub query: String,
    pub elapsed_ms: f64,
    pub project: String,
}

impl SearchResponse {
    /// Remove duplicate/overlapping results, keeping highest scoring
    ///
    /// Results from overlapping chunks (same file, overlapping line ranges)
    /// are deduplicated, keeping only the highest scoring one.
    pub fn deduplicate(&mut self) {
        if self.results.len() <= 1 {
            return;
        }

        // Sort by score descending so we keep highest scoring when deduping
        self.results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut deduped = Vec::with_capacity(self.results.len());

        for result in std::mem::take(&mut self.results) {
            // Check if this overlaps with any already-kept result
            let dominated = deduped
                .iter()
                .any(|kept: &SearchResult| kept.overlaps(&result));

            if !dominated {
                deduped.push(result);
            }
        }

        self.results = deduped;
    }
}
