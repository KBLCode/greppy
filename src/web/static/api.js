/**
 * API Module
 *
 * All fetch functions for communicating with the Greppy backend.
 *
 * @module api
 */

// =============================================================================
// STATS
// =============================================================================

/**
 * Fetch codebase statistics.
 * @returns {Promise<Object>} Stats object
 */
export async function fetchStats() {
  const res = await fetch('/api/stats');
  return res.json();
}

// =============================================================================
// LIST
// =============================================================================

/**
 * Fetch symbol list with optional filters.
 * @param {Object} filters - Filter options
 * @param {string} filters.type - Symbol type filter
 * @param {string} filters.state - Symbol state filter
 * @param {string} filters.search - Search query
 * @returns {Promise<Object>} List response with items array
 */
export async function fetchList(filters = {}) {
  const params = new URLSearchParams();
  if (filters.type && filters.type !== 'all') params.set('type', filters.type);
  if (filters.state && filters.state !== 'all') params.set('state', filters.state);
  if (filters.search) params.set('search', filters.search);
  params.set('limit', '500');
  
  const res = await fetch(`/api/list?${params}`);
  return res.json();
}

// =============================================================================
// GRAPH
// =============================================================================

/**
 * Fetch graph data for force-directed layout.
 * @param {Object} filters - Filter options
 * @returns {Promise<Object>} Graph with nodes and edges
 */
export async function fetchGraph(filters = {}) {
  const params = new URLSearchParams();
  if (filters.type && filters.type !== 'all') params.set('type', filters.type);
  if (filters.state && filters.state !== 'all') params.set('state', filters.state);
  
  const res = await fetch(`/api/graph?${params}`);
  return res.json();
}

/**
 * Fetch hierarchical graph data for treemap.
 * @param {string} path - Current drill-down path
 * @param {Object} filters - Filter options
 * @returns {Promise<Object>} Hierarchical data
 */
export async function fetchGraphHierarchical(path = '', filters = {}) {
  const params = new URLSearchParams();
  params.set('hierarchical', 'true');
  if (path) params.set('path', path);
  if (filters.state && filters.state !== 'all') params.set('state', filters.state);
  
  const res = await fetch(`/api/graph?${params}`);
  return res.json();
}

// =============================================================================
// TREE
// =============================================================================

/**
 * Fetch file tree structure.
 * @returns {Promise<Object>} Tree data
 */
export async function fetchTree() {
  const res = await fetch('/api/tree');
  return res.json();
}

/**
 * Fetch symbols for a specific file.
 * @param {string} path - File path
 * @returns {Promise<Object|null>} File symbols or null
 */
export async function fetchFileSymbols(path) {
  const res = await fetch(`/api/file/${encodeURIComponent(path)}`);
  if (!res.ok) return null;
  return res.json();
}

// =============================================================================
// PROJECTS
// =============================================================================

/**
 * Fetch list of available projects.
 * @returns {Promise<Object>} Projects list
 */
export async function fetchProjects() {
  const res = await fetch('/api/projects');
  return res.json();
}

/**
 * Switch to a different project.
 * @param {string} path - Project path
 * @returns {Promise<Object>} Result
 */
export async function switchProject(path) {
  const res = await fetch('/api/projects/switch', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path })
  });
  return res.json();
}

// =============================================================================
// SETTINGS
// =============================================================================

/**
 * Fetch user settings from server.
 * @returns {Promise<Object>} Settings object
 */
export async function fetchSettingsFromServer() {
  const res = await fetch('/api/settings');
  return res.json();
}

/**
 * Save settings to server.
 * @param {Object} settings - Settings to save
 * @returns {Promise<Object>} Updated settings
 */
export async function saveSettingsToServer(settings) {
  const res = await fetch('/api/settings', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(settings)
  });
  return res.json();
}

// =============================================================================
// SNAPSHOTS / TIMELINE
// =============================================================================

/**
 * Fetch list of snapshots.
 * @returns {Promise<Object>} Snapshots list with total count
 */
export async function fetchSnapshots() {
  const res = await fetch('/api/snapshots');
  return res.json();
}

/**
 * Create a new snapshot.
 * @param {string} name - Optional snapshot name
 * @returns {Promise<Object>} Created snapshot summary
 */
export async function createSnapshot(name = null) {
  const res = await fetch('/api/snapshots', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name })
  });
  return res.json();
}

/**
 * Fetch a specific snapshot by ID.
 * @param {string} id - Snapshot ID (timestamp)
 * @returns {Promise<Object>} Full snapshot data
 */
export async function fetchSnapshot(id) {
  const res = await fetch(`/api/snapshots/${encodeURIComponent(id)}`);
  return res.json();
}

/**
 * Compare two snapshots.
 * @param {string} a - First snapshot ID
 * @param {string} b - Second snapshot ID
 * @returns {Promise<Object>} Comparison with diff
 */
export async function compareSnapshots(a, b) {
  const params = new URLSearchParams({ a, b });
  const res = await fetch(`/api/snapshots/compare?${params}`);
  return res.json();
}
