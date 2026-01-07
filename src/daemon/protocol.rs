use crate::search::SearchResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
#[serde(tag = "type", content = "params")]
pub enum RequestMethod {
    /// Search for code
    Search {
        query: String,
        project: PathBuf,
        limit: usize,
    },
    /// Index a project
    Index {
        project: PathBuf,
        force: bool,
    },
    /// Get daemon status
    Status,
    /// List indexed projects
    ListProjects,
    /// Remove a project from the index
    ForgetProject {
        project: PathBuf,
    },
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
    #[serde(flatten)]
    pub result: ResponseResult,
}

/// Result of a request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ResponseResult {
    #[serde(rename = "ok")]
    Ok { data: ResponseData },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Response data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
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
    Projects {
        projects: Vec<ProjectInfo>,
    },
    /// Project forgotten
    Forgotten {
        project: String,
    },
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
        Self::new(RequestMethod::Search { query, project, limit })
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
