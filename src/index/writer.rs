use crate::config::Config;
use crate::error::{GreppyError, Result};
use crate::index::schema::IndexSchema;
use crate::parse::Chunk;
use std::path::Path;
use tantivy::{doc, Index, IndexWriter as TantivyWriter};

const WRITER_HEAP_SIZE: usize = 50_000_000; // 50MB

pub struct IndexWriter {
    writer: TantivyWriter,
    schema: IndexSchema,
}

impl IndexWriter {
    /// Create a new index for a project
    pub fn create(project_path: &Path) -> Result<Self> {
        let index_dir = Config::index_dir(project_path)?;
        std::fs::create_dir_all(&index_dir)?;

        let schema = IndexSchema::new();
        let index = Index::create_in_dir(&index_dir, schema.schema.clone())
            .map_err(|e| GreppyError::Index(e.to_string()))?;

        let writer = index
            .writer(WRITER_HEAP_SIZE)
            .map_err(|e| GreppyError::Index(e.to_string()))?;

        Ok(Self { writer, schema })
    }

    /// Open existing index or create new
    pub fn open_or_create(project_path: &Path) -> Result<Self> {
        let index_dir = Config::index_dir(project_path)?;

        if index_dir.join("meta.json").exists() {
            // Delete and recreate for now (simple approach)
            std::fs::remove_dir_all(&index_dir)?;
        }

        Self::create(project_path)
    }

    /// Add a chunk to the index
    pub fn add_chunk(&mut self, chunk: &Chunk) -> Result<()> {
        let doc = doc!(
            self.schema.id => chunk.id(),
            self.schema.path => chunk.path.clone(),
            self.schema.content => chunk.content.clone(),
            self.schema.symbol_name => chunk.symbol_name.clone().unwrap_or_default(),
            self.schema.symbol_type => chunk.symbol_type.clone().unwrap_or_default(),
            self.schema.start_line => chunk.start_line as u64,
            self.schema.end_line => chunk.end_line as u64,
            self.schema.language => chunk.language.clone(),
            self.schema.file_hash => chunk.file_hash.clone()
        );

        self.writer.add_document(doc)?;
        Ok(())
    }

    /// Commit changes
    pub fn commit(mut self) -> Result<()> {
        self.writer
            .commit()
            .map_err(|e| GreppyError::Index(e.to_string()))?;
        Ok(())
    }
}
