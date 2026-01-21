//! File system watcher for incremental indexing
//!
//! Watches project directories for changes and incrementally updates the index.
//! - Create/Modify: Re-index the changed file
//! - Delete: Remove file's chunks from index
//! - Debounced: Waits for activity to settle before processing
//!
//! Design: Non-blocking, runs in background task, doesn't affect search performance.

use crate::core::error::{Error, Result};
use crate::index::{IndexWriter, TantivyIndex};
use crate::parse::chunk_file;
use crate::trace::builder::{remove_file_from_index, update_file_incremental};
use crate::trace::storage::{load_index, save_index, trace_index_path};
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind},
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Debounce delay - wait this long after last event before processing
const DEBOUNCE_MS: u64 = 500;

/// Events that trigger re-indexing
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// File was created or modified - needs (re)indexing
    Changed(PathBuf),
    /// File was deleted - needs removal from index
    Deleted(PathBuf),
}

/// Manages file watchers for multiple projects
pub struct WatcherManager {
    /// Active watchers by project path
    watchers: HashMap<PathBuf, ProjectWatcher>,
    /// Channel to receive aggregated events (std::sync for use in blocking context)
    event_tx: std_mpsc::Sender<(PathBuf, FileEvent)>,
    event_rx: std_mpsc::Receiver<(PathBuf, FileEvent)>,
}

impl WatcherManager {
    pub fn new() -> Self {
        let (event_tx, event_rx) = std_mpsc::channel();
        Self {
            watchers: HashMap::new(),
            event_tx,
            event_rx,
        }
    }

    /// Start watching a project directory
    pub fn watch(&mut self, project_path: PathBuf) -> Result<()> {
        if self.watchers.contains_key(&project_path) {
            debug!(project = %project_path.display(), "Already watching");
            return Ok(());
        }

        let watcher = ProjectWatcher::new(project_path.clone(), self.event_tx.clone())?;
        self.watchers.insert(project_path.clone(), watcher);
        info!(project = %project_path.display(), "Started watching");
        Ok(())
    }

    /// Stop watching a project directory
    pub fn unwatch(&mut self, project_path: &Path) {
        if self.watchers.remove(project_path).is_some() {
            info!(project = %project_path.display(), "Stopped watching");
        }
    }

    /// Process pending events synchronously (for use in spawn_blocking)
    /// Returns list of projects that were updated
    pub fn process_events_sync(&mut self) -> Vec<PathBuf> {
        let mut pending: HashMap<PathBuf, Vec<FileEvent>> = HashMap::new();
        let debounce = Duration::from_millis(DEBOUNCE_MS);

        // Collect events with timeout (non-blocking drain)
        loop {
            match self.event_rx.recv_timeout(debounce) {
                Ok((project, event)) => {
                    pending.entry(project).or_default().push(event);
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {
                    // Debounce complete, process what we have
                    if !pending.is_empty() {
                        break;
                    }
                    // Nothing pending, return empty
                    return Vec::new();
                }
                Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        // Process each project's events
        let mut updated = Vec::new();
        for (project_path, events) in pending {
            if let Err(e) = process_project_events_sync(&project_path, events) {
                warn!(project = %project_path.display(), error = %e, "Failed to process events");
            } else {
                updated.push(project_path);
            }
        }

        updated
    }
}

impl Default for WatcherManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Watcher for a single project
struct ProjectWatcher {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    #[allow(dead_code)]
    project_path: PathBuf,
}

impl ProjectWatcher {
    fn new(
        project_path: PathBuf,
        event_tx: std_mpsc::Sender<(PathBuf, FileEvent)>,
    ) -> Result<Self> {
        let project_path_clone = project_path.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if let Some(file_event) = classify_event(&event) {
                    // Send event - ignore errors if channel is full/closed
                    let _ = event_tx.send((project_path_clone.clone(), file_event));
                }
            }
        })
        .map_err(|e| Error::WatchError {
            message: e.to_string(),
        })?;

        watcher
            .watch(&project_path, RecursiveMode::Recursive)
            .map_err(|e| Error::WatchError {
                message: e.to_string(),
            })?;

        Ok(Self {
            watcher,
            project_path,
        })
    }
}

/// Classify a notify event into our FileEvent type
fn classify_event(event: &Event) -> Option<FileEvent> {
    // Only care about files, not directories
    let paths: Vec<_> = event
        .paths
        .iter()
        .filter(|p| p.is_file() || !p.exists()) // Include deleted files
        .filter(|p| is_indexable_file(p))
        .cloned()
        .collect();

    if paths.is_empty() {
        return None;
    }

    let path = paths.into_iter().next()?;

    match &event.kind {
        EventKind::Create(CreateKind::File) => Some(FileEvent::Changed(path)),
        EventKind::Modify(ModifyKind::Data(_)) => Some(FileEvent::Changed(path)),
        EventKind::Modify(ModifyKind::Name(_)) => Some(FileEvent::Changed(path)),
        EventKind::Remove(RemoveKind::File) => Some(FileEvent::Deleted(path)),
        _ => None,
    }
}

/// Check if a file should be indexed
fn is_indexable_file(path: &Path) -> bool {
    // Skip hidden files and directories
    if path
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return false;
    }

    // Check extension
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(
        ext.as_str(),
        "ts" | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "py"
            | "pyi"
            | "rs"
            | "go"
            | "java"
            | "kt"
            | "kts"
            | "scala"
            | "rb"
            | "php"
            | "c"
            | "h"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "cs"
            | "swift"
            | "ex"
            | "exs"
            | "erl"
            | "hrl"
            | "hs"
            | "ml"
            | "mli"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "sql"
            | "vue"
            | "svelte"
    )
}

/// Process accumulated events for a project (synchronous version)
fn process_project_events_sync(project_path: &Path, events: Vec<FileEvent>) -> Result<()> {
    // Deduplicate events - if a file was changed multiple times, only process once
    let mut to_reindex: HashSet<PathBuf> = HashSet::new();
    let mut to_delete: HashSet<PathBuf> = HashSet::new();

    for event in events {
        match event {
            FileEvent::Changed(path) => {
                to_delete.remove(&path); // Changed overrides delete
                to_reindex.insert(path);
            }
            FileEvent::Deleted(path) => {
                to_reindex.remove(&path); // Delete overrides change
                to_delete.insert(path);
            }
        }
    }

    if to_reindex.is_empty() && to_delete.is_empty() {
        return Ok(());
    }

    info!(
        project = %project_path.display(),
        reindex = to_reindex.len(),
        delete = to_delete.len(),
        "Processing file changes"
    );

    // Update Tantivy text index
    update_tantivy_index(project_path, &to_reindex, &to_delete)?;

    // Update trace semantic index
    update_trace_index(project_path, &to_reindex, &to_delete);

    info!(project = %project_path.display(), "Incremental index update complete");
    Ok(())
}

/// Update the Tantivy text search index
fn update_tantivy_index(
    project_path: &Path,
    to_reindex: &HashSet<PathBuf>,
    to_delete: &HashSet<PathBuf>,
) -> Result<()> {
    let index = TantivyIndex::open_or_create(project_path)?;
    let mut writer = IndexWriter::new(&index)?;

    // Delete old chunks for files that changed or were deleted
    let all_paths: Vec<_> = to_reindex.iter().chain(to_delete.iter()).collect();
    for path in &all_paths {
        let path_str = path.to_string_lossy();
        writer.delete_by_path(&path_str)?;
    }

    // Re-index changed files
    for path in to_reindex {
        if let Ok(content) = std::fs::read_to_string(path) {
            let chunks = chunk_file(path, &content);
            for chunk in &chunks {
                writer.add_chunk(chunk)?;
            }
            debug!(path = %path.display(), chunks = chunks.len(), "Re-indexed file (tantivy)");
        }
    }

    // Commit changes
    writer.commit()?;

    Ok(())
}

/// Update the trace semantic index
///
/// This loads the existing trace index (if any), applies incremental updates,
/// and saves the updated index back to disk.
fn update_trace_index(
    project_path: &Path,
    to_reindex: &HashSet<PathBuf>,
    to_delete: &HashSet<PathBuf>,
) {
    let trace_path = trace_index_path(project_path);

    // Try to load existing trace index
    let mut index = match load_index(&trace_path) {
        Ok(idx) => idx,
        Err(e) => {
            // No trace index exists yet - this is fine, trace may not have been built
            debug!(
                project = %project_path.display(),
                error = %e,
                "No trace index to update (will be created on next full index)"
            );
            return;
        }
    };

    let start = std::time::Instant::now();
    let mut files_updated = 0;
    let mut files_deleted = 0;

    // Process deletions first
    for path in to_delete {
        let removed = remove_file_from_index(&mut index, project_path, path);
        if removed > 0 {
            files_deleted += 1;
            debug!(
                path = %path.display(),
                symbols_removed = removed,
                "Removed from trace index"
            );
        }
    }

    // Process updates/additions
    for path in to_reindex {
        if let Ok(content) = std::fs::read_to_string(path) {
            let result = update_file_incremental(&mut index, project_path, path, &content);
            files_updated += 1;
            debug!(
                path = %path.display(),
                symbols_added = result.symbols_added,
                elapsed_ms = result.elapsed_ms,
                "Updated in trace index"
            );
        }
    }

    // Save the updated index
    if files_updated > 0 || files_deleted > 0 {
        if let Err(e) = save_index(&index, &trace_path) {
            warn!(
                project = %project_path.display(),
                error = %e,
                "Failed to save trace index"
            );
        } else {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            info!(
                project = %project_path.display(),
                files_updated = files_updated,
                files_deleted = files_deleted,
                elapsed_ms = elapsed_ms,
                "Trace index updated"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_indexable_file() {
        assert!(is_indexable_file(Path::new("src/main.rs")));
        assert!(is_indexable_file(Path::new("app.tsx")));
        assert!(!is_indexable_file(Path::new(".git/config")));
        assert!(!is_indexable_file(Path::new("image.png")));
    }
}
