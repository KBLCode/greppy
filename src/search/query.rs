//! Query parsing and execution

use crate::core::error::{Error, Result};
use crate::index::TantivyIndex;
use crate::search::results::{SearchResult, SearchResults};
use std::path::PathBuf;
use std::time::Instant;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::tokenizer::TextAnalyzer;
use tantivy::Term;
use tracing::debug;

/// A search query with options
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// The search text
    pub text: String,
    /// Maximum results to return
    pub limit: usize,
    /// Filter to specific paths
    pub path_filters: Vec<PathBuf>,
    /// Include test files
    pub include_tests: bool,
}

impl SearchQuery {
    /// Create a new search query
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: 20,
            path_filters: Vec::new(),
            include_tests: false,
        }
    }

    /// Set the result limit
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Add path filters
    pub fn with_path_filters(mut self, paths: Vec<PathBuf>) -> Self {
        self.path_filters = paths;
        self
    }

    /// Include test files
    pub fn with_tests(mut self, include: bool) -> Self {
        self.include_tests = include;
        self
    }

    /// Execute the search against an index
    pub fn execute(&self, index: &TantivyIndex) -> Result<SearchResults> {
        let start = Instant::now();

        let searcher = index.reader.searcher();
        let schema = &index.schema;

        // Build the query
        let query = self.build_query(index)?;

        // Execute search
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(self.limit))
            .map_err(|e| Error::SearchError {
                message: format!("Search failed: {}", e),
            })?;

        // Collect results
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address).map_err(|e| Error::SearchError {
                message: format!("Failed to retrieve doc: {}", e),
            })?;

            let path = doc
                .get_first(schema.path)
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .unwrap_or_default();

            let content = doc
                .get_first(schema.content)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let symbol_name = doc
                .get_first(schema.symbol_name)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);

            let symbol_type = doc
                .get_first(schema.symbol_type)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);

            let start_line = doc
                .get_first(schema.start_line)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            let end_line = doc
                .get_first(schema.end_line)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            let language = doc
                .get_first(schema.language)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            results.push(SearchResult {
                path,
                content,
                symbol_name,
                symbol_type,
                start_line,
                end_line,
                language,
                score,
            });
        }

        let elapsed = start.elapsed();
        debug!(
            query = %self.text,
            results = results.len(),
            elapsed_ms = elapsed.as_secs_f64() * 1000.0,
            "Search completed"
        );

        Ok(SearchResults {
            results,
            query: self.text.clone(),
            elapsed,
        })
    }

    /// Build a Tantivy query from the search text
    fn build_query(&self, index: &TantivyIndex) -> Result<Box<dyn Query>> {
        let schema = &index.schema;

        // Tokenize the query
        let tokenizer = index.index.tokenizer_for_field(schema.content).map_err(|e| {
            Error::SearchError {
                message: format!("Failed to get tokenizer: {}", e),
            }
        })?;

        let mut tokens = Vec::new();
        let mut token_stream = tokenizer.token_stream(&self.text);
        while let Some(token) = token_stream.next() {
            tokens.push(token.text.to_string());
        }

        if tokens.is_empty() {
            return Err(Error::SearchError {
                message: "Query produced no tokens".to_string(),
            });
        }

        // Build query: search in content and symbol_name (boosted)
        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        for token in &tokens {
            // Content query
            let content_term = Term::from_field_text(schema.content, token);
            let content_query = TermQuery::new(content_term, IndexRecordOption::WithFreqs);

            // Symbol name query (boosted 3x)
            let symbol_term = Term::from_field_text(schema.symbol_name, token);
            let symbol_query = TermQuery::new(symbol_term, IndexRecordOption::WithFreqs);
            let boosted_symbol = BoostQuery::new(Box::new(symbol_query), 3.0);

            // Combine with OR
            subqueries.push((Occur::Should, Box::new(content_query)));
            subqueries.push((Occur::Should, Box::new(boosted_symbol)));
        }

        Ok(Box::new(BooleanQuery::new(subqueries)))
    }
}
