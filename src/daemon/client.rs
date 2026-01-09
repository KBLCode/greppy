use crate::config::Config;
use crate::daemon::protocol::{self, Request, Response};
use crate::error::{GreppyError, Result};
use tokio::net::UnixStream as TokioUnixStream;

/// Client for communicating with the daemon
pub struct DaemonClient {
    stream: TokioUnixStream,
}

impl DaemonClient {
    /// Connect to the daemon
    pub async fn connect() -> Result<Self> {
        let socket_path = Config::socket_path()?;

        if !socket_path.exists() {
            return Err(GreppyError::DaemonNotRunning);
        }

        let stream = TokioUnixStream::connect(&socket_path)
            .await
            .map_err(|_| GreppyError::DaemonNotRunning)?;

        Ok(Self { stream })
    }

    /// Send a request and wait for response (using binary protocol)
    pub async fn send(&mut self, request: Request) -> Result<Response> {
        // Write request using binary protocol (5-10x faster than JSON)
        protocol::write_message(&mut self.stream, &request).await?;

        // Read response using binary protocol
        let response: Response = protocol::read_message(&mut self.stream).await?;

        Ok(response)
    }

    /// Check if daemon is responsive
    pub async fn ping(&mut self) -> Result<bool> {
        let request = Request::ping();
        let response = self.send(request).await?;
        Ok(response.is_ok())
    }
}

// No Drop implementation needed - connection closes automatically when stream is dropped
