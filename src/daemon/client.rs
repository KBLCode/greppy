//! Client for communicating with daemon

use crate::core::config::Config;
use crate::core::error::{Error, Result};
use std::path::Path;

/// Check if daemon is running
pub fn is_running() -> Result<bool> {
    let socket_path = Config::socket_path()?;
    Ok(socket_path.exists())
}

/// Send a search request to the daemon
pub fn search(_query: &str, _project: &Path) -> Result<String> {
    // TODO: Implement daemon client
    Err(Error::DaemonNotRunning)
}
