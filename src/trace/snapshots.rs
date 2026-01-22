//! Snapshot Management for Timeline Tracking
//!
//! Stores index snapshots over time to track codebase evolution.
//! Snapshots contain summary metrics, not full symbol data.
//!
//! Storage: `.greppy/snapshots/{timestamp}.json`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::trace::SemanticIndex;

// Re-export DateTime for API responses
pub use chrono::{DateTime, Utc};

// =============================================================================
// TYPES
// =============================================================================

/// Snapshot metadata and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique identifier (timestamp-based)
    pub id: String,
    /// Optional user-provided name (e.g., "v1.0.0")
    pub name: Option<String>,
    /// When the snapshot was created
    pub created_at: DateTime<Utc>,
    /// Project name
    pub project: String,
    /// Summary metrics
    pub metrics: SnapshotMetrics,
}

/// Summary metrics for a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetrics {
    /// Total number of files
    pub files: u32,
    /// Total number of symbols
    pub symbols: u32,
    /// Number of dead (unreferenced) symbols
    pub dead: u32,
    /// Number of symbols in circular dependencies
    pub cycles: u32,
    /// Number of entry points
    pub entry_points: u32,
    /// Symbols by kind (function, method, class, etc.)
    pub by_kind: HashMap<String, u32>,
    /// Top files by symbol count
    pub top_files: Vec<FileMetrics>,
}

/// Per-file metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetrics {
    /// File path
    pub path: String,
    /// Number of symbols in file
    pub symbols: u32,
    /// Number of dead symbols in file
    pub dead: u32,
}

/// Comparison between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotComparison {
    /// First snapshot (older)
    pub a: SnapshotSummary,
    /// Second snapshot (newer)
    pub b: SnapshotSummary,
    /// Differences
    pub diff: SnapshotDiff,
}

/// Minimal snapshot summary for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub id: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub files: u32,
    pub symbols: u32,
    pub dead: u32,
    pub cycles: u32,
}

/// Diff between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub files: i32,
    pub symbols: i32,
    pub dead: i32,
    pub cycles: i32,
    pub entry_points: i32,
}

/// List response for snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotList {
    pub snapshots: Vec<SnapshotSummary>,
    pub total: usize,
}

// =============================================================================
// STORAGE PATH
// =============================================================================

/// Get the snapshots directory path.
pub fn snapshots_dir(project_path: &Path) -> PathBuf {
    project_path.join(".greppy").join("snapshots")
}

/// Get the path for a specific snapshot.
fn snapshot_path(project_path: &Path, id: &str) -> PathBuf {
    snapshots_dir(project_path).join(format!("{}.json", id))
}

/// Generate a snapshot ID from current timestamp.
fn generate_id() -> String {
    Utc::now().format("%Y%m%d_%H%M%S").to_string()
}

// =============================================================================
// SNAPSHOT CREATION
// =============================================================================

/// Create a snapshot from the current index state.
///
/// # Arguments
/// * `index` - The semantic index to snapshot
/// * `project_path` - Path to the project root
/// * `project_name` - Name of the project
/// * `dead_symbols` - Set of dead symbol IDs
/// * `cycles_count` - Number of symbols in cycles
/// * `name` - Optional user-provided name
///
/// # Returns
/// The created snapshot, or an error.
pub fn create_snapshot(
    index: &SemanticIndex,
    project_path: &Path,
    project_name: &str,
    dead_symbols: &std::collections::HashSet<u32>,
    cycles_count: u32,
    name: Option<String>,
) -> Result<Snapshot, String> {
    let id = generate_id();
    let now = Utc::now();

    // Count symbols by kind
    let mut by_kind: HashMap<String, u32> = HashMap::new();
    for sym in &index.symbols {
        let kind = format!("{:?}", sym.kind).to_lowercase();
        *by_kind.entry(kind).or_insert(0) += 1;
    }

    // Count entry points
    let entry_points = index.symbols.iter().filter(|s| s.is_entry_point()).count() as u32;

    // Calculate per-file metrics
    let mut file_map: HashMap<u16, (u32, u32)> = HashMap::new(); // file_id -> (symbols, dead)
    for sym in &index.symbols {
        let entry = file_map.entry(sym.file_id).or_insert((0, 0));
        entry.0 += 1;
        if dead_symbols.contains(&sym.id) {
            entry.1 += 1;
        }
    }

    // Get top 20 files by symbol count
    // files is Vec<PathBuf>, so we can convert directly
    let mut top_files: Vec<FileMetrics> = file_map
        .iter()
        .filter_map(|(file_id, (symbols, dead))| {
            index
                .files
                .get(*file_id as usize)
                .map(|path_buf| FileMetrics {
                    path: path_buf.to_string_lossy().to_string(),
                    symbols: *symbols,
                    dead: *dead,
                })
        })
        .collect();
    top_files.sort_by(|a, b| b.symbols.cmp(&a.symbols));
    top_files.truncate(20);

    let snapshot = Snapshot {
        id: id.clone(),
        name,
        created_at: now,
        project: project_name.to_string(),
        metrics: SnapshotMetrics {
            files: index.files.len() as u32,
            symbols: index.symbols.len() as u32,
            dead: dead_symbols.len() as u32,
            cycles: cycles_count,
            entry_points,
            by_kind,
            top_files,
        },
    };

    // Ensure directory exists
    let dir = snapshots_dir(project_path);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create snapshots dir: {}", e))?;

    // Save snapshot
    let path = snapshot_path(project_path, &id);
    let json = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write snapshot: {}", e))?;

    Ok(snapshot)
}

// =============================================================================
// SNAPSHOT LISTING
// =============================================================================

/// List all snapshots for a project.
///
/// # Arguments
/// * `project_path` - Path to the project root
///
/// # Returns
/// List of snapshot summaries, sorted by creation time (newest first).
pub fn list_snapshots(project_path: &Path) -> Result<SnapshotList, String> {
    let dir = snapshots_dir(project_path);

    if !dir.exists() {
        return Ok(SnapshotList {
            snapshots: vec![],
            total: 0,
        });
    }

    let mut snapshots: Vec<SnapshotSummary> = vec![];

    for entry in fs::read_dir(&dir).map_err(|e| format!("Failed to read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&content) {
                    snapshots.push(SnapshotSummary {
                        id: snapshot.id,
                        name: snapshot.name,
                        created_at: snapshot.created_at,
                        files: snapshot.metrics.files,
                        symbols: snapshot.metrics.symbols,
                        dead: snapshot.metrics.dead,
                        cycles: snapshot.metrics.cycles,
                    });
                }
            }
        }
    }

    // Sort by creation time, newest first
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = snapshots.len();
    Ok(SnapshotList { snapshots, total })
}

// =============================================================================
// SNAPSHOT RETRIEVAL
// =============================================================================

/// Load a specific snapshot by ID.
///
/// # Arguments
/// * `project_path` - Path to the project root
/// * `id` - Snapshot ID
///
/// # Returns
/// The snapshot, or an error if not found.
pub fn load_snapshot(project_path: &Path, id: &str) -> Result<Snapshot, String> {
    let path = snapshot_path(project_path, id);

    if !path.exists() {
        return Err(format!("Snapshot not found: {}", id));
    }

    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse: {}", e))
}

/// Get the latest snapshot.
///
/// # Arguments
/// * `project_path` - Path to the project root
///
/// # Returns
/// The latest snapshot, or None if no snapshots exist.
pub fn latest_snapshot(project_path: &Path) -> Result<Option<Snapshot>, String> {
    let list = list_snapshots(project_path)?;
    if list.snapshots.is_empty() {
        return Ok(None);
    }

    let latest = &list.snapshots[0];
    load_snapshot(project_path, &latest.id).map(Some)
}

// =============================================================================
// SNAPSHOT COMPARISON
// =============================================================================

/// Compare two snapshots.
///
/// # Arguments
/// * `project_path` - Path to the project root
/// * `id_a` - First snapshot ID (older)
/// * `id_b` - Second snapshot ID (newer)
///
/// # Returns
/// Comparison result with diffs.
pub fn compare_snapshots(
    project_path: &Path,
    id_a: &str,
    id_b: &str,
) -> Result<SnapshotComparison, String> {
    let a = load_snapshot(project_path, id_a)?;
    let b = load_snapshot(project_path, id_b)?;

    let diff = SnapshotDiff {
        files: b.metrics.files as i32 - a.metrics.files as i32,
        symbols: b.metrics.symbols as i32 - a.metrics.symbols as i32,
        dead: b.metrics.dead as i32 - a.metrics.dead as i32,
        cycles: b.metrics.cycles as i32 - a.metrics.cycles as i32,
        entry_points: b.metrics.entry_points as i32 - a.metrics.entry_points as i32,
    };

    Ok(SnapshotComparison {
        a: SnapshotSummary {
            id: a.id,
            name: a.name,
            created_at: a.created_at,
            files: a.metrics.files,
            symbols: a.metrics.symbols,
            dead: a.metrics.dead,
            cycles: a.metrics.cycles,
        },
        b: SnapshotSummary {
            id: b.id,
            name: b.name,
            created_at: b.created_at,
            files: b.metrics.files,
            symbols: b.metrics.symbols,
            dead: b.metrics.dead,
            cycles: b.metrics.cycles,
        },
        diff,
    })
}

// =============================================================================
// SNAPSHOT DELETION
// =============================================================================

/// Delete a snapshot by ID.
///
/// # Arguments
/// * `project_path` - Path to the project root
/// * `id` - Snapshot ID
///
/// # Returns
/// Ok if deleted, Err if not found.
pub fn delete_snapshot(project_path: &Path, id: &str) -> Result<(), String> {
    let path = snapshot_path(project_path, id);

    if !path.exists() {
        return Err(format!("Snapshot not found: {}", id));
    }

    fs::remove_file(&path).map_err(|e| format!("Failed to delete: {}", e))
}

// =============================================================================
// CLEANUP
// =============================================================================

/// Clean up old snapshots, keeping only the most recent N.
///
/// # Arguments
/// * `project_path` - Path to the project root
/// * `keep` - Number of snapshots to keep
///
/// # Returns
/// Number of snapshots deleted.
pub fn cleanup_snapshots(project_path: &Path, keep: usize) -> Result<usize, String> {
    let list = list_snapshots(project_path)?;

    if list.total <= keep {
        return Ok(0);
    }

    let mut deleted = 0;
    for snapshot in list.snapshots.iter().skip(keep) {
        if delete_snapshot(project_path, &snapshot.id).is_ok() {
            deleted += 1;
        }
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id = generate_id();
        assert!(!id.is_empty());
        // Format: YYYYMMDD_HHMMSS
        assert_eq!(id.len(), 15);
        assert!(id.contains('_'));
    }
}
