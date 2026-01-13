//! Client for communicating with daemon

use crate::core::config::Config;
use crate::core::error::{Error, Result};
use crate::daemon::protocol::{Method, Request, Response, ResponseResult};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(windows)]
use std::net::TcpStream;

/// Default timeout for daemon requests (30 seconds)
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

/// Connect to the daemon (Unix: Unix socket)
#[cfg(unix)]
fn connect_with_timeout(read_timeout: Duration) -> Result<UnixStream> {
    let socket_path = Config::socket_path()?;
    let stream = UnixStream::connect(&socket_path).map_err(|e| Error::DaemonError {
        message: format!("Failed to connect to daemon: {}", e),
    })?;
    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(|e| Error::DaemonError {
            message: format!("Failed to set read timeout: {}", e),
        })?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| Error::DaemonError {
            message: format!("Failed to set write timeout: {}", e),
        })?;
    Ok(stream)
}

/// Connect to the daemon (Windows: TCP on localhost)
#[cfg(windows)]
fn connect_with_timeout(read_timeout: Duration) -> Result<TcpStream> {
    let port_path = Config::port_path()?;
    let port_str = std::fs::read_to_string(&port_path).map_err(|e| Error::DaemonError {
        message: format!("Failed to read daemon port file: {}", e),
    })?;
    let port: u16 = port_str.trim().parse().map_err(|e| Error::DaemonError {
        message: format!("Invalid port in daemon port file: {}", e),
    })?;

    let addr = format!("127.0.0.1:{}", port);
    let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))
        .map_err(|e| Error::DaemonError {
            message: format!("Failed to connect to daemon at {}: {}", addr, e),
        })?;
    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(|e| Error::DaemonError {
            message: format!("Failed to set read timeout: {}", e),
        })?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| Error::DaemonError {
            message: format!("Failed to set write timeout: {}", e),
        })?;
    Ok(stream)
}

/// Send a request to the daemon
fn send_request<S: Write + std::io::Read>(stream: &mut S, request: &Request) -> Result<Response> {
    let json = serde_json::to_string(request).map_err(|e| Error::DaemonError {
        message: format!("Failed to serialize request: {}", e),
    })?;

    stream
        .write_all(json.as_bytes())
        .map_err(|e| Error::DaemonError {
            message: format!("Failed to send request: {}", e),
        })?;
    stream.write_all(b"\n").map_err(|e| Error::DaemonError {
        message: format!("Failed to send newline: {}", e),
    })?;
    stream.flush().map_err(|e| Error::DaemonError {
        message: format!("Failed to flush: {}", e),
    })?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| Error::DaemonError {
            message: format!("Failed to read response: {}", e),
        })?;

    serde_json::from_str(&response_line).map_err(|e| Error::DaemonError {
        message: format!("Invalid response from daemon: {}", e),
    })
}

/// Send a search request to the daemon
pub async fn search(
    query: &str,
    project: &Path,
    limit: usize,
) -> Result<crate::search::SearchResponse> {
    let mut stream = connect_with_timeout(REQUEST_TIMEOUT)?;

    let request = Request {
        id: uuid::Uuid::new_v4().to_string(),
        method: Method::Search {
            query: query.to_string(),
            project: project.to_string_lossy().to_string(),
            limit,
        },
    };

    let response = send_request(&mut stream, &request)?;

    match response.result {
        ResponseResult::Search(search_response) => Ok(search_response),
        ResponseResult::Error { message } => Err(Error::DaemonError { message }),
        _ => Err(Error::DaemonError {
            message: "Unexpected response type".to_string(),
        }),
    }
}

/// Send an index request to the daemon (uses extended timeout)
pub async fn index(project: &Path, force: bool) -> Result<(usize, usize, f64)> {
    let mut stream = connect_with_timeout(INDEX_TIMEOUT)?;

    let request = Request {
        id: uuid::Uuid::new_v4().to_string(),
        method: Method::Index {
            project: project.to_string_lossy().to_string(),
            force,
        },
    };

    let response = send_request(&mut stream, &request)?;

    match response.result {
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

/// Send a stop request to the daemon
pub async fn stop() -> Result<bool> {
    let mut stream = connect_with_timeout(REQUEST_TIMEOUT)?;

    let request = Request {
        id: uuid::Uuid::new_v4().to_string(),
        method: Method::Stop,
    };

    let response = send_request(&mut stream, &request)?;

    match response.result {
        ResponseResult::Stop { success } => Ok(success),
        ResponseResult::Error { message } => Err(Error::DaemonError { message }),
        _ => Err(Error::DaemonError {
            message: "Unexpected response type".to_string(),
        }),
    }
}

/// Get daemon status
pub async fn status() -> Result<(bool, u32, Vec<crate::daemon::protocol::ProjectInfo>)> {
    let mut stream = connect_with_timeout(REQUEST_TIMEOUT)?;

    let request = Request {
        id: uuid::Uuid::new_v4().to_string(),
        method: Method::Status,
    };

    let response = send_request(&mut stream, &request)?;

    match response.result {
        ResponseResult::Status {
            running,
            pid,
            projects,
        } => Ok((running, pid, projects)),
        ResponseResult::Error { message } => Err(Error::DaemonError { message }),
        _ => Err(Error::DaemonError {
            message: "Unexpected response type".to_string(),
        }),
    }
}
