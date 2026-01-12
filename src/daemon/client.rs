//! Client for communicating with daemon

use crate::core::config::Config;
use crate::core::error::{Error, Result};
use crate::daemon::protocol::{Method, Request, Response, ResponseResult};
use crate::search::SearchResponse;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::time::timeout;
use uuid::Uuid;

#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(windows)]
use tokio::net::TcpStream;

/// Default timeout for daemon connection (5 seconds)
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default timeout for simple requests (30 seconds)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Extended timeout for indexing operations (10 minutes)
const INDEX_TIMEOUT: Duration = Duration::from_secs(600);

/// Check if daemon is running (Unix: check socket file exists)
#[cfg(unix)]
pub fn is_running() -> Result<bool> {
    let socket_path = Config::socket_path()?;
    Ok(socket_path.exists())
}

/// Check if daemon is running (Windows: check port file exists)
#[cfg(windows)]
pub fn is_running() -> Result<bool> {
    let port_path = Config::port_path()?;
    Ok(port_path.exists())
}

/// Connect to the daemon with timeout (Unix: Unix socket)
#[cfg(unix)]
async fn connect() -> Result<impl AsyncRead + AsyncWrite + Unpin> {
    let socket_path = Config::socket_path()?;
    if !socket_path.exists() {
        return Err(Error::DaemonNotRunning);
    }

    match timeout(CONNECT_TIMEOUT, UnixStream::connect(&socket_path)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(Error::DaemonError {
            message: format!("Failed to connect to daemon: {}", e),
        }),
        Err(_) => Err(Error::DaemonError {
            message: format!("Connection to daemon timed out after {:?}", CONNECT_TIMEOUT),
        }),
    }
}

/// Connect to the daemon with timeout (Windows: TCP on localhost)
#[cfg(windows)]
async fn connect() -> Result<impl AsyncRead + AsyncWrite + Unpin> {
    let port_path = Config::port_path()?;
    if !port_path.exists() {
        return Err(Error::DaemonNotRunning);
    }

    // Read port from file
    let port_str = std::fs::read_to_string(&port_path).map_err(|e| Error::DaemonError {
        message: format!("Failed to read daemon port file: {}", e),
    })?;
    let port: u16 = port_str.trim().parse().map_err(|e| Error::DaemonError {
        message: format!("Invalid port in daemon port file: {}", e),
    })?;

    let addr = format!("127.0.0.1:{}", port);

    match timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(Error::DaemonError {
            message: format!("Failed to connect to daemon at {}: {}", addr, e),
        }),
        Err(_) => Err(Error::DaemonError {
            message: format!("Connection to daemon timed out after {:?}", CONNECT_TIMEOUT),
        }),
    }
}

/// Send a request to the daemon with configurable timeout
async fn send_request_with_timeout(
    method: Method,
    request_timeout: Duration,
) -> Result<ResponseResult> {
    let stream = connect().await?;
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);

    let request = Request {
        id: Uuid::new_v4().to_string(),
        method,
    };

    let json = serde_json::to_string(&request)? + "\n";

    // Write with timeout
    match timeout(Duration::from_secs(5), writer.write_all(json.as_bytes())).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return Err(Error::DaemonError {
                message: format!("Failed to send request: {}", e),
            })
        }
        Err(_) => {
            return Err(Error::DaemonError {
                message: "Timed out sending request to daemon".to_string(),
            })
        }
    }

    // Read response with timeout
    let mut line = String::new();
    match timeout(request_timeout, reader.read_line(&mut line)).await {
        Ok(Ok(0)) => {
            return Err(Error::DaemonError {
                message: "Daemon closed connection unexpectedly".to_string(),
            })
        }
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            return Err(Error::DaemonError {
                message: format!("Failed to read response: {}", e),
            })
        }
        Err(_) => {
            return Err(Error::DaemonError {
                message: format!("Request timed out after {:?}", request_timeout),
            })
        }
    }

    let response: Response = serde_json::from_str(&line).map_err(|e| Error::DaemonError {
        message: format!("Invalid response from daemon: {}", e),
    })?;

    Ok(response.result)
}

/// Send a request to the daemon with default timeout
async fn send_request(method: Method) -> Result<ResponseResult> {
    send_request_with_timeout(method, REQUEST_TIMEOUT).await
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

/// Send an index request to the daemon (uses extended timeout)
pub async fn index(project: &Path, force: bool) -> Result<(usize, usize, f64)> {
    let method = Method::Index {
        project: project.to_string_lossy().to_string(),
        force,
    };

    // Use extended timeout for indexing operations
    match send_request_with_timeout(method, INDEX_TIMEOUT).await? {
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
