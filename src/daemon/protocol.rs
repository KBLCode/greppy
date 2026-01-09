use crate::error::{GreppyError, Result};
use crate::search::SearchResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Request sent to the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique request ID
    pub id: String,
    /// The method to invoke
    pub method: RequestMethod,
}

/// Available request methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestMethod {
    /// Search for code
    Search {
        query: String,
        project: PathBuf,
        limit: usize,
    },
    /// Index a project
    Index { project: PathBuf, force: bool },
    /// Get daemon status
    Status,
    /// List indexed projects
    ListProjects,
    /// Remove a project from the index
    ForgetProject { project: PathBuf },
    /// Shutdown the daemon
    Shutdown,
    /// Ping (health check)
    Ping,
}

/// Response from the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Request ID this is responding to
    pub id: String,
    /// Response data or error
    pub result: ResponseResult,
}

/// Result of a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseResult {
    Ok { data: ResponseData },
    Error { message: String },
}

/// Response data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseData {
    /// Search results
    Search(SearchResponse),
    /// Index complete
    Index {
        project: String,
        files_indexed: usize,
        chunks_indexed: usize,
        elapsed_ms: f64,
    },
    /// Daemon status
    Status {
        pid: u32,
        uptime_secs: u64,
        projects_indexed: usize,
        cache_size: usize,
    },
    /// List of indexed projects
    Projects { projects: Vec<ProjectInfo> },
    /// Project forgotten
    Forgotten { project: String },
    /// Pong response
    Pong,
    /// Shutdown acknowledged
    Shutdown,
}

/// Information about an indexed project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub files_indexed: usize,
    pub last_indexed: String,
}

impl Request {
    pub fn new(method: RequestMethod) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method,
        }
    }

    pub fn search(query: String, project: PathBuf, limit: usize) -> Self {
        Self::new(RequestMethod::Search {
            query,
            project,
            limit,
        })
    }

    pub fn index(project: PathBuf, force: bool) -> Self {
        Self::new(RequestMethod::Index { project, force })
    }

    pub fn status() -> Self {
        Self::new(RequestMethod::Status)
    }

    pub fn list_projects() -> Self {
        Self::new(RequestMethod::ListProjects)
    }

    pub fn forget_project(project: PathBuf) -> Self {
        Self::new(RequestMethod::ForgetProject { project })
    }

    pub fn shutdown() -> Self {
        Self::new(RequestMethod::Shutdown)
    }

    pub fn ping() -> Self {
        Self::new(RequestMethod::Ping)
    }
}

impl Response {
    pub fn ok(id: String, data: ResponseData) -> Self {
        Self {
            id,
            result: ResponseResult::Ok { data },
        }
    }

    pub fn error(id: String, message: String) -> Self {
        Self {
            id,
            result: ResponseResult::Error { message },
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self.result, ResponseResult::Ok { .. })
    }
}

// Binary protocol helpers for high-performance IPC
// Uses length-prefixed framing: [4-byte length][bincode payload]
// This is 5-10x faster than JSON + line-based protocol

/// Write a message using binary protocol (length-prefixed MessagePack)
pub async fn write_message<T: Serialize, W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &T,
) -> Result<()> {
    // Serialize to MessagePack (5-10x faster than JSON, supports all serde features)
    // Use named format for better compatibility with custom serde functions
    let bytes = rmp_serde::to_vec_named(message)
        .map_err(|e| GreppyError::Protocol(format!("Serialization failed: {}", e)))?;

    // Write length prefix (u32 = max 4GB message)
    let len = bytes.len() as u32;
    writer.write_all(&len.to_le_bytes()).await?;

    // Write payload
    writer.write_all(&bytes).await?;
    writer.flush().await?;

    Ok(())
}

/// Read a message using binary protocol (length-prefixed MessagePack)
pub async fn read_message<T: for<'de> Deserialize<'de>, R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<T> {
    // Read length prefix
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).await?;
    let len = u32::from_le_bytes(len_bytes) as usize;

    // Sanity check: prevent DoS via huge allocations
    const MAX_MESSAGE_SIZE: usize = 100 * 1024 * 1024; // 100MB
    if len > MAX_MESSAGE_SIZE {
        return Err(GreppyError::Protocol(format!(
            "Message too large: {} bytes (max {})",
            len, MAX_MESSAGE_SIZE
        )));
    }

    // Read payload
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes).await?;

    // Deserialize from MessagePack (use from_slice which handles both named and unnamed)
    let message = rmp_serde::from_slice(&bytes)
        .map_err(|e| GreppyError::Protocol(format!("Deserialization failed: {}", e)))?;

    Ok(message)
}
