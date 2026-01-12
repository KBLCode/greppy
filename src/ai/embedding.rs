use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    /// Create a new embedder instance.
    /// Note: This is expensive (1-3 seconds). Prefer using `get_global()` for reuse.
    pub fn new() -> Result<Self> {
        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::BGEBaseENV15;
        options.show_download_progress = true;

        let model = TextEmbedding::try_new(options)?;
        Ok(Self { model })
    }

    /// Embed a single text
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let documents = vec![text];
        let embeddings = self.model.embed(documents, None)?;
        Ok(embeddings[0].clone())
    }

    /// Embed a batch of texts (more efficient than individual calls)
    pub fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let embeddings = self.model.embed(texts, None)?;
        Ok(embeddings)
    }
}

// Global singleton for embedder reuse
// Using Mutex<Option<Arc<Embedder>>> for lazy initialization
static GLOBAL_EMBEDDER: Mutex<Option<Arc<Embedder>>> = Mutex::new(None);

/// Get or initialize the global embedder instance.
/// This is thread-safe and will only initialize once.
/// Returns None if initialization fails.
pub fn get_global_embedder() -> Option<Arc<Embedder>> {
    let mut guard = GLOBAL_EMBEDDER.lock();

    if let Some(ref embedder) = *guard {
        return Some(Arc::clone(embedder));
    }

    // Initialize on first access
    match Embedder::new() {
        Ok(embedder) => {
            let arc = Arc::new(embedder);
            *guard = Some(Arc::clone(&arc));
            Some(arc)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to initialize global embedder");
            None
        }
    }
}

/// Try to get the global embedder, initializing if needed.
/// Returns an error if initialization fails.
pub fn try_get_global_embedder() -> Result<Arc<Embedder>> {
    let mut guard = GLOBAL_EMBEDDER.lock();

    if let Some(ref embedder) = *guard {
        return Ok(Arc::clone(embedder));
    }

    // Initialize on first access
    let embedder = Embedder::new()?;
    let arc = Arc::new(embedder);
    *guard = Some(Arc::clone(&arc));
    Ok(arc)
}
