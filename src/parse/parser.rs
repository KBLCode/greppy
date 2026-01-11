use crate::parse::Chunk;
use anyhow::Result;

/// Trait for code parsers
pub trait CodeParser {
    /// Parse content and return chunks
    fn chunk(&self, path: &str, content: &str) -> Result<Vec<Chunk>>;
}

pub struct HeuristicParser;

impl CodeParser for HeuristicParser {
    fn chunk(&self, path: &str, content: &str) -> Result<Vec<Chunk>> {
        // Use the existing heuristic chunker
        // We need to adapt the signature slightly or call the existing function
        // For now, let's just wrap the existing function
        use crate::parse::chunker::chunk_file;
        use std::path::Path;

        Ok(chunk_file(Path::new(path), content))
    }
}
