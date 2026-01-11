use crate::core::config::Config;
use crate::core::error::{Error, Result};
use crate::index::schema::IndexSchema;
use crate::index::tantivy_index::TantivyIndex;
use crate::parse::Chunk;
use std::path::Path;
use tantivy::{doc, IndexWriter as TantivyWriter};

const WRITER_HEAP_SIZE: usize = 50_000_000; // 50MB

pub struct IndexWriter {
    writer: TantivyWriter,
    schema: IndexSchema,
}

impl IndexWriter {
    /// Create a new index writer from an existing index
    pub fn new(index: &TantivyIndex) -> Result<Self> {
        let writer = index
            .index
            .writer(WRITER_HEAP_SIZE)
            .map_err(|e| Error::IndexError {
                message: e.to_string(),
            })?;

        Ok(Self {
            writer,
            schema: index.schema.clone(),
        })
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
        self.writer.commit().map_err(|e| Error::IndexError {
            message: e.to_string(),
        })?;
        Ok(())
    }
}
