//! File system watcher for incremental indexing
//!
//! Watches project directories for file changes and triggers incremental
//! re-indexing when code files are created, modified, or deleted.

use crate::core::error::Result;
use crate::index::{IndexWriter, TantivyIndex};
use crate::parse::{chunk_file, is_code_file};
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind},
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Debounce duration for file changes (ms)
const DEBOUNCE_MS: u64 = 500;

/// Maximum batch size for incremental indexing
const MAX_BATCH_SIZE: usize = 100;

/// File watcher manager
pub struct FileWatcher {
    /// Active watchers by project path
    watchers: RwLock<HashMap<PathBuf, WatcherHandle>>,
    /// Sender for re-index requests
    reindex_tx: mpsc::Sender<ReindexRequest>,
}

struct WatcherHandle {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

/// Request to re-index files
#[derive(Debug)]
pub struct ReindexRequest {
    pub project_path: PathBuf,
    pub changed_files: Vec<PathBuf>,
    pub deleted_files: Vec<PathBuf>,
}

impl FileWatcher {
    /// Create a new file watcher manager
    pub fn new(reindex_tx: mpsc::Sender<ReindexRequest>) -> Self {
        Self {
            watchers: RwLock::new(HashMap::new()),
            reindex_tx,
        }
    }

    /// Start watching a project directory
    pub fn watch(&self, project_path: &Path) -> Result<()> {
        let project_path = project_path.to_path_buf();

        // Check if already watching
        {
            let watchers = self.watchers.read();
            if watchers.contains_key(&project_path) {
                debug!("Already watching {:?}", project_path);
                return Ok(());
            }
        }

        let tx = self.reindex_tx.clone();
        let path_clone = project_path.clone();

        // Create debounced event handler
        let debouncer = Arc::new(RwLock::new(DebouncedHandler::new(path_clone.clone(), tx)));
        let debouncer_clone = Arc::clone(&debouncer);

        // Create watcher
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    debouncer_clone.write().handle_event(event);
                }
                Err(e) => {
                    error!("Watch error: {:?}", e);
                }
            })
            .map_err(|e| crate::core::error::Error::DaemonError {
                message: format!("Failed to create watcher: {}", e),
            })?;

        // Configure watcher
        watcher
            .configure(NotifyConfig::default().with_poll_interval(Duration::from_secs(2)))
            .map_err(|e| crate::core::error::Error::DaemonError {
                message: format!("Failed to configure watcher: {}", e),
            })?;

        // Start watching
        watcher
            .watch(&project_path, RecursiveMode::Recursive)
            .map_err(|e| crate::core::error::Error::DaemonError {
                message: format!("Failed to watch {:?}: {}", project_path, e),
            })?;

        info!("Started watching {:?}", project_path);

        // Store watcher handle
        {
            let mut watchers = self.watchers.write();
            watchers.insert(project_path, WatcherHandle { watcher });
        }

        Ok(())
    }

    /// Stop watching a project directory
    pub fn unwatch(&self, project_path: &Path) {
        let mut watchers = self.watchers.write();
        if watchers.remove(project_path).is_some() {
            info!("Stopped watching {:?}", project_path);
        }
    }

    /// Check if a project is being watched
    pub fn is_watching(&self, project_path: &Path) -> bool {
        let watchers = self.watchers.read();
        watchers.contains_key(project_path)
    }

    /// Get list of watched projects
    pub fn watched_projects(&self) -> Vec<PathBuf> {
        let watchers = self.watchers.read();
        watchers.keys().cloned().collect()
    }
}

/// Debounced event handler to batch file changes
struct DebouncedHandler {
    project_path: PathBuf,
    tx: mpsc::Sender<ReindexRequest>,
    pending_changes: HashSet<PathBuf>,
    pending_deletes: HashSet<PathBuf>,
    last_event: Instant,
}

impl DebouncedHandler {
    fn new(project_path: PathBuf, tx: mpsc::Sender<ReindexRequest>) -> Self {
        Self {
            project_path,
            tx,
            pending_changes: HashSet::new(),
            pending_deletes: HashSet::new(),
            last_event: Instant::now(),
        }
    }

    fn handle_event(&mut self, event: Event) {
        let dominated_paths: Vec<PathBuf> = event
            .paths
            .iter()
            .filter(|p| p.is_file() || !p.exists()) // Include deleted files
            .filter(|p| is_code_file(p))
            .cloned()
            .collect();

        if dominated_paths.is_empty() {
            return;
        }

        match event.kind {
            EventKind::Create(CreateKind::File) | EventKind::Modify(ModifyKind::Data(_)) => {
                for path in dominated_paths {
                    debug!("File changed: {:?}", path);
                    self.pending_deletes.remove(&path);
                    self.pending_changes.insert(path);
                }
            }
            EventKind::Remove(RemoveKind::File) => {
                for path in dominated_paths {
                    debug!("File deleted: {:?}", path);
                    self.pending_changes.remove(&path);
                    self.pending_deletes.insert(path);
                }
            }
            EventKind::Modify(ModifyKind::Name(_)) => {
                // Rename events - treat as delete + create
                for path in dominated_paths {
                    if path.exists() {
                        self.pending_changes.insert(path);
                    } else {
                        self.pending_deletes.insert(path);
                    }
                }
            }
            _ => {}
        }

        self.last_event = Instant::now();
        self.maybe_flush();
    }

    fn maybe_flush(&mut self) {
        let elapsed = self.last_event.elapsed();
        let has_pending = !self.pending_changes.is_empty() || !self.pending_deletes.is_empty();
        let batch_full = self.pending_changes.len() + self.pending_deletes.len() >= MAX_BATCH_SIZE;

        if has_pending && (elapsed >= Duration::from_millis(DEBOUNCE_MS) || batch_full) {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.pending_changes.is_empty() && self.pending_deletes.is_empty() {
            return;
        }

        let request = ReindexRequest {
            project_path: self.project_path.clone(),
            changed_files: self.pending_changes.drain().collect(),
            deleted_files: self.pending_deletes.drain().collect(),
        };

        info!(
            "Flushing {} changed, {} deleted files for {:?}",
            request.changed_files.len(),
            request.deleted_files.len(),
            self.project_path
        );

        // Send non-blocking
        if let Err(e) = self.tx.try_send(request) {
            warn!("Failed to send reindex request: {}", e);
        }
    }
}

/// Process a reindex request (incremental indexing)
pub async fn process_reindex_request(
    request: ReindexRequest,
    embedder: Option<&crate::ai::embedding::Embedder>,
) -> Result<(usize, usize)> {
    let index = TantivyIndex::open_or_create(&request.project_path)?;
    let mut writer = IndexWriter::new(&index)?;

    let mut indexed_count = 0;
    let mut deleted_count = 0;

    // Delete removed files from index
    for path in &request.deleted_files {
        let path_str = path.to_string_lossy();
        if let Err(e) = writer.delete_by_path(&path_str) {
            warn!("Failed to delete {:?} from index: {}", path, e);
        } else {
            deleted_count += 1;
        }
    }

    // Index changed files
    for path in &request.changed_files {
        if !path.exists() {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read {:?}: {}", path, e);
                continue;
            }
        };

        // Delete old chunks for this file first
        let path_str = path.to_string_lossy();
        let _ = writer.delete_by_path(&path_str);

        // Chunk and index
        let chunks = chunk_file(path, &content);

        for chunk in chunks {
            let embedding = if let Some(emb) = embedder {
                let text = format!(
                    "{}: {}",
                    chunk.symbol_name.as_deref().unwrap_or("code"),
                    chunk.content
                );
                emb.embed(&text).ok()
            } else {
                None
            };

            writer.add_chunk(&chunk, embedding.as_deref())?;
            indexed_count += 1;
        }
    }

    writer.commit()?;

    info!(
        "Incremental index: {} chunks indexed, {} files deleted",
        indexed_count, deleted_count
    );

    Ok((indexed_count, deleted_count))
}

/// Spawn the reindex worker task
pub fn spawn_reindex_worker(
    mut rx: mpsc::Receiver<ReindexRequest>,
    embedder: Option<Arc<crate::ai::embedding::Embedder>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(request) = rx.recv().await {
            let emb_ref = embedder.as_deref();
            match process_reindex_request(request, emb_ref).await {
                Ok((indexed, deleted)) => {
                    debug!("Reindex complete: {} indexed, {} deleted", indexed, deleted);
                }
                Err(e) => {
                    error!("Reindex failed: {}", e);
                }
            }
        }
    })
}
