//! Project selector API endpoints
//!
//! Provides endpoints for listing and switching between indexed projects.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::core::config::Config;
use crate::trace::trace_index_exists;

// =============================================================================
// TYPES
// =============================================================================

/// Project information for the selector dropdown
#[derive(Serialize, Clone)]
pub struct ProjectInfo {
    /// Display name (folder name)
    pub name: String,
    /// Full path to project
    pub path: String,
    /// Whether this is the currently active project
    pub active: bool,
    /// Whether the index exists and is valid
    pub indexed: bool,
}

/// Response for GET /api/projects
#[derive(Serialize)]
pub struct ProjectsResponse {
    pub projects: Vec<ProjectInfo>,
}

/// Request body for POST /api/projects/switch
#[derive(Deserialize)]
pub struct SwitchProjectRequest {
    pub path: String,
}

/// Response for POST /api/projects/switch
#[derive(Serialize)]
pub struct SwitchProjectResponse {
    pub success: bool,
    pub message: String,
}

/// Shared state for project management
#[derive(Clone)]
pub struct ProjectsState {
    /// Currently active project path
    pub active_path: Arc<RwLock<PathBuf>>,
}

// =============================================================================
// HELPERS
// =============================================================================

/// Common locations to scan for .greppy directories
fn get_scan_locations() -> Vec<PathBuf> {
    let mut locations = Vec::new();

    // Home directory
    if let Some(home) = dirs::home_dir() {
        locations.push(home.clone());
        locations.push(home.join("Desktop"));
        locations.push(home.join("Documents"));
        locations.push(home.join("projects"));
        locations.push(home.join("Projects"));
        locations.push(home.join("code"));
        locations.push(home.join("Code"));
        locations.push(home.join("dev"));
        locations.push(home.join("Dev"));
        locations.push(home.join("src"));
        locations.push(home.join("work"));
        locations.push(home.join("Work"));
        locations.push(home.join("repos"));
        locations.push(home.join("Repos"));
    }

    // Current directory
    if let Ok(cwd) = std::env::current_dir() {
        locations.push(cwd);
    }

    locations
}

/// Scan a directory for projects with .greppy index
fn scan_for_projects(dir: &PathBuf, max_depth: usize, found: &mut HashSet<PathBuf>) {
    if max_depth == 0 || !dir.is_dir() {
        return;
    }

    // Check if this directory has a .greppy folder
    let greppy_dir = dir.join(".greppy");
    if greppy_dir.exists() && greppy_dir.is_dir() {
        found.insert(dir.clone());
    }

    // Scan subdirectories (skip hidden and common non-project dirs)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Skip hidden directories and common non-project directories
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "dist"
                || name == "build"
                || name == "__pycache__"
                || name == "venv"
                || name == ".venv"
                || name == "vendor"
            {
                continue;
            }

            scan_for_projects(&path, max_depth - 1, found);
        }
    }
}

/// Find all indexed projects
pub fn discover_projects(active_path: &PathBuf) -> Vec<ProjectInfo> {
    let mut found = HashSet::new();

    // Scan common locations
    for location in get_scan_locations() {
        if location.exists() {
            scan_for_projects(&location, 3, &mut found);
        }
    }

    // Also check greppy's registry if it exists
    if let Ok(registry_path) = Config::registry_path() {
        if registry_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&registry_path) {
                if let Ok(registry) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(projects) = registry.get("projects").and_then(|p| p.as_array()) {
                        for project in projects {
                            if let Some(path) = project.get("path").and_then(|p| p.as_str()) {
                                let path_buf = PathBuf::from(path);
                                if path_buf.exists() {
                                    found.insert(path_buf);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Convert to ProjectInfo
    let mut projects: Vec<ProjectInfo> = found
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let is_active = path == *active_path;
            let indexed = trace_index_exists(&path);

            ProjectInfo {
                name,
                path: path.to_string_lossy().to_string(),
                active: is_active,
                indexed,
            }
        })
        .collect();

    // Sort: active first, then alphabetically by name
    projects.sort_by(|a, b| {
        if a.active && !b.active {
            std::cmp::Ordering::Less
        } else if !a.active && b.active {
            std::cmp::Ordering::Greater
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    projects
}

// =============================================================================
// ROUTE HANDLERS
// =============================================================================

/// GET /api/projects - List all discovered projects
pub async fn api_projects(State(state): State<ProjectsState>) -> Json<ProjectsResponse> {
    let active_path = state.active_path.read().unwrap().clone();
    let projects = discover_projects(&active_path);
    Json(ProjectsResponse { projects })
}

/// POST /api/projects/switch - Switch to a different project
///
/// Note: This endpoint signals that a switch is requested. The actual switch
/// requires reloading the server with the new project, which the frontend
/// handles by triggering a page reload.
pub async fn api_switch_project(
    State(state): State<ProjectsState>,
    Json(request): Json<SwitchProjectRequest>,
) -> impl IntoResponse {
    let path = PathBuf::from(&request.path);

    // Validate the project exists
    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(SwitchProjectResponse {
                success: false,
                message: format!("Project not found: {}", request.path),
            }),
        );
    }

    // Check if it has an index
    if !trace_index_exists(&path) {
        return (
            StatusCode::BAD_REQUEST,
            Json(SwitchProjectResponse {
                success: false,
                message: format!(
                    "Project not indexed: {}. Run 'greppy index' first.",
                    request.path
                ),
            }),
        );
    }

    // Update the active path
    {
        let mut active = state.active_path.write().unwrap();
        *active = path;
    }

    // Save to recent projects file
    let _ = save_recent_project(&request.path);

    (
        StatusCode::OK,
        Json(SwitchProjectResponse {
            success: true,
            message: format!("Switched to: {}", request.path),
        }),
    )
}

/// Save a project to the recent projects list
fn save_recent_project(path: &str) -> std::io::Result<()> {
    let config_dir = Config::greppy_home()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    let recent_path = config_dir.join("web-recent-projects.json");

    // Load existing recent projects
    let mut recent: Vec<String> = if recent_path.exists() {
        let content = std::fs::read_to_string(&recent_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Add to front, remove duplicates
    recent.retain(|p| p != path);
    recent.insert(0, path.to_string());

    // Keep only last 10
    recent.truncate(10);

    // Save
    let content = serde_json::to_string_pretty(&recent)?;
    std::fs::write(&recent_path, content)?;

    Ok(())
}
