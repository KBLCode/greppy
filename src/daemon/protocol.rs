use crate::daemon::events::DaemonEvent;
use crate::search::SearchResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: Method,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum Method {
    Search {
        query: String,
        project: String,
        limit: usize,
    },
    Index {
        project: String,
        force: bool,
    },
    IndexWatch {
        project: String,
    },
    Status,
    List,
    Forget {
        project: String,
    },
    Stop,
    /// Subscribe to daemon events (returns a stream)
    Subscribe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    pub result: ResponseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ResponseResult {
    Search(SearchResponse),
    Index {
        project: String,
        file_count: usize,
        chunk_count: usize,
        elapsed_ms: f64,
    },
    Status {
        running: bool,
        pid: u32,
        projects: Vec<ProjectInfo>,
    },
    List {
        projects: Vec<ProjectInfo>,
    },
    Forget {
        project: String,
        success: bool,
    },
    Stop {
        success: bool,
    },
    /// Subscribed to events successfully
    Subscribed,
    /// An event from the daemon (streamed after Subscribe)
    Event(DaemonEvent),
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub chunk_count: usize,
    pub watching: bool,
}
