use crate::config::Config;
use crate::error::{GreppyError, Result};
use crate::index::schema::IndexSchema;
use crate::search::SearchResult;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery};
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::{Index, IndexReader, ReloadPolicy, Term, TantivyDocument};

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
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| GreppyError::Index(format!("Failed to create reader: {}", e)))?;

        Ok(Self { reader, schema, index })
    }

    pub fn exists(project_path: &Path) -> Result<bool> {
        let index_dir = Config::index_dir(project_path)?;
        Ok(index_dir.join("meta.json").exists())
    }

    pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        let mut tokenizer = self.index
            .tokenizer_for_field(self.schema.content)
            .map_err(|e| GreppyError::Search(e.to_string()))?;

        let mut tokens = Vec::new();
        let mut stream = tokenizer.token_stream(query_text);
        while let Some(token) = stream.next() {
            tokens.push(token.text.to_string());
        }

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for token in &tokens {
            let content_term = Term::from_field_text(self.schema.content, token);
            let content_query = TermQuery::new(content_term, IndexRecordOption::WithFreqs);
            subqueries.push((Occur::Should, Box::new(content_query)));

            let symbol_term = Term::from_field_text(self.schema.symbol_name, token);
            let symbol_query = TermQuery::new(symbol_term, IndexRecordOption::WithFreqs);
            let boosted = BoostQuery::new(Box::new(symbol_query), 3.0);
            subqueries.push((Occur::Should, Box::new(boosted)));
        }

        let query = BooleanQuery::new(subqueries);
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| GreppyError::Search(e.to_string()))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| GreppyError::Search(e.to_string()))?;

            let path = doc.get_first(self.schema.path)
                .and_then(|v| v.as_str()).unwrap_or("").to_string();
            let content = doc.get_first(self.schema.content)
                .and_then(|v| v.as_str()).unwrap_or("").to_string();
            let symbol_name = doc.get_first(self.schema.symbol_name)
                .and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from);
            let symbol_type = doc.get_first(self.schema.symbol_type)
                .and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from);
            let start_line = doc.get_first(self.schema.start_line)
                .and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let end_line = doc.get_first(self.schema.end_line)
                .and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let language = doc.get_first(self.schema.language)
                .and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

            results.push(SearchResult {
                path, content, symbol_name, symbol_type,
                start_line, end_line, language, score,
            });
        }

        Ok(results)
    }
}
