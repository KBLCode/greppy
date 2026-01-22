//! Server-Sent Events (SSE) endpoint for real-time updates
//!
//! Connects to the daemon via Unix socket (or TCP on Windows) to receive
//! events and forwards them to web clients via SSE.

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use serde::Serialize;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

// =============================================================================
// TYPES
// =============================================================================

/// State for the SSE endpoint
#[derive(Clone)]
pub struct EventsState {
    /// Broadcast sender for SSE events
    pub sender: Arc<broadcast::Sender<SseEvent>>,
    /// Project path being monitored
    pub project_path: PathBuf,
    /// Timestamp when index was last updated
    pub indexed_at: Arc<std::sync::atomic::AtomicU64>,
    /// Whether daemon is connected
    pub daemon_connected: Arc<std::sync::atomic::AtomicBool>,
}

impl EventsState {
    pub fn new(project_path: PathBuf) -> Self {
        let (sender, _) = broadcast::channel(256);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            sender: Arc::new(sender),
            project_path,
            indexed_at: Arc::new(std::sync::atomic::AtomicU64::new(now)),
            daemon_connected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Broadcast an event to all SSE clients
    pub fn broadcast(&self, event: SseEvent) -> usize {
        self.sender.send(event).unwrap_or(0)
    }

    /// Update the indexed_at timestamp to now
    pub fn update_indexed_at(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.indexed_at
            .store(now, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get the indexed_at timestamp
    pub fn get_indexed_at(&self) -> u64 {
        self.indexed_at.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Set daemon connection status
    pub fn set_daemon_connected(&self, connected: bool) {
        self.daemon_connected
            .store(connected, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get daemon connection status
    pub fn is_daemon_connected(&self) -> bool {
        self.daemon_connected
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Events sent via SSE to the browser
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum SseEvent {
    /// Initial connection event
    Connected {
        daemon: bool,
        indexed_at: u64,
        project: String,
    },
    /// Reindexing has started
    ReindexStart { files: usize, reason: String },
    /// Reindex progress update
    ReindexProgress { processed: usize, total: usize },
    /// Reindexing completed
    ReindexComplete {
        files: usize,
        symbols: usize,
        dead: usize,
        duration_ms: f64,
    },
    /// A single file changed
    FileChanged { path: String, action: String },
}

// =============================================================================
// SSE ENDPOINT
// =============================================================================

/// SSE endpoint handler - `/api/events`
pub async fn api_events(
    State(state): State<EventsState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sender.subscribe();
    let project_path = state.project_path.clone();
    let indexed_at = state.get_indexed_at();
    let daemon_connected = state.is_daemon_connected();

    // Convert broadcast receiver to a stream, filtering out errors
    let event_stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(event) => {
                let event_name = match &event {
                    SseEvent::Connected { .. } => "connected",
                    SseEvent::ReindexStart { .. } => "reindex-start",
                    SseEvent::ReindexProgress { .. } => "reindex-progress",
                    SseEvent::ReindexComplete { .. } => "reindex-complete",
                    SseEvent::FileChanged { .. } => "file-changed",
                };

                let data = serde_json::to_string(&event).unwrap_or_default();
                Some(Ok::<_, Infallible>(
                    Event::default().event(event_name).data(data),
                ))
            }
            Err(_) => None, // Skip lagged/closed errors
        }
    });

    // Create initial connection event
    let project_name = project_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let initial_event = SseEvent::Connected {
        daemon: daemon_connected,
        indexed_at,
        project: project_name,
    };

    let initial_data = serde_json::to_string(&initial_event).unwrap_or_default();
    let initial =
        futures::stream::once(
            async move { Ok(Event::default().event("connected").data(initial_data)) },
        );

    // Combine initial event with the event stream
    let combined_stream = initial.chain(event_stream);

    Sse::new(combined_stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

// =============================================================================
// DAEMON CONNECTION
// =============================================================================

/// Connect to the daemon and forward events to SSE clients
pub async fn start_daemon_event_forwarder(state: EventsState) {
    loop {
        // Try to connect to daemon
        let result = connect_to_daemon().await;

        match result {
            Ok(stream) => {
                state.set_daemon_connected(true);
                state.broadcast(SseEvent::Connected {
                    daemon: true,
                    indexed_at: state.get_indexed_at(),
                    project: state
                        .project_path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                });

                tracing::info!("Connected to daemon for event streaming");

                // Handle the connection
                if let Err(e) = handle_daemon_connection(stream, state.clone()).await {
                    tracing::warn!("Daemon connection error: {}", e);
                }
            }
            Err(e) => {
                tracing::debug!("Could not connect to daemon: {}", e);
            }
        }

        // Mark as disconnected
        state.set_daemon_connected(false);

        // Wait before retrying
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Connect to the daemon socket
#[cfg(unix)]
async fn connect_to_daemon() -> std::io::Result<tokio::net::UnixStream> {
    use crate::core::config::Config;

    let socket_path = Config::socket_path()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;

    if !socket_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Daemon socket not found",
        ));
    }

    tokio::net::UnixStream::connect(&socket_path).await
}

/// Connect to the daemon socket (Windows uses TCP)
#[cfg(windows)]
async fn connect_to_daemon() -> std::io::Result<tokio::net::TcpStream> {
    use crate::core::config::Config;

    let port_path = Config::port_path()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;

    if !port_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Daemon port file not found",
        ));
    }

    let port_str = std::fs::read_to_string(&port_path)?;
    let port: u16 = port_str
        .trim()
        .parse()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid port number"))?;

    let addr = format!("127.0.0.1:{}", port);
    tokio::net::TcpStream::connect(&addr).await
}

/// Handle the daemon connection, forwarding events to SSE
#[cfg(unix)]
async fn handle_daemon_connection(
    stream: tokio::net::UnixStream,
    state: EventsState,
) -> std::io::Result<()> {
    handle_daemon_stream(stream, state).await
}

#[cfg(windows)]
async fn handle_daemon_connection(
    stream: tokio::net::TcpStream,
    state: EventsState,
) -> std::io::Result<()> {
    handle_daemon_stream(stream, state).await
}

/// Generic stream handler for daemon events
async fn handle_daemon_stream<S>(stream: S, state: EventsState) -> std::io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use crate::daemon::events::DaemonEvent;
    use crate::daemon::protocol::{Method, Request, Response, ResponseResult};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);

    // Send Subscribe request
    let subscribe_request = Request {
        id: "web-events".to_string(),
        method: Method::Subscribe,
    };
    let json = serde_json::to_string(&subscribe_request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        + "\n";
    writer.write_all(json.as_bytes()).await?;

    // Read events
    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        if let Ok(response) = serde_json::from_str::<Response>(&line) {
            match response.result {
                ResponseResult::Subscribed => {
                    tracing::debug!("Subscribed to daemon events");
                }
                ResponseResult::Event(daemon_event) => {
                    // Convert daemon event to SSE event
                    let sse_event = match daemon_event {
                        DaemonEvent::FileChanged { path, action, .. } => {
                            Some(SseEvent::FileChanged {
                                path,
                                action: match action {
                                    crate::daemon::events::FileAction::Created => "created",
                                    crate::daemon::events::FileAction::Modified => "modified",
                                    crate::daemon::events::FileAction::Deleted => "deleted",
                                }
                                .to_string(),
                            })
                        }
                        DaemonEvent::ReindexStart { files, reason, .. } => {
                            Some(SseEvent::ReindexStart { files, reason })
                        }
                        DaemonEvent::ReindexProgress {
                            processed, total, ..
                        } => Some(SseEvent::ReindexProgress { processed, total }),
                        DaemonEvent::ReindexComplete {
                            files,
                            symbols,
                            dead,
                            duration_ms,
                            ..
                        } => {
                            // Update indexed_at timestamp
                            state.update_indexed_at();
                            Some(SseEvent::ReindexComplete {
                                files,
                                symbols,
                                dead,
                                duration_ms,
                            })
                        }
                        DaemonEvent::StatusUpdate { .. } => None,
                    };

                    if let Some(event) = sse_event {
                        state.broadcast(event);
                    }
                }
                ResponseResult::Error { message } => {
                    tracing::warn!("Daemon error: {}", message);
                }
                _ => {}
            }
        }
        line.clear();
    }

    Ok(())
}
