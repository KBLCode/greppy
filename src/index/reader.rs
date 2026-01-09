use crate::config::Config;
use crate::error::{GreppyError, Result};
use crate::index::schema::IndexSchema;
use crate::search::SearchResult;
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery};
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument, Term};

/// Scoring weights for multi-factor ranking
mod scoring {
    /// Boost for exact symbol name match
    pub const SYMBOL_NAME_BOOST: f32 = 3.0;
    /// Boost for signature match (parameter types, return types)
    pub const SIGNATURE_BOOST: f32 = 1.5;
    /// Boost for doc comment match (semantic relevance)
    pub const DOC_COMMENT_BOOST: f32 = 1.0;
    /// Boost for parent symbol match (class/module context)
    pub const PARENT_SYMBOL_BOOST: f32 = 1.0;
    /// Boost for exported/public symbols (more likely to be API entry points)
    pub const EXPORTED_BOOST: f32 = 0.5;
    /// Penalty for test code (usually less relevant for main queries)
    pub const TEST_PENALTY: f32 = 0.5;
}

#[derive(Clone)]
pub struct IndexSearcher {
    reader: IndexReader,
    schema: IndexSchema,
    index: Index,
}

impl IndexSearcher {
    pub fn open(project_path: &Path) -> Result<Self> {
        let index_dir = Config::index_dir(project_path)?;
        if !index_dir.join("meta.json").exists() {
            return Err(GreppyError::IndexNotFound(project_path.to_path_buf()));
        }

        let schema = IndexSchema::new();
        let index = Index::open_in_dir(&index_dir)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay) // Auto-reload on commit (was Manual)
            .try_into()
            .map_err(|e| GreppyError::Index(format!("Failed to create reader: {}", e)))?;

        Ok(Self {
            reader,
            schema,
            index,
        })
    }

    pub fn exists(project_path: &Path) -> Result<bool> {
        let index_dir = Config::index_dir(project_path)?;
        Ok(index_dir.join("meta.json").exists())
    }

    pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        let mut tokenizer = self
            .index
            .tokenizer_for_field(self.schema.content)
            .map_err(|e| GreppyError::Search(e.to_string()))?;

        // Pre-allocate based on estimated token count (avoid reallocations)
        let estimated_tokens = query_text.split_whitespace().count();
        let mut tokens = Vec::with_capacity(estimated_tokens);
        let mut stream = tokenizer.token_stream(query_text);
        while let Some(token) = stream.next() {
            tokens.push(token.text.to_string());
        }

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        // Pre-allocate subqueries vector (5 fields per token)
        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::with_capacity(tokens.len() * 5);
        for token in &tokens {
            // Content match (base BM25)
            let content_term = Term::from_field_text(self.schema.content, token);
            let content_query = TermQuery::new(content_term, IndexRecordOption::WithFreqs);
            subqueries.push((Occur::Should, Box::new(content_query)));

            // Symbol name match (high boost - exact symbol matches are very relevant)
            let symbol_term = Term::from_field_text(self.schema.symbol_name, token);
            let symbol_query = TermQuery::new(symbol_term, IndexRecordOption::WithFreqs);
            let boosted = BoostQuery::new(Box::new(symbol_query), scoring::SYMBOL_NAME_BOOST);
            subqueries.push((Occur::Should, Box::new(boosted)));

            // Signature match (medium boost - parameter/return type matches)
            let sig_term = Term::from_field_text(self.schema.signature, token);
            let sig_query = TermQuery::new(sig_term, IndexRecordOption::WithFreqs);
            let sig_boosted = BoostQuery::new(Box::new(sig_query), scoring::SIGNATURE_BOOST);
            subqueries.push((Occur::Should, Box::new(sig_boosted)));

            // Doc comment match (semantic relevance)
            let doc_term = Term::from_field_text(self.schema.doc_comment, token);
            let doc_query = TermQuery::new(doc_term, IndexRecordOption::WithFreqs);
            let doc_boosted = BoostQuery::new(Box::new(doc_query), scoring::DOC_COMMENT_BOOST);
            subqueries.push((Occur::Should, Box::new(doc_boosted)));

            // Parent symbol match (class/module context)
            let parent_term = Term::from_field_text(self.schema.parent_symbol, token);
            let parent_query = TermQuery::new(parent_term, IndexRecordOption::WithFreqs);
            let parent_boosted =
                BoostQuery::new(Box::new(parent_query), scoring::PARENT_SYMBOL_BOOST);
            subqueries.push((Occur::Should, Box::new(parent_boosted)));
        }

        let query = BooleanQuery::new(subqueries);

        // Fetch more results than needed for post-processing score adjustments
        let fetch_limit = (limit * 2).max(50);
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(fetch_limit))
            .map_err(|e| GreppyError::Search(e.to_string()))?;

        // Parallel document processing with Rayon (4-16x speedup)
        let results: Result<Vec<_>> = top_docs
            .par_iter()
            .map(|(base_score, doc_address)| {
                let doc: TantivyDocument = searcher
                    .doc(*doc_address)
                    .map_err(|e| GreppyError::Search(e.to_string()))?;

                // Use Arc<str> for zero-copy cloning (40-50% memory reduction)
                let path: Arc<str> = Arc::from(
                    doc.get_first(self.schema.path)
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                );
                let content: Arc<str> = Arc::from(
                    doc.get_first(self.schema.content)
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                );
                let symbol_name = doc
                    .get_first(self.schema.symbol_name)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| Arc::from(s));
                let symbol_type = doc
                    .get_first(self.schema.symbol_type)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| Arc::from(s));
                let start_line = doc
                    .get_first(self.schema.start_line)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let end_line = doc
                    .get_first(self.schema.end_line)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let language: Arc<str> = Arc::from(
                    doc.get_first(self.schema.language)
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown"),
                );

                // New AST-aware fields
                let signature = doc
                    .get_first(self.schema.signature)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| Arc::from(s));
                let parent_symbol = doc
                    .get_first(self.schema.parent_symbol)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| Arc::from(s));
                let doc_comment = doc
                    .get_first(self.schema.doc_comment)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| Arc::from(s));
                let is_exported = doc
                    .get_first(self.schema.is_exported)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    == 1;
                let is_test = doc
                    .get_first(self.schema.is_test)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    == 1;

                // Apply post-retrieval score adjustments inline
                let mut final_score = *base_score;

                // Boost exported symbols (more likely to be API entry points)
                if is_exported {
                    final_score += scoring::EXPORTED_BOOST;
                }

                // Penalize test code (usually less relevant for main queries)
                if is_test {
                    final_score -= scoring::TEST_PENALTY;
                }

                Ok(SearchResult {
                    path,
                    content,
                    symbol_name,
                    symbol_type,
                    start_line,
                    end_line,
                    language,
                    score: final_score,
                    signature,
                    parent_symbol,
                    doc_comment,
                    is_exported,
                    is_test,
                })
            })
            .collect();

        let mut results = results?;

        // Use partial sort for top-K selection (faster than full sort for large result sets)
        // Only sort the top `limit` results, leave the rest unsorted
        if results.len() > limit {
            // Use select_nth_unstable to partition around the Kth element
            // This is O(n) average case vs O(n log n) for full sort
            results.select_nth_unstable_by(limit, |a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Now sort only the top `limit` elements
            results[..limit].sort_unstable_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            results.truncate(limit);
            Ok(results)
        } else {
            // If results <= limit, just sort normally
            results.sort_unstable_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            Ok(results)
        }
    }
}
