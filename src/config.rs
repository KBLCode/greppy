use crate::error::Result;
use directories::ProjectDirs;
use std::path::PathBuf;

pub struct Config;

impl Config {
    pub fn home() -> Result<PathBuf> {
        if let Ok(home) = std::env::var("GREPPY_HOME") {
            return Ok(PathBuf::from(home));
        }
        ProjectDirs::from("dev", "greppy", "greppy")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Could not find home directory")
                    .into()
            })
    }

    pub fn socket_path() -> Result<PathBuf> {
        Ok(Self::home()?.join("daemon.sock"))
    }

    pub fn pid_path() -> Result<PathBuf> {
        Ok(Self::home()?.join("daemon.pid"))
    }

    pub fn indexes_dir() -> Result<PathBuf> {
        Ok(Self::home()?.join("indexes"))
    }

    pub fn index_dir(project_path: &std::path::Path) -> Result<PathBuf> {
        let hash = xxhash_rust::xxh3::xxh3_64(project_path.to_string_lossy().as_bytes());
        Ok(Self::indexes_dir()?.join(format!("{:016x}", hash)))
    }

    pub fn registry_path() -> Result<PathBuf> {
        Ok(Self::home()?.join("registry.json"))
    }

    pub fn ensure_home() -> Result<()> {
        let home = Self::home()?;
        if !home.exists() {
            std::fs::create_dir_all(&home)?;
        }
        Ok(())
    }
}

pub const DEFAULT_LIMIT: usize = 20;
pub const MAX_FILE_SIZE: u64 = 1_048_576;
pub const CHUNK_MAX_LINES: usize = 50;
pub const CHUNK_OVERLAP: usize = 5;
pub const CACHE_SIZE: usize = 1000;
