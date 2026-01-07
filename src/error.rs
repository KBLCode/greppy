use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, GreppyError>;

#[derive(Error, Debug)]
pub enum GreppyError {
    #[error("Project not found: {0}")]
    ProjectNotFound(PathBuf),

    #[error("No project root found (looked for .git, package.json, Cargo.toml, etc.)")]
    NoProjectRoot,

    #[error("Index not found for project: {0}")]
    IndexNotFound(PathBuf),

    #[error("Index error: {0}")]
    Index(String),

    #[error("Search error: {0}")]
    Search(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Daemon not running")]
    DaemonNotRunning,

    #[error("Daemon already running (pid {0})")]
    DaemonAlreadyRunning(u32),

    #[error("Daemon error: {0}")]
    Daemon(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),
}
