use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Arc;

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new() -> Result<Self> {
        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::BGEBaseENV15;
        options.show_download_progress = true;

        let model = TextEmbedding::try_new(options)?;
        Ok(Self { model })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let documents = vec![text];
        let embeddings = self.model.embed(documents, None)?;
        Ok(embeddings[0].clone())
    }
}

// Singleton instance for reuse if needed, though we might instantiate per process
// For parallel indexing, we might want one embedder per thread or a shared one.
// fastembed is thread-safe.
