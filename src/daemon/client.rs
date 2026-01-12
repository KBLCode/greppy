//! Client for communicating with daemon

use crate::core::config::Config;
use crate::core::error::{Error, Result};
use crate::daemon::protocol::{Method, Request, Response, ResponseResult};
use crate::search::SearchResponse;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use uuid::Uuid;

/// Check if daemon is running
pub fn is_running() -> Result<bool> {
    let socket_path = Config::socket_path()?;
    Ok(socket_path.exists())
}

/// Connect to the daemon
async fn connect() -> Result<UnixStream> {
    let socket_path = Config::socket_path()?;
    if !socket_path.exists() {
        return Err(Error::DaemonNotRunning);
    }
    Ok(UnixStream::connect(socket_path).await?)
}

/// Send a request to the daemon
async fn send_request(method: Method) -> Result<ResponseResult> {
    let mut stream = connect().await?;
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    let request = Request {
        id: Uuid::new_v4().to_string(),
        method,
    };

    let json = serde_json::to_string(&request)? + "\n";
    writer.write_all(json.as_bytes()).await?;

    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        return Err(Error::DaemonError {
            message: "Daemon closed connection".to_string(),
        });
    }

    let response: Response = serde_json::from_str(&line)?;
    Ok(response.result)
}

/// Send a search request to the daemon
pub async fn search(query: &str, project: &Path, limit: usize) -> Result<SearchResponse> {
    let method = Method::Search {
        query: query.to_string(),
        project: project.to_string_lossy().to_string(),
        limit,
    };

    match send_request(method).await? {
        ResponseResult::Search(response) => Ok(response),
        ResponseResult::Error { message } => Err(Error::DaemonError { message }),
        _ => Err(Error::DaemonError {
            message: "Unexpected response type".to_string(),
        }),
    }
}

/// Send an index request to the daemon
pub async fn index(project: &Path, force: bool) -> Result<(usize, usize, f64)> {
    let method = Method::Index {
        project: project.to_string_lossy().to_string(),
        force,
    };

    match send_request(method).await? {
        ResponseResult::Index {
            file_count,
            chunk_count,
            elapsed_ms,
            ..
        } => Ok((file_count, chunk_count, elapsed_ms)),
        ResponseResult::Error { message } => Err(Error::DaemonError { message }),
        _ => Err(Error::DaemonError {
            message: "Unexpected response type".to_string(),
        }),
    }
}
