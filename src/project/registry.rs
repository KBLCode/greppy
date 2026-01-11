use crate::config::Config;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: PathBuf,
    pub name: String,
    pub indexed_at: SystemTime,
    pub file_count: usize,
    pub chunk_count: usize,
    pub watching: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub projects: HashMap<String, ProjectEntry>,
}

impl Registry {
    /// Load registry from disk
    pub fn load() -> Result<Self> {
        let path = Config::registry_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let registry: Registry = serde_json::from_str(&content)?;
        Ok(registry)
    }

    /// Save registry to disk
    pub fn save(&self) -> Result<()> {
        Config::ensure_home()?;
        let path = Config::registry_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get project key (hash of path)
    fn key(path: &Path) -> String {
        let hash = xxhash_rust::xxh3::xxh3_64(path.to_string_lossy().as_bytes());
        format!("{:016x}", hash)
    }

    /// Add or update a project
    pub fn upsert(&mut self, entry: ProjectEntry) {
        let key = Self::key(&entry.path);
        self.projects.insert(key, entry);
    }

    /// Get a project by path
    pub fn get(&self, path: &Path) -> Option<&ProjectEntry> {
        let key = Self::key(path);
        self.projects.get(&key)
    }

    /// Remove a project
    pub fn remove(&mut self, path: &Path) -> Option<ProjectEntry> {
        let key = Self::key(path);
        self.projects.remove(&key)
    }

    /// List all projects
    pub fn list(&self) -> Vec<&ProjectEntry> {
        self.projects.values().collect()
    }

    /// Set watching status
    pub fn set_watching(&mut self, path: &Path, watching: bool) {
        let key = Self::key(path);
        if let Some(entry) = self.projects.get_mut(&key) {
            entry.watching = watching;
        }
    }
}
