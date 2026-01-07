use crate::config::Config;
use crate::daemon::protocol::{Request, Response};
use crate::error::{GreppyError, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Client for communicating with the daemon
pub struct DaemonClient {
    stream: UnixStream,
}

impl DaemonClient {
    /// Connect to the daemon
    pub fn connect() -> Result<Self> {
        let socket_path = Config::socket_path()?;
        
        if !socket_path.exists() {
            return Err(GreppyError::DaemonNotRunning);
        }

        let stream = UnixStream::connect(&socket_path)
            .map_err(|_| GreppyError::DaemonNotRunning)?;
        
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self { stream })
    }

    /// Send a request and wait for response
    pub fn send(&mut self, request: Request) -> Result<Response> {
        let json = serde_json::to_string(&request)? + "\n";
        self.stream.write_all(json.as_bytes())?;
        self.stream.flush()?;

        let mut reader = BufReader::new(&self.stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;

        if line.is_empty() {
            return Err(GreppyError::Protocol("Empty response from daemon".to_string()));
        }

        let response: Response = serde_json::from_str(&line)?;
        Ok(response)
    }

    /// Check if daemon is responsive
    pub fn ping(&mut self) -> Result<bool> {
        let request = Request::ping();
        let response = self.send(request)?;
        Ok(response.is_ok())
    }
}
