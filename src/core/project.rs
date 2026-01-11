//! Project detection and management

use crate::core::error::{Error, Result};
use std::path::{Path, PathBuf};

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
