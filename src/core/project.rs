//! Project detection and management

use crate::core::config::Config;
use crate::core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Project root markers in priority order
const PROJECT_MARKERS: &[&str] = &[
    ".greppy",        // Explicit greppy marker
    ".git",           // Git repository
    "package.json",   // Node.js
    "Cargo.toml",     // Rust
    "pyproject.toml", // Python (modern)
    "setup.py",       // Python (legacy)
    "go.mod",         // Go
    "pom.xml",        // Java Maven
    "build.gradle",   // Java Gradle
    "Gemfile",        // Ruby
    "composer.json",  // PHP
    "mix.exs",        // Elixir
    "deno.json",      // Deno
    "bun.lockb",      // Bun
];

/// Represents a detected project
#[derive(Debug, Clone)]
pub struct Project {
    /// Absolute path to project root
    pub root: PathBuf,
    /// Type of project (based on marker found)
    pub project_type: ProjectType,
    /// Name of the project (directory name)
    pub name: String,
}

/// Type of project based on detected marker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Greppy,
    Git,
    NodeJs,
    Rust,
    Python,
    Go,
    Java,
    Ruby,
    Php,
    Elixir,
    Deno,
    Bun,
    Unknown,
}

impl Project {
    /// Detect project from a path (searches upward for markers)
    pub fn detect(start_path: &Path) -> Result<Self> {
        let root = find_project_root(start_path)?;
        let project_type = detect_project_type(&root);
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            root,
            project_type,
            name,
        })
    }

    /// Create project from explicit path (must exist)
    pub fn from_path(path: &Path) -> Result<Self> {
        let root = path.canonicalize().map_err(|_| Error::ProjectNotFound {
            path: path.to_path_buf(),
        })?;

        if !root.is_dir() {
            return Err(Error::ProjectNotFound { path: root });
        }

        let project_type = detect_project_type(&root);
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            root,
            project_type,
            name,
        })
    }
}

/// Find project root by searching upward for markers
fn find_project_root(start: &Path) -> Result<PathBuf> {
    let start = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };

    let mut current = start.canonicalize().map_err(|_| Error::NoProjectRoot)?;

    loop {
        // Check for any project marker
        for marker in PROJECT_MARKERS {
            if current.join(marker).exists() {
                return Ok(current);
            }
        }

        // Move up to parent
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    Err(Error::NoProjectRoot)
}

/// Detect project type from root directory
fn detect_project_type(root: &Path) -> ProjectType {
    for (marker, project_type) in [
        (".greppy", ProjectType::Greppy),
        (".git", ProjectType::Git),
        ("package.json", ProjectType::NodeJs),
        ("Cargo.toml", ProjectType::Rust),
        ("pyproject.toml", ProjectType::Python),
        ("setup.py", ProjectType::Python),
        ("go.mod", ProjectType::Go),
        ("pom.xml", ProjectType::Java),
        ("build.gradle", ProjectType::Java),
        ("Gemfile", ProjectType::Ruby),
        ("composer.json", ProjectType::Php),
        ("mix.exs", ProjectType::Elixir),
        ("deno.json", ProjectType::Deno),
        ("bun.lockb", ProjectType::Bun),
    ] {
        if root.join(marker).exists() {
            return project_type;
        }
    }

    ProjectType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root_git() {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir(&git_dir).unwrap();

        let nested = temp.path().join("src").join("deep").join("nested");
        std::fs::create_dir_all(&nested).unwrap();

        let project = Project::detect(&nested).unwrap();
        assert_eq!(project.root, temp.path().canonicalize().unwrap());
        assert_eq!(project.project_type, ProjectType::Git);
    }

    #[test]
    fn test_find_project_root_cargo() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let project = Project::detect(temp.path()).unwrap();
        assert_eq!(project.project_type, ProjectType::Rust);
    }

    #[test]
    fn test_no_project_root() {
        let temp = TempDir::new().unwrap();
        let result = Project::detect(temp.path());
        assert!(result.is_err());
    }
}

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
