//! Error types for Greppy

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using Greppy's Error
pub type Result<T> = std::result::Result<T, Error>;

/// Greppy error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("Project not found: {path}")]
    ProjectNotFound { path: PathBuf },

    #[error("No project root found (looked for .git, package.json, Cargo.toml, etc.)")]
    NoProjectRoot,

    #[error("Index not found for project: {path}")]
    IndexNotFound { path: PathBuf },

    #[error("Index error: {message}")]
    IndexError { message: String },

    #[error("Search error: {message}")]
    SearchError { message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Daemon error: {message}")]
    DaemonError { message: String },

    #[error("Daemon not running")]
    DaemonNotRunning,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {message}")]
    IoError { message: String },

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Auth error: {0}")]
    Auth(#[from] anyhow::Error),
}
