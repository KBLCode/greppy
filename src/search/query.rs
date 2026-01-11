//! Query parsing and execution

use crate::ai::embedding::Embedder;
use crate::core::error::{Error, Result};
use crate::index::TantivyIndex;
use crate::search::results::{SearchResponse, SearchResult};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery};
use tantivy::schema::{IndexRecordOption, Value};
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
    /// Optional embedding for vector search
    pub embedding: Option<Vec<f32>>,
}

impl SearchQuery {
    /// Create a new search query
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: 20,
            path_filters: Vec::new(),
            include_tests: false,
            embedding: None,
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

    /// Set embedding for vector search
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Execute the search against an index
    pub fn execute(&self, index: &TantivyIndex) -> Result<SearchResponse> {
        let start = Instant::now();

        let searcher = index.reader.searcher();
        let schema = &index.schema;

        // Build the query
        let query = self.build_query(index)?;

        // Execute search
        // Note: If we had proper vector search, we would use a different collector or query here.
        // Since we are using keyword search (BM25) primarily for now (as vector field is bytes),
        // we rely on the BooleanQuery built in build_query.
        // To implement "True Semantic Search" with the current schema limitation (bytes field),
        // we would need to:
        // 1. Retrieve candidate docs using keyword search (or all docs if small).
        // 2. Load their embeddings (bytes -> Vec<f32>).
        // 3. Compute cosine similarity with self.embedding.
        // 4. Re-rank.
        // This is "Rescoring".

        // Let's implement a simple rescoring if embedding is present.

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(self.limit * 2)) // Fetch more for re-ranking
            .map_err(|e| Error::SearchError {
                message: format!("Search failed: {}", e),
            })?;

        // Collect results
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument =
                searcher.doc(doc_address).map_err(|e| Error::SearchError {
                    message: format!("Failed to retrieve doc: {}", e),
                })?;

            let path = doc
                .get_first(schema.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

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

            // Calculate vector score if embedding is present
            let mut final_score = score;
            if let Some(query_emb) = &self.embedding {
                if let Some(doc_emb_bytes) =
                    doc.get_first(schema.embedding).and_then(|v| v.as_bytes())
                {
                    // Convert bytes back to f32
                    let doc_emb: Vec<f32> = doc_emb_bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                        .collect();

                    if doc_emb.len() == query_emb.len() {
                        let similarity = cosine_similarity(query_emb, &doc_emb);
                        // Combine scores: BM25 + Vector Similarity
                        // Simple linear combination for now
                        final_score = score + (similarity * 10.0); // Boost vector score
                    }
                }
            }

            results.push(SearchResult {
                path,
                content,
                symbol_name,
                symbol_type,
                start_line,
                end_line,
                language,
                score: final_score,
            });
        }

        // Sort by final score
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to limit
        if results.len() > self.limit {
            results.truncate(self.limit);
        }

        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_secs_f64() * 1000.0;

        debug!(
            query = %self.text,
            results = results.len(),
            elapsed_ms = elapsed_ms,
            "Search completed"
        );

        Ok(SearchResponse {
            results,
            query: self.text.clone(),
            elapsed_ms,
            project: "unknown".to_string(), // TODO: Pass project name
        })
    }

    /// Build a Tantivy query from the search text
    fn build_query(&self, index: &TantivyIndex) -> Result<Box<dyn Query>> {
        let schema = &index.schema;

        // Tokenize the query
        let mut tokenizer = index
            .index
            .tokenizer_for_field(schema.content)
            .map_err(|e| Error::SearchError {
                message: format!("Failed to get tokenizer: {}", e),
            })?;

        let mut tokens = Vec::new();
        let mut token_stream = tokenizer.token_stream(&self.text);
        while let Some(token) = token_stream.next() {
            tokens.push(token.text.to_string());
        }

        if tokens.is_empty() {
            // If no tokens (e.g. symbols only), fall back to MatchAll if we have embedding?
            // Or return error.
            // For now, return error as before.
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
            subqueries.push((Occur::Should, Box::new(content_query)));

            // Symbol name query (boosted 3x)
            let symbol_term = Term::from_field_text(schema.symbol_name, token);
            let symbol_query = TermQuery::new(symbol_term, IndexRecordOption::WithFreqs);
            let boosted_symbol = BoostQuery::new(Box::new(symbol_query), 3.0);
            subqueries.push((Occur::Should, Box::new(boosted_symbol)));
        }

        Ok(Box::new(BooleanQuery::new(subqueries)))
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}
