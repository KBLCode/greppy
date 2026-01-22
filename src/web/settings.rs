//! Settings API endpoints
//!
//! Provides endpoints for managing web UI settings including streamer mode.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::core::config::Config;

// =============================================================================
// TYPES
// =============================================================================

/// Web UI settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebSettings {
    /// Streamer mode - hides sensitive files
    #[serde(rename = "streamerMode")]
    pub streamer_mode: bool,

    /// Glob patterns for files to hide in streamer mode
    #[serde(rename = "hiddenPatterns")]
    pub hidden_patterns: Vec<String>,

    /// Show dead code badges in UI
    #[serde(rename = "showDeadBadges")]
    pub show_dead_badges: bool,

    /// Show cycle indicators in UI
    #[serde(rename = "showCycleIndicators")]
    pub show_cycle_indicators: bool,

    /// Maximum nodes to render in graph view
    #[serde(rename = "maxGraphNodes")]
    pub max_graph_nodes: usize,

    /// Maximum items to show in list view
    #[serde(rename = "maxListItems")]
    pub max_list_items: usize,

    /// Compact mode - reduces spacing
    #[serde(rename = "compactMode")]
    pub compact_mode: bool,

    /// Theme (reserved for future use)
    pub theme: String,
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            streamer_mode: false,
            hidden_patterns: vec![
                ".env*".to_string(),
                "*secret*".to_string(),
                "*credential*".to_string(),
                "**/config/production.*".to_string(),
                "**/*.pem".to_string(),
                "**/*.key".to_string(),
                "**/secrets/**".to_string(),
                "**/.aws/**".to_string(),
                "**/.ssh/**".to_string(),
            ],
            show_dead_badges: true,
            show_cycle_indicators: true,
            max_graph_nodes: 100,
            max_list_items: 500,
            compact_mode: false,
            theme: "dark".to_string(),
        }
    }
}

/// Partial settings update (for PATCH-like behavior)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettingsUpdate {
    #[serde(rename = "streamerMode")]
    pub streamer_mode: Option<bool>,

    #[serde(rename = "hiddenPatterns")]
    pub hidden_patterns: Option<Vec<String>>,

    #[serde(rename = "showDeadBadges")]
    pub show_dead_badges: Option<bool>,

    #[serde(rename = "showCycleIndicators")]
    pub show_cycle_indicators: Option<bool>,

    #[serde(rename = "maxGraphNodes")]
    pub max_graph_nodes: Option<usize>,

    #[serde(rename = "maxListItems")]
    pub max_list_items: Option<usize>,

    #[serde(rename = "compactMode")]
    pub compact_mode: Option<bool>,

    pub theme: Option<String>,
}

/// Shared state for settings
#[derive(Clone)]
pub struct SettingsState {
    pub settings: Arc<RwLock<WebSettings>>,
}

impl SettingsState {
    pub fn new() -> Self {
        let settings = load_settings().unwrap_or_default();
        Self {
            settings: Arc::new(RwLock::new(settings)),
        }
    }
}

// =============================================================================
// PERSISTENCE
// =============================================================================

/// Get the settings file path
fn settings_path() -> Option<PathBuf> {
    Config::greppy_home()
        .ok()
        .map(|home| home.join("web-settings.json"))
}

/// Load settings from disk
pub fn load_settings() -> Option<WebSettings> {
    let path = settings_path()?;
    if !path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save settings to disk
pub fn save_settings(settings: &WebSettings) -> std::io::Result<()> {
    let path = settings_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine settings path",
        )
    })?;

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, content)?;

    Ok(())
}

// =============================================================================
// ROUTE HANDLERS
// =============================================================================

/// GET /api/settings - Get current settings
pub async fn api_get_settings(State(state): State<SettingsState>) -> Json<WebSettings> {
    let settings = state.settings.read().unwrap().clone();
    Json(settings)
}

/// PUT /api/settings - Update settings (full replace or partial update)
pub async fn api_put_settings(
    State(state): State<SettingsState>,
    Json(update): Json<SettingsUpdate>,
) -> impl IntoResponse {
    let mut settings = state.settings.write().unwrap();

    // Apply partial updates
    if let Some(v) = update.streamer_mode {
        settings.streamer_mode = v;
    }
    if let Some(v) = update.hidden_patterns {
        settings.hidden_patterns = v;
    }
    if let Some(v) = update.show_dead_badges {
        settings.show_dead_badges = v;
    }
    if let Some(v) = update.show_cycle_indicators {
        settings.show_cycle_indicators = v;
    }
    if let Some(v) = update.max_graph_nodes {
        settings.max_graph_nodes = v.max(10).min(500); // Clamp to reasonable range
    }
    if let Some(v) = update.max_list_items {
        settings.max_list_items = v.max(50).min(2000); // Clamp to reasonable range
    }
    if let Some(v) = update.compact_mode {
        settings.compact_mode = v;
    }
    if let Some(v) = update.theme {
        settings.theme = v;
    }

    // Save to disk
    let settings_clone = settings.clone();
    drop(settings); // Release lock before IO

    match save_settings(&settings_clone) {
        Ok(_) => (StatusCode::OK, Json(settings_clone)),
        Err(e) => {
            eprintln!("Failed to save settings: {}", e);
            // Still return the settings even if save failed
            (StatusCode::OK, Json(settings_clone))
        }
    }
}

// =============================================================================
// STREAMER MODE HELPERS
// =============================================================================

/// Check if a path should be hidden based on streamer mode patterns
pub fn should_hide_path(path: &str, settings: &WebSettings) -> bool {
    if !settings.streamer_mode {
        return false;
    }

    for pattern_str in &settings.hidden_patterns {
        // Try to match as glob pattern
        if let Ok(pattern) = Pattern::new(pattern_str) {
            if pattern.matches(path) {
                return true;
            }
        }

        // Also try simple contains match for patterns like "*secret*"
        let simple_pattern = pattern_str
            .trim_start_matches('*')
            .trim_end_matches('*')
            .to_lowercase();

        if !simple_pattern.is_empty() && path.to_lowercase().contains(&simple_pattern) {
            return true;
        }
    }

    false
}

/// Redact a path for display in streamer mode
pub fn redact_path(path: &str, settings: &WebSettings) -> String {
    if !should_hide_path(path, settings) {
        return path.to_string();
    }

    // Extract file name and extension
    let path_buf = PathBuf::from(path);
    let extension = path_buf
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    // Get parent path
    let parent = path_buf
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // Return redacted version
    if parent.is_empty() {
        format!("[HIDDEN]{}", extension)
    } else {
        format!("{}/[HIDDEN]{}", parent, extension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_hide_path() {
        let mut settings = WebSettings::default();
        settings.streamer_mode = true;

        assert!(should_hide_path(".env", &settings));
        assert!(should_hide_path(".env.local", &settings));
        assert!(should_hide_path("config/secrets.json", &settings));
        assert!(should_hide_path("credentials.txt", &settings));
        assert!(!should_hide_path("src/main.rs", &settings));
        assert!(!should_hide_path("README.md", &settings));
    }

    #[test]
    fn test_redact_path() {
        let mut settings = WebSettings::default();
        settings.streamer_mode = true;

        assert_eq!(redact_path(".env", &settings), "[HIDDEN]");
        assert_eq!(
            redact_path("config/secrets.json", &settings),
            "config/[HIDDEN].json"
        );
        assert_eq!(redact_path("src/main.rs", &settings), "src/main.rs");
    }

    #[test]
    fn test_streamer_mode_off() {
        let settings = WebSettings::default(); // streamer_mode = false

        assert!(!should_hide_path(".env", &settings));
        assert_eq!(redact_path(".env", &settings), ".env");
    }
}
