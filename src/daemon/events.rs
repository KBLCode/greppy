//! Event broadcasting for daemon
//!
//! Provides a broadcast channel for daemon events that can be subscribed to
//! by clients (like the web server) for real-time updates.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::broadcast;

// =============================================================================
// EVENT TYPES
// =============================================================================

/// Events emitted by the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum DaemonEvent {
    /// File changed in a watched project
    FileChanged {
        project: String,
        path: String,
        action: FileAction,
    },
    /// Reindexing has started
    ReindexStart {
        project: String,
        files: usize,
        reason: String,
    },
    /// Progress update during reindexing
    ReindexProgress {
        project: String,
        processed: usize,
        total: usize,
    },
    /// Reindexing completed
    ReindexComplete {
        project: String,
        files: usize,
        symbols: usize,
        dead: usize,
        duration_ms: f64,
    },
    /// Daemon status update
    StatusUpdate { projects: usize, watching: usize },
}

/// File change action type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileAction {
    Created,
    Modified,
    Deleted,
}

// =============================================================================
// EVENT BROADCASTER
// =============================================================================

/// Broadcasts events to all subscribers
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: broadcast::Sender<DaemonEvent>,
}

impl EventBroadcaster {
    /// Create a new event broadcaster with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast an event to all subscribers
    /// Returns the number of receivers that received the event
    pub fn broadcast(&self, event: DaemonEvent) -> usize {
        // send() returns Err if there are no receivers, which is fine
        self.sender.send(event).unwrap_or(0)
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonEvent> {
        self.sender.subscribe()
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    // ==========================================================================
    // CONVENIENCE METHODS
    // ==========================================================================

    /// Emit a file changed event
    pub fn file_changed(&self, project: &PathBuf, path: &PathBuf, action: FileAction) {
        self.broadcast(DaemonEvent::FileChanged {
            project: project.to_string_lossy().to_string(),
            path: path.to_string_lossy().to_string(),
            action,
        });
    }

    /// Emit a reindex start event
    pub fn reindex_start(&self, project: &PathBuf, files: usize, reason: &str) {
        self.broadcast(DaemonEvent::ReindexStart {
            project: project.to_string_lossy().to_string(),
            files,
            reason: reason.to_string(),
        });
    }

    /// Emit a reindex progress event
    pub fn reindex_progress(&self, project: &PathBuf, processed: usize, total: usize) {
        self.broadcast(DaemonEvent::ReindexProgress {
            project: project.to_string_lossy().to_string(),
            processed,
            total,
        });
    }

    /// Emit a reindex complete event
    pub fn reindex_complete(
        &self,
        project: &PathBuf,
        files: usize,
        symbols: usize,
        dead: usize,
        duration_ms: f64,
    ) {
        self.broadcast(DaemonEvent::ReindexComplete {
            project: project.to_string_lossy().to_string(),
            files,
            symbols,
            dead,
            duration_ms,
        });
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(256) // Default capacity of 256 events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_broadcaster() {
        let broadcaster = EventBroadcaster::new(16);
        let mut rx = broadcaster.subscribe();

        broadcaster.broadcast(DaemonEvent::StatusUpdate {
            projects: 1,
            watching: 1,
        });

        let event = rx.recv().await.unwrap();
        match event {
            DaemonEvent::StatusUpdate { projects, watching } => {
                assert_eq!(projects, 1);
                assert_eq!(watching, 1);
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[test]
    fn test_no_subscribers() {
        let broadcaster = EventBroadcaster::new(16);
        // Should not panic even with no subscribers
        let count = broadcaster.broadcast(DaemonEvent::StatusUpdate {
            projects: 0,
            watching: 0,
        });
        assert_eq!(count, 0);
    }
}
