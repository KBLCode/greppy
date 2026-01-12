use crate::core::error::{Error, Result};
use crate::index::schema::IndexSchema;
use crate::index::tantivy_index::TantivyIndex;
use crate::parse::Chunk;
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
    pub fn add_chunk(&mut self, chunk: &Chunk, embedding: Option<&[f32]>) -> Result<()> {
        let mut doc = doc!(
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

        if let Some(emb) = embedding {
            // Convert f32 slice to bytes for storage
            // Note: For real vector search, we need to use Tantivy's vector field support
            // which might require a different add_document API or schema definition.
            // But for now, we store it as bytes to enable retrieval.
            // To enable vector search, we would need to use `add_vector_field` in schema
            // and pass the vector here.
            // Since we defined it as bytes in schema for now (as a placeholder/storage),
            // we serialize it.
            let bytes: Vec<u8> = emb.iter().flat_map(|f| f.to_le_bytes()).collect();
            doc.add_field_value(self.schema.embedding, &bytes);
        }

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
