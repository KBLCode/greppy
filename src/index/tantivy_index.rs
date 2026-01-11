//! Tantivy index wrapper

use crate::core::config::Config;
use crate::core::error::{Error, Result};
use crate::index::schema::IndexSchema;
use std::path::Path;
use tantivy::{Index, IndexReader, ReloadPolicy};

/// Wrapper around Tantivy index
pub struct TantivyIndex {
    pub index: Index,
    pub schema: IndexSchema,
    pub reader: IndexReader,
}

impl TantivyIndex {
    /// Open or create an index for a project
    pub fn open_or_create(project_path: &Path) -> Result<Self> {
        let index_dir = Config::index_dir(project_path)?;
        std::fs::create_dir_all(&index_dir)?;

        let schema = IndexSchema::new();

        let index = if index_dir.join("meta.json").exists() {
            // Open existing index
            Index::open_in_dir(&index_dir).map_err(|e| Error::IndexError {
                message: format!("Failed to open index: {}", e),
            })?
        } else {
            // Create new index
            Index::create_in_dir(&index_dir, schema.schema.clone()).map_err(|e| {
                Error::IndexError {
                    message: format!("Failed to create index: {}", e),
                }
            })?
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| Error::IndexError {
                message: format!("Failed to create reader: {}", e),
            })?;

        Ok(Self {
            index,
            schema,
            reader,
        })
    }

    /// Open an existing index (fails if not found)
    pub fn open(project_path: &Path) -> Result<Self> {
        let index_dir = Config::index_dir(project_path)?;

        if !index_dir.join("meta.json").exists() {
            return Err(Error::IndexNotFound {
                path: project_path.to_path_buf(),
            });
        }

        let schema = IndexSchema::new();
        let index = Index::open_in_dir(&index_dir)?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| Error::IndexError {
                message: format!("Failed to create reader: {}", e),
            })?;

        Ok(Self {
            index,
            schema,
            reader,
        })
    }

    /// Check if an index exists for a project
    pub fn exists(project_path: &Path) -> Result<bool> {
        let index_dir = Config::index_dir(project_path)?;
        Ok(index_dir.join("meta.json").exists())
    }

    /// Delete an index for a project
    pub fn delete(project_path: &Path) -> Result<()> {
        let index_dir = Config::index_dir(project_path)?;
        if index_dir.exists() {
            std::fs::remove_dir_all(&index_dir)?;
        }
        Ok(())
    }
}
