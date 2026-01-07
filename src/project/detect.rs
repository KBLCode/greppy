use crate::error::{GreppyError, Result};
use std::path::{Path, PathBuf};

const PROJECT_MARKERS: &[&str] = &[
    ".greppy", ".git", "package.json", "Cargo.toml", "pyproject.toml",
    "setup.py", "go.mod", "pom.xml", "build.gradle", "Gemfile",
    "composer.json", "mix.exs", "deno.json",
];

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
}

impl Project {
    pub fn detect(start_path: &Path) -> Result<Self> {
        let root = detect_project_root(start_path)?;
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Ok(Self { root, name })
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let root = path
            .canonicalize()
            .map_err(|_| GreppyError::ProjectNotFound(path.to_path_buf()))?;
        if !root.is_dir() {
            return Err(GreppyError::ProjectNotFound(root));
        }
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Ok(Self { root, name })
    }
}

pub fn detect_project_root(start: &Path) -> Result<PathBuf> {
    let start = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };
    let mut current = start.canonicalize().map_err(|_| GreppyError::NoProjectRoot)?;

    loop {
        for marker in PROJECT_MARKERS {
            if current.join(marker).exists() {
                return Ok(current);
            }
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    Err(GreppyError::NoProjectRoot)
}
