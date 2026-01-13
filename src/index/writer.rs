use crate::core::error::{Error, Result};
use crate::index::schema::IndexSchema;
use crate::index::tantivy_index::TantivyIndex;
use crate::parse::Chunk;
use tantivy::{doc, IndexWriter as TantivyWriter, Term};

/// Writer heap size - 50MB is reasonable for most projects
/// This bounds Tantivy's internal memory usage
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

    /// Delete all chunks for a given file path
    ///
    /// Used for incremental updates - delete old chunks before re-indexing.
    /// This is O(1) in Tantivy - it marks documents as deleted without scanning.
    #[inline]
    pub fn delete_by_path(&mut self, path: &str) -> Result<()> {
        let term = Term::from_field_text(self.schema.path, path);
        self.writer.delete_term(term);
        Ok(())
    }

    /// Commit changes and return a new writer
    ///
    /// This is used for periodic commits during large indexing operations
    /// to prevent unbounded memory growth in Tantivy's internal buffers.
    /// After commit, the old writer is consumed and a fresh one is returned.
    pub fn commit_and_reopen(mut self, index: &TantivyIndex) -> Result<Self> {
        self.writer.commit().map_err(|e| Error::IndexError {
            message: e.to_string(),
        })?;
        // Drop old writer, create fresh one
        drop(self.writer);
        Self::new(index)
    }

    /// Commit changes (final commit, consumes writer)
    pub fn commit(mut self) -> Result<()> {
        self.writer.commit().map_err(|e| Error::IndexError {
            message: e.to_string(),
        })?;
        Ok(())
    }
}
