/**
 * State Persistence Module
 *
 * Persists UI state to localStorage so users return to exactly where they left off.
 * All state changes are tracked and restored on page load.
 *
 * @module lib/persistence
 */

// =============================================================================
// CONSTANTS
// =============================================================================

const STORAGE_KEY = 'greppy-state';
const VERSION = 1;

// =============================================================================
// DEFAULT STATE
// =============================================================================

const defaultState = {
  version: VERSION,
  
  // View state
  currentView: 'stats',
  
  // Panel states
  detailPanelOpen: false,
  detailPanelWidth: 400,
  treePanelWidth: 250,
  treePanelCollapsed: false,
  
  // Scroll positions per view
  scrollPositions: {
    list: 0,
    stats: 0,
    graph: 0,
    tree: 0,
    tables: 0,
    cycles: 0
  },
  
  // Graph state
  graphMode: 'treemap',
  graphZoom: 1,
  graphPan: { x: 0, y: 0 },
  treemapPath: '',
  
  // Table state
  activeTable: 'symbols',
  columnWidths: {},
  columnOrder: {},
  
  // Sort state per view
  sortState: {
    list: { column: 'name', direction: 'asc' },
    tables: { column: 'name', direction: 'asc' }
  },
  
  // Filters (synced with search component)
  filters: {
    search: '',
    kind: 'all',
    state: 'all',
    file: ''
  },
  
  // User data
  recentSymbols: [],      // Last 20 viewed symbols
  pinnedSymbols: [],      // User pinned symbols
  selectedSymbolId: null,
  
  // Expanded states
  expandedTreeNodes: [],
  expandedCycles: [],
  
  // Settings
  theme: 'dark',
  density: 'comfortable',
  fontSize: 14
};

// =============================================================================
// INTERNAL STATE
// =============================================================================

/** @type {Object} Cached state in memory */
let cachedState = null;

/** @type {number|null} Debounce timeout for saves */
let saveTimeout = null;

/** @type {Set<Function>} State change listeners */
const listeners = new Set();

// =============================================================================
// CORE FUNCTIONS
// =============================================================================

/**
 * Load state from localStorage, merging with defaults.
 * @returns {Object} The current state
 */
export function loadState() {
  if (cachedState) {
    return cachedState;
  }
  
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      const parsed = JSON.parse(saved);
      
      // Version migration if needed
      if (parsed.version !== VERSION) {
        console.log('[persistence] Migrating state from version', parsed.version, 'to', VERSION);
        // For now, just merge with defaults (future: add migration logic)
      }
      
      // Deep merge with defaults to ensure all keys exist
      cachedState = deepMerge(defaultState, parsed);
      cachedState.version = VERSION;
      return cachedState;
    }
  } catch (e) {
    console.warn('[persistence] Failed to load state:', e);
  }
  
  cachedState = { ...defaultState };
  return cachedState;
}

/**
 * Save state to localStorage.
 * @param {Object} state - State to save
 */
export function saveState(state) {
  try {
    cachedState = state;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    notifyListeners(state);
  } catch (e) {
    console.warn('[persistence] Failed to save state:', e);
  }
}

/**
 * Update a single key in state and save.
 * @param {string} key - Key to update
 * @param {*} value - New value
 * @returns {Object} Updated state
 */
export function updateState(key, value) {
  const state = loadState();
  state[key] = value;
  saveState(state);
  return state;
}

/**
 * Update a nested key using dot notation (e.g., 'sortState.list').
 * @param {string} path - Dot-separated path
 * @param {*} value - New value
 * @returns {Object} Updated state
 */
export function updateNestedState(path, value) {
  const state = loadState();
  const keys = path.split('.');
  let current = state;
  
  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i];
    if (!(key in current) || typeof current[key] !== 'object') {
      current[key] = {};
    }
    current = current[key];
  }
  
  current[keys[keys.length - 1]] = value;
  saveState(state);
  return state;
}

/**
 * Debounced save for frequent updates (scroll, zoom, drag).
 * @param {Object} state - State to save
 * @param {number} delay - Debounce delay in ms (default: 300)
 */
export function debouncedSave(state, delay = 300) {
  cachedState = state;
  
  if (saveTimeout) {
    clearTimeout(saveTimeout);
  }
  
  saveTimeout = setTimeout(() => {
    saveState(state);
    saveTimeout = null;
  }, delay);
}

/**
 * Force immediate save (e.g., before page unload).
 */
export function flushSave() {
  if (saveTimeout) {
    clearTimeout(saveTimeout);
    saveTimeout = null;
  }
  
  if (cachedState) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(cachedState));
    } catch (e) {
      console.warn('[persistence] Failed to flush save:', e);
    }
  }
}

// =============================================================================
// RECENT SYMBOLS
// =============================================================================

const MAX_RECENT = 20;

/**
 * Add a symbol to recent history.
 * @param {Object} symbol - Symbol to track (must have id and name)
 * @returns {Array} Updated recent symbols list
 */
export function trackRecentSymbol(symbol) {
  if (!symbol || !symbol.id) return loadState().recentSymbols;
  
  const state = loadState();
  const recent = state.recentSymbols || [];
  
  // Create minimal symbol record
  const record = {
    id: symbol.id,
    name: symbol.name || 'unknown',
    path: symbol.path || symbol.file || '',
    type: symbol.type || symbol.kind || '',
    viewedAt: Date.now()
  };
  
  // Remove existing entry for same symbol
  const filtered = recent.filter(s => s.id !== symbol.id);
  
  // Add to front, limit to MAX_RECENT
  const updated = [record, ...filtered].slice(0, MAX_RECENT);
  
  state.recentSymbols = updated;
  saveState(state);
  
  return updated;
}

/**
 * Get recent symbols list.
 * @returns {Array} Recent symbols
 */
export function getRecentSymbols() {
  return loadState().recentSymbols || [];
}

/**
 * Clear recent symbols.
 */
export function clearRecentSymbols() {
  updateState('recentSymbols', []);
}

// =============================================================================
// PINNED SYMBOLS
// =============================================================================

/**
 * Pin a symbol.
 * @param {Object} symbol - Symbol to pin
 * @returns {Array} Updated pinned list
 */
export function pinSymbol(symbol) {
  if (!symbol || !symbol.id) return loadState().pinnedSymbols;
  
  const state = loadState();
  const pinned = state.pinnedSymbols || [];
  
  // Check if already pinned
  if (pinned.some(s => s.id === symbol.id)) {
    return pinned;
  }
  
  const record = {
    id: symbol.id,
    name: symbol.name || 'unknown',
    path: symbol.path || symbol.file || '',
    type: symbol.type || symbol.kind || '',
    pinnedAt: Date.now()
  };
  
  const updated = [...pinned, record];
  state.pinnedSymbols = updated;
  saveState(state);
  
  return updated;
}

/**
 * Unpin a symbol.
 * @param {number|string} symbolId - Symbol ID to unpin
 * @returns {Array} Updated pinned list
 */
export function unpinSymbol(symbolId) {
  const state = loadState();
  const updated = (state.pinnedSymbols || []).filter(s => s.id !== symbolId);
  state.pinnedSymbols = updated;
  saveState(state);
  return updated;
}

/**
 * Check if a symbol is pinned.
 * @param {number|string} symbolId - Symbol ID
 * @returns {boolean}
 */
export function isSymbolPinned(symbolId) {
  const pinned = loadState().pinnedSymbols || [];
  return pinned.some(s => s.id === symbolId);
}

/**
 * Get pinned symbols.
 * @returns {Array}
 */
export function getPinnedSymbols() {
  return loadState().pinnedSymbols || [];
}

// =============================================================================
// SCROLL POSITIONS
// =============================================================================

/**
 * Save scroll position for a view.
 * @param {string} view - View name
 * @param {number} position - Scroll position
 */
export function saveScrollPosition(view, position) {
  const state = loadState();
  if (!state.scrollPositions) {
    state.scrollPositions = {};
  }
  state.scrollPositions[view] = position;
  debouncedSave(state);
}

/**
 * Get scroll position for a view.
 * @param {string} view - View name
 * @returns {number} Scroll position (0 if not saved)
 */
export function getScrollPosition(view) {
  const state = loadState();
  return state.scrollPositions?.[view] || 0;
}

/**
 * Restore scroll position for current view.
 * @param {string} view - View name
 */
export function restoreScrollPosition(view) {
  const position = getScrollPosition(view);
  if (position > 0) {
    // Use requestAnimationFrame to ensure DOM is ready
    requestAnimationFrame(() => {
      window.scrollTo({ top: position, behavior: 'instant' });
    });
  }
}

// =============================================================================
// EXPANDED STATES
// =============================================================================

/**
 * Toggle tree node expansion state.
 * @param {string} nodePath - Tree node path
 * @param {boolean} expanded - Whether expanded
 */
export function setTreeNodeExpanded(nodePath, expanded) {
  const state = loadState();
  const nodes = new Set(state.expandedTreeNodes || []);
  
  if (expanded) {
    nodes.add(nodePath);
  } else {
    nodes.delete(nodePath);
  }
  
  state.expandedTreeNodes = Array.from(nodes);
  saveState(state);
}

/**
 * Check if a tree node is expanded.
 * @param {string} nodePath - Tree node path
 * @returns {boolean}
 */
export function isTreeNodeExpanded(nodePath) {
  const state = loadState();
  return (state.expandedTreeNodes || []).includes(nodePath);
}

/**
 * Get all expanded tree nodes.
 * @returns {Array<string>}
 */
export function getExpandedTreeNodes() {
  return loadState().expandedTreeNodes || [];
}

// =============================================================================
// LISTENERS
// =============================================================================

/**
 * Subscribe to state changes.
 * @param {Function} callback - Called with new state on changes
 * @returns {Function} Unsubscribe function
 */
export function onStateChange(callback) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

/**
 * Notify all listeners of state change.
 * @param {Object} state - New state
 */
function notifyListeners(state) {
  listeners.forEach(fn => {
    try {
      fn(state);
    } catch (e) {
      console.warn('[persistence] Listener error:', e);
    }
  });
}

// =============================================================================
// UTILITIES
// =============================================================================

/**
 * Deep merge two objects.
 * @param {Object} target - Target object (defaults)
 * @param {Object} source - Source object (saved state)
 * @returns {Object} Merged object
 */
function deepMerge(target, source) {
  const result = { ...target };
  
  for (const key of Object.keys(source)) {
    if (source[key] === null || source[key] === undefined) {
      continue;
    }
    
    if (
      typeof source[key] === 'object' &&
      !Array.isArray(source[key]) &&
      typeof target[key] === 'object' &&
      !Array.isArray(target[key])
    ) {
      result[key] = deepMerge(target[key], source[key]);
    } else {
      result[key] = source[key];
    }
  }
  
  return result;
}

/**
 * Reset state to defaults.
 */
export function resetState() {
  cachedState = { ...defaultState };
  saveState(cachedState);
}

/**
 * Get default state (for reference).
 * @returns {Object} Default state object
 */
export function getDefaultState() {
  return { ...defaultState };
}

// =============================================================================
// PAGE LIFECYCLE
// =============================================================================

// Save on page unload
if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', flushSave);
  window.addEventListener('pagehide', flushSave);
}
