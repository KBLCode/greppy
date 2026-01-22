//! Axum web server for greppy web UI

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::core::error::Result;
use crate::core::project::Project;
use crate::trace::{
    compare_snapshots, create_snapshot, find_dead_symbols, list_snapshots, load_index,
    load_snapshot, trace_index_exists, trace_index_path, SemanticIndex, SymbolKind,
};
use crate::web::events::{api_events, start_daemon_event_forwarder, EventsState};
use crate::web::projects::{api_projects, api_switch_project, ProjectsState};
use crate::web::settings::{
    api_get_settings, api_put_settings, redact_path, SettingsState, WebSettings,
};

// =============================================================================
// STATIC FILES (EMBEDDED)
// =============================================================================

const INDEX_HTML: &str = include_str!("static/index.html");
const STYLE_CSS: &str = include_str!("static/style.css");
const APP_JS: &str = include_str!("static/app.js");

// Module files
const API_JS: &str = include_str!("static/api.js");
const UTILS_JS: &str = include_str!("static/utils.js");

// Views
const VIEWS_LIST_JS: &str = include_str!("static/views/list.js");
const VIEWS_STATS_JS: &str = include_str!("static/views/stats.js");
const VIEWS_GRAPH_JS: &str = include_str!("static/views/graph.js");
const VIEWS_TREE_JS: &str = include_str!("static/views/tree.js");
const VIEWS_TABLES_JS: &str = include_str!("static/views/tables.js");
const VIEWS_CYCLES_JS: &str = include_str!("static/views/cycles.js");
const VIEWS_TIMELINE_JS: &str = include_str!("static/views/timeline.js");

// Components
const COMPONENTS_DETAIL_JS: &str = include_str!("static/components/detail.js");
const COMPONENTS_DROPDOWN_JS: &str = include_str!("static/components/dropdown.js");
const COMPONENTS_SSE_JS: &str = include_str!("static/components/sse.js");
const COMPONENTS_CYCLES_JS: &str = include_str!("static/components/cycles.js");
const COMPONENTS_EXPORT_JS: &str = include_str!("static/components/export.js");
const COMPONENTS_SEARCH_JS: &str = include_str!("static/components/search.js");
const COMPONENTS_SETTINGS_JS: &str = include_str!("static/components/settings.js");
const COMPONENTS_SKELETON_JS: &str = include_str!("static/components/skeleton.js");
const COMPONENTS_EMPTY_JS: &str = include_str!("static/components/empty.js");
const COMPONENTS_ERROR_JS: &str = include_str!("static/components/error.js");

// Lib
const LIB_PERSISTENCE_JS: &str = include_str!("static/lib/persistence.js");

// =============================================================================
// STATE
// =============================================================================

#[derive(Clone)]
pub struct AppState {
    pub project_name: String,
    pub project_path: PathBuf,
    pub index: Arc<SemanticIndex>,
    pub dead_symbols: Arc<HashSet<u32>>,
    pub settings: Arc<RwLock<WebSettings>>,
}

impl AppState {
    /// Redact a path if streamer mode is enabled
    fn redact(&self, path: &str) -> String {
        let settings = self.settings.read().unwrap();
        if settings.streamer_mode {
            redact_path(path, &settings)
        } else {
            path.to_string()
        }
    }
}

// =============================================================================
// API TYPES
// =============================================================================

#[derive(Serialize)]
pub struct StatsResponse {
    pub project: String,
    pub files: usize,
    pub symbols: usize,
    pub dead: usize,
    pub cycles: usize,
    pub last_indexed: String,
    pub breakdown: SymbolBreakdown,
}

#[derive(Serialize)]
pub struct SymbolBreakdown {
    pub functions: usize,
    pub classes: usize,
    pub types: usize,
    pub variables: usize,
    pub interfaces: usize,
    pub methods: usize,
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(rename = "type")]
    pub symbol_type: Option<String>,
    pub state: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct ListResponse {
    pub items: Vec<ListItem>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct ListItem {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub symbol_type: String,
    pub path: String,
    pub line: u32,
    pub refs: usize,
    pub callers: usize,
    pub callees: usize,
    pub state: String,
}

#[derive(Deserialize)]
pub struct GraphQuery {
    #[serde(rename = "type")]
    pub symbol_type: Option<String>,
    pub state: Option<String>,
    /// If true, return hierarchical data for treemap visualization
    pub hierarchical: Option<bool>,
    /// Path to zoom into (for drill-down navigation)
    pub path: Option<String>,
}

#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub symbols: usize,
    pub dead: usize,
    pub imports: usize,
    pub exports: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cycle: bool,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub weight: usize,
}

// =============================================================================
// HIERARCHICAL GRAPH TYPES (FOR TREEMAP)
// =============================================================================

#[derive(Serialize)]
pub struct HierarchicalGraphResponse {
    /// Root of the hierarchy
    pub root: HierarchyNode,
    /// Current path for breadcrumb navigation
    pub current_path: String,
    /// Total counts at this level
    pub totals: HierarchyTotals,
}

#[derive(Serialize, Clone)]
pub struct HierarchyNode {
    /// Display name (folder or file name)
    pub name: String,
    /// Full path from root
    pub path: String,
    /// "dir" or "file"
    #[serde(rename = "type")]
    pub node_type: String,
    /// Symbol count (used for treemap sizing)
    pub value: usize,
    /// Number of dead symbols
    pub dead: usize,
    /// Health percentage (0-100)
    pub health: u8,
    /// Is this node or any child involved in a cycle?
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cycle: bool,
    /// Children (folders or files)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<HierarchyNode>>,
    /// Number of files (for directories)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
}

#[derive(Serialize)]
pub struct HierarchyTotals {
    pub files: usize,
    pub symbols: usize,
    pub dead: usize,
    pub cycles: usize,
    pub health: u8,
}

// =============================================================================
// TREE API TYPES
// =============================================================================

#[derive(Serialize)]
pub struct TreeResponse {
    pub root: TreeNode,
}

#[derive(Serialize, Clone)]
pub struct TreeNode {
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreeNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead: Option<usize>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cycle: bool,
}

#[derive(Serialize)]
pub struct FileResponse {
    pub path: String,
    pub symbols: Vec<FileSymbol>,
}

#[derive(Serialize)]
pub struct FileSymbol {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub symbol_type: String,
    pub line: u32,
    pub end_line: u32,
    pub refs: usize,
    pub dead: bool,
}

// =============================================================================
// SYMBOL DETAIL API TYPES
// =============================================================================

/// Full details for a single symbol
#[derive(Serialize)]
pub struct SymbolDetailResponse {
    pub id: u32,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    pub end_line: u32,
    pub refs: usize,
    pub callers_count: usize,
    pub callees_count: usize,
    pub is_dead: bool,
    pub in_cycle: bool,
    pub is_entry_point: bool,
}

/// Caller information with depth
#[derive(Serialize)]
pub struct CallerInfo {
    pub id: u32,
    pub name: String,
    pub file: String,
    pub line: u32,
    pub depth: usize,
}

/// Response for symbol callers endpoint
#[derive(Serialize)]
pub struct CallersResponse {
    pub symbol_id: u32,
    pub callers: Vec<CallerInfo>,
    pub total: usize,
}

/// Callee information
#[derive(Serialize)]
pub struct CalleeInfo {
    pub id: u32,
    pub name: String,
    pub file: String,
    pub line: u32,
}

/// Response for symbol callees endpoint
#[derive(Serialize)]
pub struct CalleesResponse {
    pub symbol_id: u32,
    pub callees: Vec<CalleeInfo>,
    pub total: usize,
}

/// Reference information with context
#[derive(Serialize)]
pub struct RefInfo {
    pub file: String,
    pub line: u32,
    pub kind: String,
    pub context: String,
}

/// Response for symbol refs endpoint
#[derive(Serialize)]
pub struct RefsResponse {
    pub symbol_id: u32,
    pub refs: Vec<RefInfo>,
    pub total: usize,
}

/// Blast radius information
#[derive(Serialize)]
pub struct BlastRadius {
    pub direct_callers: usize,
    pub transitive_callers: usize,
    pub files_affected: usize,
    pub entry_points_affected: usize,
}

/// Response for symbol impact endpoint
#[derive(Serialize)]
pub struct ImpactResponse {
    pub symbol_id: u32,
    pub risk_level: String,
    pub blast_radius: BlastRadius,
    pub paths_to_entry: Vec<Vec<String>>,
}

/// Cycle symbol information
#[derive(Serialize)]
pub struct CycleSymbol {
    pub id: u32,
    pub name: String,
    pub file: String,
}

/// Individual cycle information
#[derive(Serialize)]
pub struct CycleInfo {
    pub id: usize,
    pub size: usize,
    pub severity: String,
    pub symbols: Vec<CycleSymbol>,
    pub path: Vec<String>,
}

/// Response for cycles endpoint
#[derive(Serialize)]
pub struct CyclesResponse {
    pub total_cycles: usize,
    pub total_symbols_in_cycles: usize,
    pub cycles: Vec<CycleInfo>,
}

// =============================================================================
// SNAPSHOT API TYPES
// =============================================================================

/// Response for snapshot list endpoint
#[derive(Serialize)]
pub struct SnapshotsListResponse {
    pub snapshots: Vec<SnapshotSummaryResponse>,
    pub total: usize,
}

/// Summary of a snapshot
#[derive(Serialize)]
pub struct SnapshotSummaryResponse {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
    pub files: u32,
    pub symbols: u32,
    pub dead: u32,
    pub cycles: u32,
}

/// Request to create a snapshot
#[derive(Deserialize)]
pub struct CreateSnapshotRequest {
    pub name: Option<String>,
}

/// Response for snapshot comparison
#[derive(Serialize)]
pub struct SnapshotCompareResponse {
    pub a: SnapshotSummaryResponse,
    pub b: SnapshotSummaryResponse,
    pub diff: SnapshotDiffResponse,
}

/// Diff between two snapshots
#[derive(Serialize)]
pub struct SnapshotDiffResponse {
    pub files: i32,
    pub symbols: i32,
    pub dead: i32,
    pub cycles: i32,
}

// =============================================================================
// HELPERS
// =============================================================================

fn symbol_kind_str(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Interface => "interface",
        SymbolKind::TypeAlias => "type",
        SymbolKind::Constant => "constant",
        SymbolKind::Variable => "variable",
        SymbolKind::Module => "module",
        SymbolKind::Unknown => "unknown",
    }
}

/// Count cycles using DFS (simplified version)
fn count_cycles(index: &SemanticIndex) -> usize {
    let mut graph: HashMap<u16, HashSet<u16>> = HashMap::new();

    for edge in &index.edges {
        if let (Some(from_sym), Some(to_sym)) =
            (index.symbol(edge.from_symbol), index.symbol(edge.to_symbol))
        {
            if from_sym.file_id != to_sym.file_id {
                graph
                    .entry(from_sym.file_id)
                    .or_default()
                    .insert(to_sym.file_id);
            }
        }
    }

    let mut cycles = 0;
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for &node in graph.keys() {
        if !visited.contains(&node) {
            cycles += count_cycles_dfs(node, &graph, &mut visited, &mut rec_stack);
        }
    }

    cycles
}

fn count_cycles_dfs(
    node: u16,
    graph: &HashMap<u16, HashSet<u16>>,
    visited: &mut HashSet<u16>,
    rec_stack: &mut HashSet<u16>,
) -> usize {
    visited.insert(node);
    rec_stack.insert(node);

    let mut cycles = 0;

    if let Some(neighbors) = graph.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                cycles += count_cycles_dfs(neighbor, graph, visited, rec_stack);
            } else if rec_stack.contains(&neighbor) {
                cycles += 1;
            }
        }
    }

    rec_stack.remove(&node);
    cycles
}

/// Find files involved in cycles
fn find_cycle_files(index: &SemanticIndex) -> HashSet<u16> {
    let mut graph: HashMap<u16, HashSet<u16>> = HashMap::new();

    for edge in &index.edges {
        if let (Some(from_sym), Some(to_sym)) =
            (index.symbol(edge.from_symbol), index.symbol(edge.to_symbol))
        {
            if from_sym.file_id != to_sym.file_id {
                graph
                    .entry(from_sym.file_id)
                    .or_default()
                    .insert(to_sym.file_id);
            }
        }
    }

    let mut cycle_files = HashSet::new();
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();

    for &node in graph.keys() {
        if !visited.contains(&node) {
            find_cycle_files_dfs(
                node,
                &graph,
                &mut visited,
                &mut rec_stack,
                &mut path,
                &mut cycle_files,
            );
        }
    }

    cycle_files
}

fn find_cycle_files_dfs(
    node: u16,
    graph: &HashMap<u16, HashSet<u16>>,
    visited: &mut HashSet<u16>,
    rec_stack: &mut HashSet<u16>,
    path: &mut Vec<u16>,
    cycle_files: &mut HashSet<u16>,
) {
    visited.insert(node);
    rec_stack.insert(node);
    path.push(node);

    if let Some(neighbors) = graph.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                find_cycle_files_dfs(neighbor, graph, visited, rec_stack, path, cycle_files);
            } else if rec_stack.contains(&neighbor) {
                // Found cycle - mark all files in the cycle
                if let Some(start) = path.iter().position(|&n| n == neighbor) {
                    for &f in &path[start..] {
                        cycle_files.insert(f);
                    }
                }
            }
        }
    }

    path.pop();
    rec_stack.remove(&node);
}

/// Build a hierarchical tree from flat file paths
fn build_file_tree(
    index: &SemanticIndex,
    dead_symbols: &HashSet<u32>,
    cycle_files: &HashSet<u16>,
) -> TreeNode {
    // Collect file info: (path, file_id, symbol_count, dead_count, is_cycle)
    let mut file_info: Vec<(String, u16, usize, usize, bool)> = Vec::new();

    for (file_id, path) in index.files.iter().enumerate() {
        let file_id = file_id as u16;
        let path_str = path.to_string_lossy().to_string();

        // Count symbols and dead symbols in this file
        let mut symbol_count = 0usize;
        let mut dead_count = 0usize;

        for symbol in index.symbols_in_file(file_id) {
            symbol_count += 1;
            if dead_symbols.contains(&symbol.id) {
                dead_count += 1;
            }
        }

        let is_cycle = cycle_files.contains(&file_id);
        file_info.push((path_str, file_id, symbol_count, dead_count, is_cycle));
    }

    // Build tree structure
    let mut root = TreeNode {
        name: "root".to_string(),
        node_type: "dir".to_string(),
        path: None,
        children: Some(Vec::new()),
        symbols: None,
        dead: None,
        cycle: false,
    };

    for (path, _file_id, symbol_count, dead_count, is_cycle) in file_info {
        insert_path_into_tree(&mut root, &path, symbol_count, dead_count, is_cycle);
    }

    // Sort children alphabetically, directories first
    sort_tree_children(&mut root);

    // Propagate cycle status up the tree
    propagate_cycle_status(&mut root);

    root
}

fn insert_path_into_tree(
    root: &mut TreeNode,
    path: &str,
    symbol_count: usize,
    dead_count: usize,
    is_cycle: bool,
) {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    insert_path_recursive(root, &parts, 0, path, symbol_count, dead_count, is_cycle);
}

fn insert_path_recursive(
    current: &mut TreeNode,
    parts: &[&str],
    index: usize,
    full_path: &str,
    symbol_count: usize,
    dead_count: usize,
    is_cycle: bool,
) {
    if index >= parts.len() {
        return;
    }

    let part = parts[index];
    let is_last = index == parts.len() - 1;

    // Ensure children vec exists
    if current.children.is_none() {
        current.children = Some(Vec::new());
    }

    let children = current.children.as_mut().unwrap();

    // Find existing child or create new one
    let child_idx = children.iter().position(|c| c.name == part);

    if let Some(idx) = child_idx {
        if is_last {
            // Update existing file node
            let child = &mut children[idx];
            child.symbols = Some(symbol_count);
            child.dead = if dead_count > 0 {
                Some(dead_count)
            } else {
                None
            };
            child.cycle = is_cycle;
            child.path = Some(full_path.to_string());
        } else {
            // Recurse into existing directory
            insert_path_recursive(
                &mut children[idx],
                parts,
                index + 1,
                full_path,
                symbol_count,
                dead_count,
                is_cycle,
            );
        }
    } else {
        // Create new node
        let new_node = if is_last {
            TreeNode {
                name: part.to_string(),
                node_type: "file".to_string(),
                path: Some(full_path.to_string()),
                children: None,
                symbols: Some(symbol_count),
                dead: if dead_count > 0 {
                    Some(dead_count)
                } else {
                    None
                },
                cycle: is_cycle,
            }
        } else {
            TreeNode {
                name: part.to_string(),
                node_type: "dir".to_string(),
                path: None,
                children: Some(Vec::new()),
                symbols: None,
                dead: None,
                cycle: false,
            }
        };

        children.push(new_node);

        if !is_last {
            // Recurse into newly created directory
            let len = children.len();
            insert_path_recursive(
                &mut children[len - 1],
                parts,
                index + 1,
                full_path,
                symbol_count,
                dead_count,
                is_cycle,
            );
        }
    }
}

fn sort_tree_children(node: &mut TreeNode) {
    if let Some(ref mut children) = node.children {
        // Sort: directories first, then alphabetically
        children.sort_by(|a, b| {
            let a_is_dir = a.node_type == "dir";
            let b_is_dir = b.node_type == "dir";

            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        // Recursively sort children
        for child in children.iter_mut() {
            sort_tree_children(child);
        }
    }
}

/// Propagate cycle status up the tree (if any child has cycle, parent has cycle)
fn propagate_cycle_status(node: &mut TreeNode) -> bool {
    if let Some(ref mut children) = node.children {
        let mut has_cycle = node.cycle;
        for child in children.iter_mut() {
            if propagate_cycle_status(child) {
                has_cycle = true;
            }
        }
        node.cycle = has_cycle;
        has_cycle
    } else {
        node.cycle
    }
}

/// Recursively redact all paths in the tree for streamer mode
fn redact_tree_paths(node: &mut TreeNode, state: &AppState) {
    // Redact this node's path if present
    if let Some(ref path) = node.path {
        node.path = Some(state.redact(path));
    }

    // Recursively redact children
    if let Some(ref mut children) = node.children {
        for child in children.iter_mut() {
            redact_tree_paths(child, state);
        }
    }
}

// =============================================================================
// ROUTES
// =============================================================================

async fn index_html() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn style_css() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css")],
        STYLE_CSS,
    )
        .into_response()
}

async fn app_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        APP_JS,
    )
        .into_response()
}

async fn api_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        API_JS,
    )
        .into_response()
}

async fn utils_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        UTILS_JS,
    )
        .into_response()
}

async fn views_list_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        VIEWS_LIST_JS,
    )
        .into_response()
}

async fn views_stats_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        VIEWS_STATS_JS,
    )
        .into_response()
}

async fn views_graph_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        VIEWS_GRAPH_JS,
    )
        .into_response()
}

async fn views_tree_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        VIEWS_TREE_JS,
    )
        .into_response()
}

async fn views_tables_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        VIEWS_TABLES_JS,
    )
        .into_response()
}

async fn views_cycles_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        VIEWS_CYCLES_JS,
    )
        .into_response()
}

async fn views_timeline_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        VIEWS_TIMELINE_JS,
    )
        .into_response()
}

async fn components_detail_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_DETAIL_JS,
    )
        .into_response()
}

async fn components_dropdown_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_DROPDOWN_JS,
    )
        .into_response()
}

async fn components_sse_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_SSE_JS,
    )
        .into_response()
}

async fn components_cycles_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_CYCLES_JS,
    )
        .into_response()
}

async fn components_export_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_EXPORT_JS,
    )
        .into_response()
}

async fn components_search_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_SEARCH_JS,
    )
        .into_response()
}

async fn components_settings_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_SETTINGS_JS,
    )
        .into_response()
}

async fn components_skeleton_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_SKELETON_JS,
    )
        .into_response()
}

async fn components_empty_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_EMPTY_JS,
    )
        .into_response()
}

async fn components_error_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        COMPONENTS_ERROR_JS,
    )
        .into_response()
}

async fn lib_persistence_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        LIB_PERSISTENCE_JS,
    )
        .into_response()
}

async fn api_stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let index = &state.index;
    let stats = index.stats();

    let mut breakdown = HashMap::new();
    for symbol in &index.symbols {
        let kind = symbol_kind_str(symbol.symbol_kind());
        *breakdown.entry(kind).or_insert(0usize) += 1;
    }

    let dead = state.dead_symbols.len();
    let cycles = count_cycles(index);

    Json(StatsResponse {
        project: state.project_name.clone(),
        files: stats.files,
        symbols: stats.symbols,
        dead,
        cycles,
        last_indexed: "just now".to_string(),
        breakdown: SymbolBreakdown {
            functions: *breakdown.get("function").unwrap_or(&0),
            classes: *breakdown.get("class").unwrap_or(&0),
            types: *breakdown.get("type").unwrap_or(&0),
            variables: *breakdown.get("variable").unwrap_or(&0),
            interfaces: *breakdown.get("interface").unwrap_or(&0),
            methods: *breakdown.get("method").unwrap_or(&0),
        },
    })
}

async fn api_list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Json<ListResponse> {
    let index = &state.index;
    let limit = query.limit.unwrap_or(200).min(1000);

    let mut items = Vec::new();

    for symbol in &index.symbols {
        let name = index.symbol_name(symbol).unwrap_or("").to_string();
        let kind = symbol_kind_str(symbol.symbol_kind());
        let path = index
            .file_path(symbol.file_id)
            .map(|p| state.redact(&p.to_string_lossy()))
            .unwrap_or_default();

        // Apply filters
        if let Some(ref type_filter) = query.symbol_type {
            if type_filter != "all" && kind != type_filter {
                continue;
            }
        }

        let is_dead = state.dead_symbols.contains(&symbol.id);

        if let Some(ref state_filter) = query.state {
            match state_filter.as_str() {
                "dead" if !is_dead => continue,
                "used" if is_dead => continue,
                _ => {}
            }
        }

        if let Some(ref search) = query.search {
            if !search.is_empty() && !name.to_lowercase().contains(&search.to_lowercase()) {
                continue;
            }
        }

        let refs = index.references_to(symbol.id).count();
        let callers = index.callers(symbol.id).len();
        let callees = index.callees(symbol.id).len();

        items.push(ListItem {
            id: symbol.id,
            name,
            symbol_type: kind.to_string(),
            path,
            line: symbol.start_line as u32,
            refs,
            callers,
            callees,
            state: if is_dead {
                "dead".to_string()
            } else {
                "used".to_string()
            },
        });

        if items.len() >= limit {
            break;
        }
    }

    let total = items.len();
    Json(ListResponse { items, total })
}

async fn api_graph(State(state): State<AppState>, Query(query): Query<GraphQuery>) -> Response {
    // Check if hierarchical treemap data is requested
    if query.hierarchical.unwrap_or(false) {
        return api_graph_hierarchical(State(state), query)
            .await
            .into_response();
    }

    // Original force-directed graph logic
    let index = &state.index;

    // Build file-level graph
    let mut file_symbols: HashMap<u16, usize> = HashMap::new();
    let mut file_dead: HashMap<u16, usize> = HashMap::new();
    let mut file_edges: HashMap<(u16, u16), usize> = HashMap::new();
    let mut file_imports: HashMap<u16, usize> = HashMap::new();
    let mut file_exports: HashMap<u16, usize> = HashMap::new();

    // Count symbols per file
    for symbol in &index.symbols {
        *file_symbols.entry(symbol.file_id).or_insert(0) += 1;

        if state.dead_symbols.contains(&symbol.id) {
            *file_dead.entry(symbol.file_id).or_insert(0) += 1;
        }

        // Count exports (entry points)
        if symbol.is_entry_point() {
            *file_exports.entry(symbol.file_id).or_insert(0) += 1;
        }
    }

    // Build edges between files
    for edge in &index.edges {
        if let (Some(from_sym), Some(to_sym)) =
            (index.symbol(edge.from_symbol), index.symbol(edge.to_symbol))
        {
            if from_sym.file_id != to_sym.file_id {
                *file_edges
                    .entry((from_sym.file_id, to_sym.file_id))
                    .or_insert(0) += 1;
                *file_imports.entry(from_sym.file_id).or_insert(0) += 1;
            }
        }
    }

    // Find cycle files
    let cycle_files = find_cycle_files(index);

    // Filter files based on query
    let include_file = |file_id: u16| -> bool {
        if let Some(ref state_filter) = query.state {
            match state_filter.as_str() {
                "dead" => return file_dead.get(&file_id).copied().unwrap_or(0) > 0,
                "cycle" => return cycle_files.contains(&file_id),
                _ => {}
            }
        }
        true
    };

    // Build nodes
    let mut nodes = Vec::new();
    for (file_id, &symbol_count) in &file_symbols {
        if !include_file(*file_id) {
            continue;
        }

        let raw_path = index
            .file_path(*file_id)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("file_{}", file_id));

        let path = state.redact(&raw_path);

        let name = std::path::Path::new(&raw_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());

        nodes.push(GraphNode {
            id: path.clone(),
            name,
            symbols: symbol_count,
            dead: file_dead.get(file_id).copied().unwrap_or(0),
            imports: file_imports.get(file_id).copied().unwrap_or(0),
            exports: file_exports.get(file_id).copied().unwrap_or(0),
            cycle: cycle_files.contains(file_id),
        });
    }

    // Build edges (only between included nodes)
    let node_ids: HashSet<_> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut edges = Vec::new();

    for ((from_id, to_id), weight) in &file_edges {
        let from_path = index
            .file_path(*from_id)
            .map(|p| state.redact(&p.to_string_lossy()))
            .unwrap_or_default();
        let to_path = index
            .file_path(*to_id)
            .map(|p| state.redact(&p.to_string_lossy()))
            .unwrap_or_default();

        if node_ids.contains(&from_path) && node_ids.contains(&to_path) {
            edges.push(GraphEdge {
                source: from_path,
                target: to_path,
                weight: *weight,
            });
        }
    }

    // Limit for performance
    if nodes.len() > 100 {
        // Sort by symbol count and take top 100
        nodes.sort_by(|a, b| b.symbols.cmp(&a.symbols));
        nodes.truncate(100);

        // Filter edges to only include remaining nodes
        let remaining_ids: HashSet<_> = nodes.iter().map(|n| n.id.clone()).collect();
        edges.retain(|e| remaining_ids.contains(&e.source) && remaining_ids.contains(&e.target));
    }

    Json(GraphResponse { nodes, edges }).into_response()
}

/// Build hierarchical treemap data for scalable visualization
async fn api_graph_hierarchical(
    State(state): State<AppState>,
    query: GraphQuery,
) -> Json<HierarchicalGraphResponse> {
    let index = &state.index;
    let cycle_files = find_cycle_files(index);
    let base_path = query.path.unwrap_or_default();

    // Build file stats map: file_id -> (symbols, dead, in_cycle)
    let mut file_stats: HashMap<u16, (usize, usize, bool)> = HashMap::new();

    for symbol in &index.symbols {
        let entry = file_stats.entry(symbol.file_id).or_insert((0, 0, false));
        entry.0 += 1; // symbol count
        if state.dead_symbols.contains(&symbol.id) {
            entry.1 += 1; // dead count
        }
    }

    // Mark cycle files
    for file_id in &cycle_files {
        if let Some(entry) = file_stats.get_mut(file_id) {
            entry.2 = true;
        }
    }

    // Build hierarchy from file paths
    let root = build_treemap_hierarchy(index, &file_stats, &base_path, &state);

    // Calculate totals
    let totals = calc_treemap_totals(&root);

    Json(HierarchicalGraphResponse {
        root,
        current_path: state.redact(&base_path),
        totals,
    })
}

/// Build a hierarchical tree structure from file paths
fn build_treemap_hierarchy(
    index: &SemanticIndex,
    file_stats: &HashMap<u16, (usize, usize, bool)>,
    base_path: &str,
    state: &AppState,
) -> HierarchyNode {
    // Group files by their path components
    let mut dir_children: HashMap<String, Vec<(String, u16)>> = HashMap::new();

    for (file_id, _stats) in file_stats {
        if let Some(path) = index.file_path(*file_id) {
            let path_str: String = path.to_string_lossy().to_string();

            // Filter by base_path if specified
            if !base_path.is_empty() && !path_str.starts_with(base_path) {
                continue;
            }

            // Get relative path from base
            let relative = if base_path.is_empty() {
                path_str.clone()
            } else {
                path_str
                    .strip_prefix(base_path)
                    .unwrap_or(&path_str)
                    .trim_start_matches('/')
                    .to_string()
            };

            // Get first component (immediate child)
            let first_component = relative.split('/').next().unwrap_or(&relative);
            let full_child_path = if base_path.is_empty() {
                first_component.to_string()
            } else {
                format!("{}/{}", base_path, first_component)
            };

            dir_children
                .entry(full_child_path)
                .or_default()
                .push((path_str, *file_id));
        }
    }

    // Build children nodes
    let mut children: Vec<HierarchyNode> = Vec::new();

    for (child_path, files) in dir_children {
        let name = std::path::Path::new(&child_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| child_path.clone());

        // Check if this is a single file or a directory
        if files.len() == 1 && files[0].0 == child_path {
            // It's a file
            let (symbols, dead, cycle) = file_stats
                .get(&files[0].1)
                .copied()
                .unwrap_or((1, 0, false));
            let health = if symbols > 0 {
                (((symbols - dead) as f64 / symbols as f64) * 100.0) as u8
            } else {
                100
            };

            children.push(HierarchyNode {
                name,
                path: state.redact(&child_path),
                node_type: "file".to_string(),
                value: symbols.max(1), // Ensure minimum value for treemap
                dead,
                health,
                cycle,
                children: None,
                file_count: None,
            });
        } else {
            // It's a directory - recurse
            let sub_node = build_treemap_hierarchy(index, file_stats, &child_path, state);
            children.push(sub_node);
        }
    }

    // Sort children by value (largest first)
    children.sort_by(|a, b| b.value.cmp(&a.value));

    // Calculate aggregate stats for this directory
    let (total_value, total_dead, any_cycle, file_count) =
        children.iter().fold((0, 0, false, 0), |acc, child| {
            let files = if child.node_type == "file" {
                1
            } else {
                child.file_count.unwrap_or(0)
            };
            (
                acc.0 + child.value,
                acc.1 + child.dead,
                acc.2 || child.cycle,
                acc.3 + files,
            )
        });

    let health = if total_value > 0 {
        (((total_value - total_dead) as f64 / total_value as f64) * 100.0) as u8
    } else {
        100
    };

    let name = if base_path.is_empty() {
        ".".to_string()
    } else {
        std::path::Path::new(base_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| base_path.to_string())
    };

    HierarchyNode {
        name,
        path: state.redact(base_path),
        node_type: "dir".to_string(),
        value: total_value.max(1),
        dead: total_dead,
        health,
        cycle: any_cycle,
        children: Some(children),
        file_count: Some(file_count),
    }
}

/// Calculate totals for the hierarchy response
fn calc_treemap_totals(root: &HierarchyNode) -> HierarchyTotals {
    fn count_recursive(node: &HierarchyNode) -> (usize, usize, usize, usize) {
        if node.node_type == "file" {
            let cycle_count = if node.cycle { 1 } else { 0 };
            (1, node.value, node.dead, cycle_count)
        } else if let Some(children) = &node.children {
            children.iter().fold((0, 0, 0, 0), |acc, child| {
                let (f, s, d, c) = count_recursive(child);
                (acc.0 + f, acc.1 + s, acc.2 + d, acc.3 + c)
            })
        } else {
            (0, 0, 0, 0)
        }
    }

    let (files, symbols, dead, cycles) = count_recursive(root);
    let health = if symbols > 0 {
        (((symbols - dead) as f64 / symbols as f64) * 100.0) as u8
    } else {
        100
    };

    HierarchyTotals {
        files,
        symbols,
        dead,
        cycles,
        health,
    }
}

async fn api_tree(State(state): State<AppState>) -> Json<TreeResponse> {
    let index = &state.index;
    let cycle_files = find_cycle_files(index);
    let mut tree = build_file_tree(index, &state.dead_symbols, &cycle_files);

    // Apply path redaction for streamer mode
    redact_tree_paths(&mut tree, &state);

    Json(TreeResponse { root: tree })
}

async fn api_file(
    State(state): State<AppState>,
    Path(file_path): Path<String>,
) -> std::result::Result<Json<FileResponse>, StatusCode> {
    let index = &state.index;

    // URL decode the path
    let decoded_path = urlencoding::decode(&file_path)
        .map(|s| s.into_owned())
        .unwrap_or(file_path);

    // Find the file_id for this path
    let file_id = index
        .files
        .iter()
        .enumerate()
        .find(|(_, p)| p.to_string_lossy() == decoded_path)
        .map(|(id, _)| id as u16);

    let file_id = match file_id {
        Some(id) => id,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Get symbols for this file
    let mut symbols: Vec<FileSymbol> = Vec::new();

    for symbol in index.symbols_in_file(file_id) {
        let name = index.symbol_name(symbol).unwrap_or("").to_string();
        let kind = symbol_kind_str(symbol.symbol_kind());
        let refs = index.references_to(symbol.id).count();
        let is_dead = state.dead_symbols.contains(&symbol.id);

        symbols.push(FileSymbol {
            id: symbol.id,
            name,
            symbol_type: kind.to_string(),
            line: symbol.start_line,
            end_line: symbol.end_line,
            refs,
            dead: is_dead,
        });
    }

    // Sort by line number
    symbols.sort_by_key(|s| s.line);

    Ok(Json(FileResponse {
        path: state.redact(&decoded_path),
        symbols,
    }))
}

// =============================================================================
// SYMBOL DETAIL API HANDLERS
// =============================================================================

/// GET /api/symbol/:id - Full details for a single symbol
async fn api_symbol_detail(
    State(state): State<AppState>,
    Path(symbol_id): Path<u32>,
) -> std::result::Result<Json<SymbolDetailResponse>, StatusCode> {
    let index = &state.index;

    let symbol = match index.symbol(symbol_id) {
        Some(s) => s,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let name = index.symbol_name(symbol).unwrap_or("").to_string();
    let kind = symbol_kind_str(symbol.symbol_kind()).to_string();
    let file = index
        .file_path(symbol.file_id)
        .map(|p| state.redact(&p.to_string_lossy()))
        .unwrap_or_default();

    let refs = index.references_to(symbol.id).count();
    let callers_count = index.callers(symbol.id).len();
    let callees_count = index.callees(symbol.id).len();
    let is_dead = state.dead_symbols.contains(&symbol.id);

    // Check if symbol is in a cycle
    let cycle_files = find_cycle_files(index);
    let in_cycle = cycle_files.contains(&symbol.file_id);

    Ok(Json(SymbolDetailResponse {
        id: symbol.id,
        name,
        kind,
        file,
        line: symbol.start_line,
        end_line: symbol.end_line,
        refs,
        callers_count,
        callees_count,
        is_dead,
        in_cycle,
        is_entry_point: symbol.is_entry_point(),
    }))
}

/// GET /api/symbol/:id/callers - All callers of a symbol with depth
async fn api_symbol_callers(
    State(state): State<AppState>,
    Path(symbol_id): Path<u32>,
) -> std::result::Result<Json<CallersResponse>, StatusCode> {
    let index = &state.index;

    // Verify symbol exists
    if index.symbol(symbol_id).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // BFS to find all callers with depth
    let mut callers: Vec<CallerInfo> = Vec::new();
    let mut visited: HashSet<u32> = HashSet::new();
    let mut queue: VecDeque<(u32, usize)> = VecDeque::new();

    // Start with direct callers (depth 1)
    for &caller_id in index.callers(symbol_id) {
        if visited.insert(caller_id) {
            queue.push_back((caller_id, 1));
        }
    }

    // BFS traversal
    while let Some((current_id, depth)) = queue.pop_front() {
        if let Some(symbol) = index.symbol(current_id) {
            let name = index.symbol_name(symbol).unwrap_or("").to_string();
            let file = index
                .file_path(symbol.file_id)
                .map(|p| state.redact(&p.to_string_lossy()))
                .unwrap_or_default();

            callers.push(CallerInfo {
                id: current_id,
                name,
                file,
                line: symbol.start_line,
                depth,
            });

            // Continue BFS up to depth 10
            if depth < 10 {
                for &parent_id in index.callers(current_id) {
                    if visited.insert(parent_id) {
                        queue.push_back((parent_id, depth + 1));
                    }
                }
            }
        }
    }

    // Sort by depth, then by name
    callers.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.name.cmp(&b.name)));

    let total = callers.len();
    Ok(Json(CallersResponse {
        symbol_id,
        callers,
        total,
    }))
}

/// GET /api/symbol/:id/callees - All symbols this symbol calls
async fn api_symbol_callees(
    State(state): State<AppState>,
    Path(symbol_id): Path<u32>,
) -> std::result::Result<Json<CalleesResponse>, StatusCode> {
    let index = &state.index;

    // Verify symbol exists
    if index.symbol(symbol_id).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut callees: Vec<CalleeInfo> = Vec::new();

    for &callee_id in index.callees(symbol_id) {
        if let Some(symbol) = index.symbol(callee_id) {
            let name = index.symbol_name(symbol).unwrap_or("").to_string();
            let file = index
                .file_path(symbol.file_id)
                .map(|p| state.redact(&p.to_string_lossy()))
                .unwrap_or_default();

            callees.push(CalleeInfo {
                id: callee_id,
                name,
                file,
                line: symbol.start_line,
            });
        }
    }

    // Sort by name
    callees.sort_by(|a, b| a.name.cmp(&b.name));

    let total = callees.len();
    Ok(Json(CalleesResponse {
        symbol_id,
        callees,
        total,
    }))
}

/// GET /api/symbol/:id/refs - All references to a symbol with context
async fn api_symbol_refs(
    State(state): State<AppState>,
    Path(symbol_id): Path<u32>,
) -> std::result::Result<Json<RefsResponse>, StatusCode> {
    let index = &state.index;

    // Verify symbol exists
    if index.symbol(symbol_id).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut refs: Vec<RefInfo> = Vec::new();

    for reference in index.references_to(symbol_id) {
        if let Some(token) = index.token(reference.token_id) {
            let file = index
                .file_path(token.file_id)
                .map(|p| state.redact(&p.to_string_lossy()))
                .unwrap_or_default();

            let kind = match reference.ref_kind() {
                crate::trace::RefKind::Read => "Read",
                crate::trace::RefKind::Write => "Write",
                crate::trace::RefKind::Call => "Call",
                crate::trace::RefKind::TypeAnnotation => "Type",
                crate::trace::RefKind::Import => "Import",
                crate::trace::RefKind::Export => "Export",
                crate::trace::RefKind::Inheritance => "Inheritance",
                crate::trace::RefKind::Decorator => "Decorator",
                crate::trace::RefKind::Construction => "Construction",
                crate::trace::RefKind::Unknown => "Unknown",
            }
            .to_string();

            // Build context string (we don't have source content, so use token name)
            let token_name = index.token_name(token).unwrap_or("");
            let context = format!(
                "{}:{} - {}",
                file.split('/').last().unwrap_or(&file),
                token.line,
                token_name
            );

            refs.push(RefInfo {
                file,
                line: token.line,
                kind,
                context,
            });
        }
    }

    // Sort by file, then by line
    refs.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));

    let total = refs.len();
    Ok(Json(RefsResponse {
        symbol_id,
        refs,
        total,
    }))
}

/// GET /api/symbol/:id/impact - Impact analysis for a symbol
async fn api_symbol_impact(
    State(state): State<AppState>,
    Path(symbol_id): Path<u32>,
) -> std::result::Result<Json<ImpactResponse>, StatusCode> {
    let index = &state.index;

    // Verify symbol exists
    if index.symbol(symbol_id).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Calculate transitive callers using BFS
    let mut all_callers: HashSet<u32> = HashSet::new();
    let mut affected_files: HashSet<u16> = HashSet::new();
    let mut affected_entry_points: HashSet<u32> = HashSet::new();
    let mut paths_to_entry: Vec<Vec<String>> = Vec::new();
    let mut queue: VecDeque<(u32, Vec<u32>)> = VecDeque::new();

    // Start with direct callers
    let direct_callers = index.callers(symbol_id).len();
    for &caller_id in index.callers(symbol_id) {
        queue.push_back((caller_id, vec![caller_id]));
        all_callers.insert(caller_id);
    }

    // BFS to find all transitive callers and paths to entry points
    while let Some((current_id, path)) = queue.pop_front() {
        if let Some(symbol) = index.symbol(current_id) {
            affected_files.insert(symbol.file_id);

            // Check if this is an entry point
            if symbol.is_entry_point() {
                affected_entry_points.insert(current_id);

                // Build path names (limit to 5 paths)
                if paths_to_entry.len() < 5 {
                    let path_names: Vec<String> = path
                        .iter()
                        .filter_map(|&id| {
                            index
                                .symbol(id)
                                .and_then(|s| index.symbol_name(s).map(|n| n.to_string()))
                        })
                        .collect();
                    if !path_names.is_empty() {
                        paths_to_entry.push(path_names);
                    }
                }
            }

            // Continue BFS (limit depth to 50)
            if path.len() < 50 {
                for &parent_id in index.callers(current_id) {
                    if all_callers.insert(parent_id) {
                        let mut new_path = path.clone();
                        new_path.push(parent_id);
                        queue.push_back((parent_id, new_path));
                    }
                }
            }
        }
    }

    // Calculate risk level
    let risk_level = if affected_entry_points.len() > 10 || affected_files.len() > 50 {
        "critical"
    } else if affected_entry_points.len() > 5 || affected_files.len() > 20 {
        "high"
    } else if direct_callers > 5 || affected_files.len() > 5 {
        "medium"
    } else {
        "low"
    }
    .to_string();

    Ok(Json(ImpactResponse {
        symbol_id,
        risk_level,
        blast_radius: BlastRadius {
            direct_callers,
            transitive_callers: all_callers.len(),
            files_affected: affected_files.len(),
            entry_points_affected: affected_entry_points.len(),
        },
        paths_to_entry,
    }))
}

/// GET /api/cycles - All circular dependencies
async fn api_cycles(State(state): State<AppState>) -> Json<CyclesResponse> {
    let index = &state.index;

    // Build file-level graph and find cycles
    let mut graph: HashMap<u16, HashSet<u16>> = HashMap::new();

    for edge in &index.edges {
        if let (Some(from_sym), Some(to_sym)) =
            (index.symbol(edge.from_symbol), index.symbol(edge.to_symbol))
        {
            if from_sym.file_id != to_sym.file_id {
                graph
                    .entry(from_sym.file_id)
                    .or_default()
                    .insert(to_sym.file_id);
            }
        }
    }

    // Find all cycles using DFS with path tracking
    let mut all_cycles: Vec<Vec<u16>> = Vec::new();
    let mut visited: HashSet<u16> = HashSet::new();
    let mut rec_stack: HashSet<u16> = HashSet::new();
    let mut path: Vec<u16> = Vec::new();

    for &node in graph.keys() {
        if !visited.contains(&node) {
            find_all_cycles(
                node,
                &graph,
                &mut visited,
                &mut rec_stack,
                &mut path,
                &mut all_cycles,
            );
        }
    }

    // Convert cycles to response format
    let mut cycles: Vec<CycleInfo> = Vec::new();
    let mut symbols_in_cycles: HashSet<u32> = HashSet::new();

    for (i, cycle) in all_cycles.iter().enumerate() {
        let mut cycle_symbols: Vec<CycleSymbol> = Vec::new();
        let mut path_names: Vec<String> = Vec::new();

        for &file_id in cycle {
            // Get first symbol from this file for display
            if let Some(symbol) = index.symbols.iter().find(|s| s.file_id == file_id) {
                let name = index.symbol_name(symbol).unwrap_or("").to_string();
                let file = index
                    .file_path(file_id)
                    .map(|p| state.redact(&p.to_string_lossy()))
                    .unwrap_or_default();

                cycle_symbols.push(CycleSymbol {
                    id: symbol.id,
                    name: name.clone(),
                    file,
                });
                path_names.push(name);
                symbols_in_cycles.insert(symbol.id);
            }
        }

        // Close the cycle path
        if let Some(first) = path_names.first() {
            path_names.push(first.clone());
        }

        let severity = if cycle.len() > 5 {
            "critical"
        } else if cycle.len() > 3 {
            "high"
        } else {
            "medium"
        }
        .to_string();

        cycles.push(CycleInfo {
            id: i + 1,
            size: cycle.len(),
            severity,
            symbols: cycle_symbols,
            path: path_names,
        });
    }

    // Sort by size (largest first)
    cycles.sort_by(|a, b| b.size.cmp(&a.size));

    Json(CyclesResponse {
        total_cycles: cycles.len(),
        total_symbols_in_cycles: symbols_in_cycles.len(),
        cycles,
    })
}

/// Helper function to find all cycles in the graph
fn find_all_cycles(
    node: u16,
    graph: &HashMap<u16, HashSet<u16>>,
    visited: &mut HashSet<u16>,
    rec_stack: &mut HashSet<u16>,
    path: &mut Vec<u16>,
    cycles: &mut Vec<Vec<u16>>,
) {
    visited.insert(node);
    rec_stack.insert(node);
    path.push(node);

    if let Some(neighbors) = graph.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                find_all_cycles(neighbor, graph, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(&neighbor) {
                // Found a cycle - extract it
                if let Some(start_idx) = path.iter().position(|&n| n == neighbor) {
                    let cycle: Vec<u16> = path[start_idx..].to_vec();
                    // Only add unique cycles (avoid duplicates from different starting points)
                    if !cycles
                        .iter()
                        .any(|c| c.len() == cycle.len() && cycle.iter().all(|n| c.contains(n)))
                    {
                        cycles.push(cycle);
                    }
                }
            }
        }
    }

    path.pop();
    rec_stack.remove(&node);
}

// =============================================================================
// SNAPSHOT HANDLERS
// =============================================================================

/// GET /api/snapshots - List all snapshots
async fn api_list_snapshots(State(state): State<AppState>) -> impl IntoResponse {
    match list_snapshots(&state.project_path) {
        Ok(list) => {
            let snapshots: Vec<SnapshotSummaryResponse> = list
                .snapshots
                .into_iter()
                .map(|s| SnapshotSummaryResponse {
                    id: s.id,
                    name: s.name,
                    created_at: s.created_at.to_rfc3339(),
                    files: s.files,
                    symbols: s.symbols,
                    dead: s.dead,
                    cycles: s.cycles,
                })
                .collect();

            let total = snapshots.len();
            (
                StatusCode::OK,
                Json(SnapshotsListResponse { snapshots, total }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /api/snapshots - Create a new snapshot
async fn api_create_snapshot(
    State(state): State<AppState>,
    Json(req): Json<CreateSnapshotRequest>,
) -> impl IntoResponse {
    // Count cycles for the snapshot
    let cycles_count = count_cycles(&state.index) as u32;

    match create_snapshot(
        &state.index,
        &state.project_path,
        &state.project_name,
        &state.dead_symbols,
        cycles_count,
        req.name,
    ) {
        Ok(snapshot) => {
            let response = SnapshotSummaryResponse {
                id: snapshot.id,
                name: snapshot.name,
                created_at: snapshot.created_at.to_rfc3339(),
                files: snapshot.metrics.files,
                symbols: snapshot.metrics.symbols,
                dead: snapshot.metrics.dead,
                cycles: snapshot.metrics.cycles,
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /api/snapshots/:id - Get a specific snapshot
async fn api_get_snapshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match load_snapshot(&state.project_path, &id) {
        Ok(snapshot) => (StatusCode::OK, Json(snapshot)).into_response(),
        Err(e) => {
            if e.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": e })),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e })),
                )
                    .into_response()
            }
        }
    }
}

/// Query parameters for snapshot comparison
#[derive(Deserialize)]
pub struct CompareQuery {
    pub a: String,
    pub b: String,
}

/// GET /api/snapshots/compare?a=id1&b=id2 - Compare two snapshots
async fn api_compare_snapshots(
    State(state): State<AppState>,
    Query(query): Query<CompareQuery>,
) -> impl IntoResponse {
    match compare_snapshots(&state.project_path, &query.a, &query.b) {
        Ok(comparison) => {
            let response = SnapshotCompareResponse {
                a: SnapshotSummaryResponse {
                    id: comparison.a.id,
                    name: comparison.a.name,
                    created_at: comparison.a.created_at.to_rfc3339(),
                    files: comparison.a.files,
                    symbols: comparison.a.symbols,
                    dead: comparison.a.dead,
                    cycles: comparison.a.cycles,
                },
                b: SnapshotSummaryResponse {
                    id: comparison.b.id,
                    name: comparison.b.name,
                    created_at: comparison.b.created_at.to_rfc3339(),
                    files: comparison.b.files,
                    symbols: comparison.b.symbols,
                    dead: comparison.b.dead,
                    cycles: comparison.b.cycles,
                },
                diff: SnapshotDiffResponse {
                    files: comparison.diff.files,
                    symbols: comparison.diff.symbols,
                    dead: comparison.diff.dead,
                    cycles: comparison.diff.cycles,
                },
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            if e.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": e })),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e })),
                )
                    .into_response()
            }
        }
    }
}

// =============================================================================
// SERVER
// =============================================================================

pub async fn run(project_path: PathBuf, port: u16, open_browser: bool) -> Result<()> {
    let project = Project::detect(&project_path)?;
    let project_name = project
        .root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if !trace_index_exists(&project.root) {
        eprintln!("\x1b[31m>\x1b[0m Trace index not found. Run 'greppy index' first.");
        return Err(crate::core::error::Error::IndexError {
            message: "Index not found".to_string(),
        });
    }

    eprintln!("\x1b[36m>\x1b[0m Loading index...");
    let index_path = trace_index_path(&project.root);
    let index = load_index(&index_path)?;

    // Pre-compute dead symbols
    let dead_symbols: HashSet<u32> = find_dead_symbols(&index).iter().map(|s| s.id).collect();

    let stats = index.stats();
    eprintln!(
        "\x1b[36m>\x1b[0m Loaded {} files, {} symbols ({} dead)",
        stats.files,
        stats.symbols,
        dead_symbols.len()
    );

    // Create settings state (shared between AppState and settings routes)
    let settings_state = SettingsState::new();

    let state = AppState {
        project_name,
        project_path: project.root.clone(),
        index: Arc::new(index),
        dead_symbols: Arc::new(dead_symbols),
        settings: settings_state.settings.clone(),
    };

    // Create project selector state
    let projects_state = ProjectsState {
        active_path: Arc::new(RwLock::new(project.root.clone())),
    };

    // Create events state for SSE
    let events_state = EventsState::new(project.root.clone());

    // Start daemon event forwarder in background
    let events_state_clone = events_state.clone();
    tokio::spawn(async move {
        start_daemon_event_forwarder(events_state_clone).await;
    });

    // Build sub-routers with their respective states
    let data_routes = Router::new()
        .route("/stats", get(api_stats))
        .route("/list", get(api_list))
        .route("/graph", get(api_graph))
        .route("/tree", get(api_tree))
        .route("/file/*path", get(api_file))
        // Symbol detail endpoints
        .route("/symbol/:id", get(api_symbol_detail))
        .route("/symbol/:id/callers", get(api_symbol_callers))
        .route("/symbol/:id/callees", get(api_symbol_callees))
        .route("/symbol/:id/refs", get(api_symbol_refs))
        .route("/symbol/:id/impact", get(api_symbol_impact))
        // Cycles endpoint
        .route("/cycles", get(api_cycles))
        // Snapshot/timeline endpoints
        .route(
            "/snapshots",
            get(api_list_snapshots).post(api_create_snapshot),
        )
        .route("/snapshots/compare", get(api_compare_snapshots))
        .route("/snapshots/:id", get(api_get_snapshot))
        .with_state(state);

    let projects_routes = Router::new()
        .route("/", get(api_projects))
        .route("/switch", post(api_switch_project))
        .with_state(projects_state);

    let settings_routes = Router::new()
        .route("/", get(api_get_settings).put(api_put_settings))
        .with_state(settings_state);

    let events_routes = Router::new()
        .route("/", get(api_events))
        .with_state(events_state);

    // Build main router
    let app = Router::new()
        .route("/", get(index_html))
        .route("/style.css", get(style_css))
        .route("/app.js", get(app_js))
        // Module files
        .route("/api.js", get(api_js))
        .route("/utils.js", get(utils_js))
        // Views
        .route("/views/list.js", get(views_list_js))
        .route("/views/stats.js", get(views_stats_js))
        .route("/views/graph.js", get(views_graph_js))
        .route("/views/tree.js", get(views_tree_js))
        .route("/views/tables.js", get(views_tables_js))
        .route("/views/cycles.js", get(views_cycles_js))
        .route("/views/timeline.js", get(views_timeline_js))
        // Components
        .route("/components/detail.js", get(components_detail_js))
        .route("/components/dropdown.js", get(components_dropdown_js))
        .route("/components/sse.js", get(components_sse_js))
        .route("/components/cycles.js", get(components_cycles_js))
        .route("/components/export.js", get(components_export_js))
        .route("/components/search.js", get(components_search_js))
        .route("/components/settings.js", get(components_settings_js))
        .route("/components/skeleton.js", get(components_skeleton_js))
        .route("/components/empty.js", get(components_empty_js))
        .route("/components/error.js", get(components_error_js))
        // Lib
        .route("/lib/persistence.js", get(lib_persistence_js))
        // Nest API routes with their states
        .nest("/api", data_routes)
        .nest("/api/projects", projects_routes)
        .nest("/api/settings", settings_routes)
        .nest("/api/events", events_routes);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    eprintln!();
    eprintln!(
        "\x1b[36m>\x1b[0m greppy web running at \x1b[36mhttp://{}\x1b[0m",
        addr
    );
    eprintln!("\x1b[90m  Press Ctrl+C to stop\x1b[0m");

    if open_browser {
        let url = format!("http://{}", addr);
        let _ = open::that(&url);
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
