use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new() -> Result<Self> {
        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::BGEBaseENV15;
        options.show_download_progress = true;

        // Enable quantization if available (faster inference)
        // Note: fastembed-rs might not expose quantization flags directly in InitOptions
        // but BGEBaseENV15 is already a good balance.
        // For "instant" speed, we might want BGESmallENV15.

        let model = TextEmbedding::try_new(options)?;
        Ok(Self { model })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let documents = vec![text];
        let embeddings = self.model.embed(documents, None)?;
        Ok(embeddings[0].clone())
    }

    pub fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model.embed(texts, None)?;
        Ok(embeddings)
    }
}

// Singleton instance for reuse if needed, though we might instantiate per process
// For parallel indexing, we might want one embedder per thread or a shared one.
// fastembed is thread-safe.
