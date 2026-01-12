//! Configuration management

use crate::core::error::{Error, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Global configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub watch: WatchConfig,
    pub ignore: IgnoreConfig,
    pub index: IndexConfig,
    pub cache: CacheConfig,
    #[serde(default)]
    pub projects: HashMap<String, ProjectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Default result limit
    pub default_limit: usize,
    /// Auto-start daemon
    pub daemon_autostart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchConfig {
    /// Directories to watch
    pub paths: Vec<PathBuf>,
    /// Recursively discover projects
    pub recursive: bool,
    /// Debounce time in milliseconds
    pub debounce_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IgnoreConfig {
    /// Global ignore patterns
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexConfig {
    /// Maximum file size to index (bytes)
    pub max_file_size: u64,
    /// Maximum files per project
    pub max_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Query cache TTL (seconds)
    pub query_ttl: u64,
    /// Maximum cached queries
    pub max_queries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    /// Project-specific ignore patterns
    pub ignore: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            watch: WatchConfig::default(),
            ignore: IgnoreConfig::default(),
            index: IndexConfig::default(),
            cache: CacheConfig::default(),
            projects: HashMap::new(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_limit: 20,
            daemon_autostart: false,
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            paths: vec![],
            recursive: true,
            debounce_ms: 100,
        }
    }
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "__pycache__".to_string(),
                "*.min.js".to_string(),
                "*.map".to_string(),
            ],
        }
    }
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            max_file_size: 1_048_576, // 1MB
            max_files: 100_000,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            query_ttl: 60,
            max_queries: 1000,
        }
    }
}

impl Config {
    /// Load configuration from default location
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        let home = Self::greppy_home()?;
        Ok(home.join("config.toml"))
    }

    /// Get the greppy home directory
    pub fn greppy_home() -> Result<PathBuf> {
        // Check GREPPY_HOME env var first
        if let Ok(home) = std::env::var("GREPPY_HOME") {
            return Ok(PathBuf::from(home));
        }

        // Use XDG directories
        ProjectDirs::from("dev", "greppy", "greppy")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .ok_or_else(|| Error::ConfigError {
                message: "Could not determine greppy home directory".to_string(),
            })
    }

    /// Get the index directory for a project
    pub fn index_dir(project_path: &std::path::Path) -> Result<PathBuf> {
        let home = Self::greppy_home()?;
        let hash = xxhash_rust::xxh3::xxh3_64(project_path.to_string_lossy().as_bytes());
        Ok(home.join("indexes").join(format!("{:016x}", hash)))
    }

    /// Get registry file path (tracks indexed projects)
    pub fn registry_path() -> Result<PathBuf> {
        Ok(Self::greppy_home()?.join("registry.json"))
    }

    /// Ensure home directory exists
    pub fn ensure_home() -> Result<()> {
        let home = Self::greppy_home()?;
        if !home.exists() {
            std::fs::create_dir_all(&home)?;
        }
        Ok(())
    }

    /// Get the daemon socket path
    pub fn socket_path() -> Result<PathBuf> {
        if let Ok(socket) = std::env::var("GREPPY_DAEMON_SOCKET") {
            return Ok(PathBuf::from(socket));
        }
        let home = Self::greppy_home()?;
        Ok(home.join("daemon.sock"))
    }

    /// Get the daemon PID file path
    pub fn pid_path() -> Result<PathBuf> {
        let home = Self::greppy_home()?;
        Ok(home.join("daemon.pid"))
    }

    /// Get the daemon port (Windows only - uses TCP instead of Unix sockets)
    /// Can be overridden with GREPPY_DAEMON_PORT environment variable
    pub fn daemon_port() -> u16 {
        std::env::var("GREPPY_DAEMON_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_DAEMON_PORT)
    }

    /// Get the daemon port file path (Windows - stores which port daemon is using)
    pub fn port_path() -> Result<PathBuf> {
        let home = Self::greppy_home()?;
        Ok(home.join("daemon.port"))
    }
}

/// Default daemon port for Windows TCP connection
/// Using an uncommon port to avoid conflicts
pub const DEFAULT_DAEMON_PORT: u16 = 19532;

pub const MAX_FILE_SIZE: u64 = 1_048_576; // 1MB
pub const CHUNK_MAX_LINES: usize = 50;
pub const CHUNK_OVERLAP: usize = 5;
