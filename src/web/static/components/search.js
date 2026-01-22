/**
 * Search Component
 *
 * Advanced search with smart syntax parsing, filter presets, and quick filters.
 * Data operations only - no opinions, just matching results.
 *
 * @module components/search
 */

import { updateNestedState, loadState as loadPersistedState } from '../lib/persistence.js';

// =============================================================================
// CONSTANTS
// =============================================================================

const STORAGE_KEY = 'greppy-filter-presets';

const DEFAULT_PRESETS = [
  { id: 'dead-functions', name: 'Dead Functions', query: 'kind:function state:dead' },
  { id: 'cycle-symbols', name: 'Cycle Symbols', query: 'state:cycle' },
  { id: 'entry-points', name: 'Entry Points', query: 'entry:true' },
  { id: 'no-callers', name: 'No Callers', query: 'callers:0' }
];

const QUICK_FILTERS = [
  { label: 'functions', filter: { kind: 'function' } },
  { label: 'structs', filter: { kind: 'struct' } },
  { label: 'dead', filter: { state: 'dead' } },
  { label: 'cycles', filter: { state: 'cycle' } },
  { label: 'entry points', filter: { entry: true } },
  { label: 'no callers', filter: { maxCallers: 0 } }
];

const KIND_OPTIONS = [
  { value: 'all', label: 'all kinds' },
  { value: 'function', label: 'functions' },
  { value: 'method', label: 'methods' },
  { value: 'class', label: 'classes' },
  { value: 'struct', label: 'structs' },
  { value: 'enum', label: 'enums' },
  { value: 'interface', label: 'interfaces' },
  { value: 'type', label: 'types' },
  { value: 'variable', label: 'variables' }
];

const STATE_OPTIONS = [
  { value: 'all', label: 'all states' },
  { value: 'used', label: 'used' },
  { value: 'dead', label: 'dead' },
  { value: 'cycle', label: 'in cycle' },
  { value: 'entry', label: 'entry points' }
];

const FILE_OPTIONS = [
  { value: '', label: 'all files' }
];

const REFS_OPTIONS = [
  { value: 'all', label: 'all refs' },
  { value: '0', label: '0 refs' },
  { value: '1-5', label: '1-5 refs' },
  { value: '6-20', label: '6-20 refs' },
  { value: '20+', label: '20+ refs' }
];

// =============================================================================
// FILTER STATE
// =============================================================================

/**
 * Global filter state - exported for views to consume.
 * Views should read this state to filter their data.
 */
export const filters = {
  search: '',
  kind: 'all',
  state: 'all',
  file: '',
  minRefs: null,
  maxRefs: null,
  hasCallers: null,
  hasCallees: null,
  entry: null
};

/**
 * Listeners for filter changes.
 * @type {Set<Function>}
 */
const listeners = new Set();

/**
 * Subscribe to filter changes.
 * @param {Function} callback - Called when filters change
 * @returns {Function} Unsubscribe function
 */
export function onFilterChange(callback) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

/**
 * Notify all listeners of filter change.
 */
function notifyListeners() {
  listeners.forEach(fn => fn({ ...filters }));
}

/**
 * Update filters and notify listeners.
 * @param {Object} updates - Partial filter updates
 */
export function updateFilters(updates) {
  Object.assign(filters, updates);
  
  // Persist filter state
  updateNestedState('filters', {
    search: filters.search,
    kind: filters.kind,
    state: filters.state,
    file: filters.file
  });
  
  notifyListeners();
}

/**
 * Reset all filters to defaults.
 */
export function resetFilters() {
  filters.search = '';
  filters.kind = 'all';
  filters.state = 'all';
  filters.file = '';
  filters.minRefs = null;
  filters.maxRefs = null;
  filters.hasCallers = null;
  filters.hasCallees = null;
  filters.entry = null;
  
  // Persist reset state
  updateNestedState('filters', {
    search: '',
    kind: 'all',
    state: 'all',
    file: ''
  });
  
  notifyListeners();
}

/**
 * Load persisted filters on init.
 */
export function loadPersistedFilters() {
  const persisted = loadPersistedState();
  if (persisted.filters) {
    filters.search = persisted.filters.search || '';
    filters.kind = persisted.filters.kind || 'all';
    filters.state = persisted.filters.state || 'all';
    filters.file = persisted.filters.file || '';
  }
}

/**
 * Check if any filters are active.
 * @returns {boolean}
 */
export function hasActiveFilters() {
  return (
    filters.search !== '' ||
    filters.kind !== 'all' ||
    filters.state !== 'all' ||
    filters.file !== '' ||
    filters.minRefs !== null ||
    filters.maxRefs !== null ||
    filters.hasCallers !== null ||
    filters.hasCallees !== null ||
    filters.entry !== null
  );
}

/**
 * Get active filters as a display-friendly array.
 * @returns {Array<{key: string, value: string, label: string}>}
 */
export function getActiveFiltersList() {
  const active = [];
  
  if (filters.kind !== 'all') {
    active.push({ key: 'kind', value: filters.kind, label: `kind:${filters.kind}` });
  }
  if (filters.state !== 'all') {
    active.push({ key: 'state', value: filters.state, label: `state:${filters.state}` });
  }
  if (filters.file) {
    active.push({ key: 'file', value: filters.file, label: `file:${filters.file}` });
  }
  if (filters.minRefs !== null) {
    active.push({ key: 'minRefs', value: filters.minRefs, label: `refs:>${filters.minRefs}` });
  }
  if (filters.maxRefs !== null) {
    active.push({ key: 'maxRefs', value: filters.maxRefs, label: `refs:<${filters.maxRefs}` });
  }
  if (filters.hasCallers === true) {
    active.push({ key: 'hasCallers', value: true, label: 'has:callers' });
  }
  if (filters.hasCallers === false) {
    active.push({ key: 'hasCallers', value: false, label: 'callers:0' });
  }
  if (filters.hasCallees === true) {
    active.push({ key: 'hasCallees', value: true, label: 'has:callees' });
  }
  if (filters.hasCallees === false) {
    active.push({ key: 'hasCallees', value: false, label: 'callees:0' });
  }
  if (filters.entry === true) {
    active.push({ key: 'entry', value: true, label: 'entry:true' });
  }
  
  return active;
}

/**
 * Remove a specific filter.
 * @param {string} key - Filter key to remove
 */
export function removeFilter(key) {
  switch (key) {
    case 'kind':
      filters.kind = 'all';
      break;
    case 'state':
      filters.state = 'all';
      break;
    case 'file':
      filters.file = '';
      break;
    case 'minRefs':
      filters.minRefs = null;
      break;
    case 'maxRefs':
      filters.maxRefs = null;
      break;
    case 'hasCallers':
      filters.hasCallers = null;
      break;
    case 'hasCallees':
      filters.hasCallees = null;
      break;
    case 'entry':
      filters.entry = null;
      break;
  }
  notifyListeners();
}

// =============================================================================
// SEARCH SYNTAX PARSING
// =============================================================================

/**
 * Parse search query with smart syntax.
 * 
 * Supported syntax:
 *   trace              - name contains "trace"
 *   kind:function      - only functions
 *   kind:struct        - only structs
 *   state:dead         - only dead symbols
 *   state:used         - only used symbols
 *   state:cycle        - only symbols in cycles
 *   file:src/trace/*   - in specific path
 *   refs:>10           - more than 10 references
 *   refs:<5            - less than 5 references
 *   callers:0          - no callers
 *   callees:>5         - more than 5 callees
 *   entry:true         - only entry points
 *   has:callers        - has at least one caller
 *   has:callees        - has at least one callee
 * 
 * @param {string} query - Raw search query
 * @returns {Object} Parsed filter object
 */
export function parseSearchQuery(query) {
  const result = {
    search: '',
    kind: 'all',
    state: 'all',
    file: '',
    minRefs: null,
    maxRefs: null,
    hasCallers: null,
    hasCallees: null,
    entry: null
  };
  
  if (!query || typeof query !== 'string') {
    return result;
  }
  
  const tokens = query.trim().split(/\s+/);
  const textParts = [];
  
  for (const token of tokens) {
    const lower = token.toLowerCase();
    
    // kind:value
    if (lower.startsWith('kind:')) {
      const value = token.slice(5);
      if (KIND_OPTIONS.some(o => o.value === value)) {
        result.kind = value;
      }
      continue;
    }
    
    // state:value
    if (lower.startsWith('state:')) {
      const value = token.slice(6);
      if (['used', 'dead', 'cycle', 'entry'].includes(value)) {
        result.state = value;
      }
      continue;
    }
    
    // file:path
    if (lower.startsWith('file:')) {
      result.file = token.slice(5);
      continue;
    }
    
    // refs:>N or refs:<N or refs:N
    if (lower.startsWith('refs:')) {
      const value = token.slice(5);
      if (value.startsWith('>')) {
        result.minRefs = parseInt(value.slice(1), 10) || null;
      } else if (value.startsWith('<')) {
        result.maxRefs = parseInt(value.slice(1), 10) || null;
      } else {
        const num = parseInt(value, 10);
        if (!isNaN(num)) {
          result.minRefs = num;
          result.maxRefs = num;
        }
      }
      continue;
    }
    
    // callers:N or callers:>N
    if (lower.startsWith('callers:')) {
      const value = token.slice(8);
      if (value === '0') {
        result.hasCallers = false;
      } else if (value.startsWith('>')) {
        result.hasCallers = true;
      } else {
        const num = parseInt(value, 10);
        if (!isNaN(num) && num === 0) {
          result.hasCallers = false;
        } else if (!isNaN(num) && num > 0) {
          result.hasCallers = true;
        }
      }
      continue;
    }
    
    // callees:N or callees:>N
    if (lower.startsWith('callees:')) {
      const value = token.slice(8);
      if (value === '0') {
        result.hasCallees = false;
      } else if (value.startsWith('>')) {
        result.hasCallees = true;
      } else {
        const num = parseInt(value, 10);
        if (!isNaN(num) && num === 0) {
          result.hasCallees = false;
        } else if (!isNaN(num) && num > 0) {
          result.hasCallees = true;
        }
      }
      continue;
    }
    
    // entry:true/false
    if (lower.startsWith('entry:')) {
      const value = token.slice(6).toLowerCase();
      result.entry = value === 'true';
      continue;
    }
    
    // has:callers/callees
    if (lower.startsWith('has:')) {
      const value = token.slice(4).toLowerCase();
      if (value === 'callers') {
        result.hasCallers = true;
      } else if (value === 'callees') {
        result.hasCallees = true;
      }
      continue;
    }
    
    // Plain text search
    textParts.push(token);
  }
  
  result.search = textParts.join(' ');
  return result;
}

/**
 * Apply parsed query to filters.
 * @param {string} query - Raw search query
 */
export function applySearchQuery(query) {
  const parsed = parseSearchQuery(query);
  Object.assign(filters, parsed);
  notifyListeners();
}

// =============================================================================
// FILTER PRESETS
// =============================================================================

/**
 * Load presets from localStorage.
 * @returns {Array}
 */
export function loadPresets() {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Failed to load presets:', e);
  }
  return [...DEFAULT_PRESETS];
}

/**
 * Save presets to localStorage.
 * @param {Array} presets
 */
export function savePresets(presets) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(presets));
  } catch (e) {
    console.error('Failed to save presets:', e);
  }
}

/**
 * Add a new preset.
 * @param {string} name - Preset name
 * @param {string} query - Filter query string
 * @returns {Object} The created preset
 */
export function addPreset(name, query) {
  const presets = loadPresets();
  const preset = {
    id: `preset-${Date.now()}`,
    name,
    query
  };
  presets.push(preset);
  savePresets(presets);
  return preset;
}

/**
 * Remove a preset.
 * @param {string} id - Preset ID
 */
export function removePreset(id) {
  const presets = loadPresets().filter(p => p.id !== id);
  savePresets(presets);
}

/**
 * Get current filters as query string for saving.
 * @returns {string}
 */
export function filtersToQueryString() {
  const parts = [];
  
  if (filters.search) {
    parts.push(filters.search);
  }
  if (filters.kind !== 'all') {
    parts.push(`kind:${filters.kind}`);
  }
  if (filters.state !== 'all') {
    parts.push(`state:${filters.state}`);
  }
  if (filters.file) {
    parts.push(`file:${filters.file}`);
  }
  if (filters.minRefs !== null) {
    parts.push(`refs:>${filters.minRefs}`);
  }
  if (filters.hasCallers === true) {
    parts.push('has:callers');
  }
  if (filters.hasCallers === false) {
    parts.push('callers:0');
  }
  if (filters.hasCallees === true) {
    parts.push('has:callees');
  }
  if (filters.hasCallees === false) {
    parts.push('callees:0');
  }
  if (filters.entry === true) {
    parts.push('entry:true');
  }
  
  return parts.join(' ');
}

// =============================================================================
// SYMBOL FILTERING
// =============================================================================

/**
 * Check if a symbol matches current filters.
 * @param {Object} symbol - Symbol object
 * @returns {boolean}
 */
export function matchesFilters(symbol) {
  // Text search
  if (filters.search) {
    const searchLower = filters.search.toLowerCase();
    const nameLower = (symbol.name || '').toLowerCase();
    const pathLower = (symbol.path || symbol.file || '').toLowerCase();
    if (!nameLower.includes(searchLower) && !pathLower.includes(searchLower)) {
      return false;
    }
  }
  
  // Kind filter
  if (filters.kind !== 'all') {
    const symbolKind = (symbol.kind || symbol.type || '').toLowerCase();
    if (symbolKind !== filters.kind) {
      return false;
    }
  }
  
  // State filter
  if (filters.state !== 'all') {
    switch (filters.state) {
      case 'dead':
        if (!symbol.dead && symbol.refs !== 0) return false;
        break;
      case 'used':
        if (symbol.dead || symbol.refs === 0) return false;
        break;
      case 'cycle':
        if (!symbol.in_cycle && !symbol.inCycle) return false;
        break;
      case 'entry':
        if (!symbol.entry && !symbol.is_entry) return false;
        break;
    }
  }
  
  // File filter (glob-like)
  if (filters.file) {
    const path = symbol.path || symbol.file || '';
    if (!pathMatchesGlob(path, filters.file)) {
      return false;
    }
  }
  
  // Refs filters
  const refs = symbol.refs ?? symbol.references ?? 0;
  if (filters.minRefs !== null && refs < filters.minRefs) {
    return false;
  }
  if (filters.maxRefs !== null && refs > filters.maxRefs) {
    return false;
  }
  
  // Callers filter
  const callers = symbol.callers ?? symbol.caller_count ?? 0;
  if (filters.hasCallers === true && callers === 0) {
    return false;
  }
  if (filters.hasCallers === false && callers > 0) {
    return false;
  }
  
  // Callees filter
  const callees = symbol.callees ?? symbol.callee_count ?? 0;
  if (filters.hasCallees === true && callees === 0) {
    return false;
  }
  if (filters.hasCallees === false && callees > 0) {
    return false;
  }
  
  // Entry point filter
  if (filters.entry === true) {
    if (!symbol.entry && !symbol.is_entry) {
      return false;
    }
  }
  
  return true;
}

/**
 * Simple glob matching for file paths.
 * Supports * and ** wildcards.
 * @param {string} path - File path to check
 * @param {string} pattern - Glob pattern
 * @returns {boolean}
 */
function pathMatchesGlob(path, pattern) {
  // Convert glob to regex
  const regexPattern = pattern
    .replace(/[.+^${}()|[\]\\]/g, '\\$&') // Escape special chars except * and ?
    .replace(/\*\*/g, '{{DOUBLESTAR}}')   // Placeholder for **
    .replace(/\*/g, '[^/]*')              // * matches anything except /
    .replace(/{{DOUBLESTAR}}/g, '.*')     // ** matches anything including /
    .replace(/\?/g, '.');                 // ? matches single char
  
  const regex = new RegExp(`^${regexPattern}`, 'i');
  return regex.test(path);
}

/**
 * Filter an array of symbols.
 * @param {Array} symbols - Array of symbol objects
 * @returns {Array} Filtered symbols
 */
export function filterSymbols(symbols) {
  if (!Array.isArray(symbols)) return [];
  return symbols.filter(matchesFilters);
}

// =============================================================================
// SEARCH COMPONENT CLASS
// =============================================================================

/**
 * Search component with enhanced search bar, quick filters, and presets.
 */
export class SearchComponent {
  /**
   * Create search component.
   * @param {Object} options - Configuration
   * @param {HTMLElement} options.searchContainer - Container for search bar
   * @param {HTMLElement} options.quickFilterContainer - Container for quick filters
   * @param {HTMLElement} options.presetsContainer - Container for presets (optional)
   * @param {HTMLElement} options.activeFiltersContainer - Container for active filters
   * @param {Function} options.onFilterChange - Callback when filters change
   */
  constructor(options = {}) {
    this.searchContainer = options.searchContainer;
    this.quickFilterContainer = options.quickFilterContainer;
    this.presetsContainer = options.presetsContainer;
    this.activeFiltersContainer = options.activeFiltersContainer;
    this.onFilterChangeCallback = options.onFilterChange || (() => {});
    
    this.searchTimeout = null;
    this.presets = loadPresets();
    
    // Load persisted filters before rendering
    loadPersistedFilters();
    
    // Subscribe to filter changes
    onFilterChange(() => {
      this.renderActiveFilters();
      this.onFilterChangeCallback({ ...filters });
    });
    
    this.render();
    this.bindEvents();
  }
  
  /**
   * Render all component parts.
   */
  render() {
    this.renderSearchBar();
    this.renderQuickFilters();
    this.renderActiveFilters();
    if (this.presetsContainer) {
      this.renderPresets();
    }
  }
  
  /**
   * Render the search bar.
   */
  renderSearchBar() {
    if (!this.searchContainer) return;
    
    this.searchContainer.innerHTML = `
      <div class="search-bar">
        <div class="search-input-wrapper">
          <input 
            type="text" 
            id="search-input" 
            class="search-input" 
            placeholder="Search symbols... (kind:function state:dead)"
            value="${escapeHtml(filters.search)}"
          >
          <button class="search-clear ${filters.search ? '' : 'hidden'}" title="Clear search">×</button>
        </div>
        <div class="search-dropdowns">
          <div class="search-dropdown" data-filter="kind">
            <button class="search-dropdown-trigger" type="button">
              <span class="search-dropdown-label">Kind</span>
              <span class="search-dropdown-value">${this.getKindLabel()}</span>
              <span class="search-dropdown-arrow">&#9660;</span>
            </button>
            <div class="search-dropdown-menu">
              ${KIND_OPTIONS.map(opt => `
                <div class="search-dropdown-item ${opt.value === filters.kind ? 'selected' : ''}" data-value="${opt.value}">
                  ${opt.label}
                </div>
              `).join('')}
            </div>
          </div>
          <div class="search-dropdown" data-filter="state">
            <button class="search-dropdown-trigger" type="button">
              <span class="search-dropdown-label">State</span>
              <span class="search-dropdown-value">${this.getStateLabel()}</span>
              <span class="search-dropdown-arrow">&#9660;</span>
            </button>
            <div class="search-dropdown-menu">
              ${STATE_OPTIONS.map(opt => `
                <div class="search-dropdown-item ${opt.value === filters.state ? 'selected' : ''}" data-value="${opt.value}">
                  ${opt.label}
                </div>
              `).join('')}
            </div>
          </div>
          <div class="search-dropdown" data-filter="refs">
            <button class="search-dropdown-trigger" type="button">
              <span class="search-dropdown-label">Refs</span>
              <span class="search-dropdown-value">${this.getRefsLabel()}</span>
              <span class="search-dropdown-arrow">&#9660;</span>
            </button>
            <div class="search-dropdown-menu">
              ${REFS_OPTIONS.map(opt => `
                <div class="search-dropdown-item ${this.isRefsSelected(opt.value) ? 'selected' : ''}" data-value="${opt.value}">
                  ${opt.label}
                </div>
              `).join('')}
            </div>
          </div>
        </div>
      </div>
    `;
  }
  
  /**
   * Render quick filter tags.
   */
  renderQuickFilters() {
    if (!this.quickFilterContainer) return;
    
    this.quickFilterContainer.innerHTML = `
      <div class="quick-filters">
        <span class="quick-filters-label">Quick:</span>
        ${QUICK_FILTERS.map(qf => `
          <button class="quick-filter-tag ${this.isQuickFilterActive(qf) ? 'active' : ''}" data-filter='${JSON.stringify(qf.filter)}'>
            ${qf.label}
          </button>
        `).join('')}
      </div>
    `;
  }
  
  /**
   * Render active filters display.
   */
  renderActiveFilters() {
    if (!this.activeFiltersContainer) return;
    
    const activeList = getActiveFiltersList();
    
    if (activeList.length === 0) {
      this.activeFiltersContainer.innerHTML = '';
      this.activeFiltersContainer.classList.add('hidden');
      return;
    }
    
    this.activeFiltersContainer.classList.remove('hidden');
    this.activeFiltersContainer.innerHTML = `
      <div class="active-filters">
        <span class="active-filters-label">Active:</span>
        ${activeList.map(f => `
          <span class="active-filter-tag" data-key="${f.key}">
            ${escapeHtml(f.label)}
            <button class="active-filter-remove" title="Remove filter">×</button>
          </span>
        `).join('')}
        <button class="active-filters-clear" title="Clear all filters">[Clear All]</button>
      </div>
    `;
  }
  
  /**
   * Render presets panel.
   */
  renderPresets() {
    if (!this.presetsContainer) return;
    
    this.presets = loadPresets();
    
    this.presetsContainer.innerHTML = `
      <div class="search-presets">
        <div class="presets-header">
          <span class="presets-title">Saved Filters</span>
          <button class="presets-add" title="Save current filters">[+ Save Current]</button>
        </div>
        <div class="presets-list">
          ${this.presets.map(p => `
            <div class="preset-item" data-id="${p.id}" data-query="${escapeHtml(p.query)}">
              <span class="preset-dot">●</span>
              <span class="preset-name">${escapeHtml(p.name)}</span>
              <span class="preset-query">${escapeHtml(p.query)}</span>
              <button class="preset-delete" data-id="${p.id}" title="Delete preset">×</button>
            </div>
          `).join('')}
        </div>
      </div>
    `;
  }
  
  /**
   * Bind event handlers.
   */
  bindEvents() {
    // Search input
    const searchInput = document.getElementById('search-input');
    if (searchInput) {
      searchInput.addEventListener('input', (e) => {
        clearTimeout(this.searchTimeout);
        this.searchTimeout = setTimeout(() => {
          applySearchQuery(e.target.value);
          this.updateSearchClearButton();
        }, 300);
      });
      
      searchInput.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
          searchInput.blur();
        }
      });
    }
    
    // Search clear button
    this.searchContainer?.addEventListener('click', (e) => {
      if (e.target.classList.contains('search-clear')) {
        const input = document.getElementById('search-input');
        if (input) {
          input.value = '';
          resetFilters();
          this.updateSearchClearButton();
          input.focus();
        }
      }
    });
    
    // Dropdowns
    this.searchContainer?.addEventListener('click', (e) => {
      const trigger = e.target.closest('.search-dropdown-trigger');
      if (trigger) {
        e.stopPropagation();
        const dropdown = trigger.closest('.search-dropdown');
        this.toggleDropdown(dropdown);
      }
      
      const item = e.target.closest('.search-dropdown-item');
      if (item) {
        const dropdown = item.closest('.search-dropdown');
        const filterType = dropdown.dataset.filter;
        const value = item.dataset.value;
        this.handleDropdownSelect(filterType, value);
        this.closeAllDropdowns();
      }
    });
    
    // Quick filters
    this.quickFilterContainer?.addEventListener('click', (e) => {
      const tag = e.target.closest('.quick-filter-tag');
      if (tag) {
        const filter = JSON.parse(tag.dataset.filter);
        this.toggleQuickFilter(filter, tag);
      }
    });
    
    // Active filters
    this.activeFiltersContainer?.addEventListener('click', (e) => {
      if (e.target.classList.contains('active-filter-remove')) {
        const tag = e.target.closest('.active-filter-tag');
        if (tag) {
          removeFilter(tag.dataset.key);
        }
      }
      
      if (e.target.classList.contains('active-filters-clear')) {
        resetFilters();
        const input = document.getElementById('search-input');
        if (input) input.value = '';
      }
    });
    
    // Presets
    this.presetsContainer?.addEventListener('click', (e) => {
      const presetItem = e.target.closest('.preset-item');
      const deleteBtn = e.target.closest('.preset-delete');
      const addBtn = e.target.closest('.presets-add');
      
      if (deleteBtn) {
        e.stopPropagation();
        removePreset(deleteBtn.dataset.id);
        this.renderPresets();
      } else if (presetItem) {
        const query = presetItem.dataset.query;
        const input = document.getElementById('search-input');
        if (input) input.value = query;
        applySearchQuery(query);
      } else if (addBtn) {
        this.showSavePresetDialog();
      }
    });
    
    // Close dropdowns on outside click
    document.addEventListener('click', () => {
      this.closeAllDropdowns();
    });
    
    // Keyboard shortcut
    document.addEventListener('keydown', (e) => {
      if (e.key === '/' && e.target.tagName !== 'INPUT') {
        e.preventDefault();
        document.getElementById('search-input')?.focus();
      }
    });
  }
  
  /**
   * Toggle a dropdown open/closed.
   * @param {HTMLElement} dropdown
   */
  toggleDropdown(dropdown) {
    const wasOpen = dropdown.classList.contains('open');
    this.closeAllDropdowns();
    if (!wasOpen) {
      dropdown.classList.add('open');
    }
  }
  
  /**
   * Close all dropdowns.
   */
  closeAllDropdowns() {
    document.querySelectorAll('.search-dropdown.open').forEach(d => {
      d.classList.remove('open');
    });
  }
  
  /**
   * Handle dropdown selection.
   * @param {string} filterType
   * @param {string} value
   */
  handleDropdownSelect(filterType, value) {
    switch (filterType) {
      case 'kind':
        updateFilters({ kind: value });
        break;
      case 'state':
        updateFilters({ state: value });
        break;
      case 'refs':
        this.handleRefsSelect(value);
        break;
    }
    this.renderSearchBar();
    this.renderQuickFilters();
  }
  
  /**
   * Handle refs dropdown selection.
   * @param {string} value
   */
  handleRefsSelect(value) {
    if (value === 'all') {
      updateFilters({ minRefs: null, maxRefs: null });
    } else if (value === '0') {
      updateFilters({ minRefs: 0, maxRefs: 0 });
    } else if (value === '1-5') {
      updateFilters({ minRefs: 1, maxRefs: 5 });
    } else if (value === '6-20') {
      updateFilters({ minRefs: 6, maxRefs: 20 });
    } else if (value === '20+') {
      updateFilters({ minRefs: 20, maxRefs: null });
    }
  }
  
  /**
   * Toggle a quick filter on/off.
   * @param {Object} filter
   * @param {HTMLElement} tag
   */
  toggleQuickFilter(filter, tag) {
    const isActive = tag.classList.contains('active');
    
    if (isActive) {
      // Remove filter
      const updates = {};
      for (const key of Object.keys(filter)) {
        if (key === 'kind') updates.kind = 'all';
        else if (key === 'state') updates.state = 'all';
        else if (key === 'entry') updates.entry = null;
        else if (key === 'maxCallers') updates.hasCallers = null;
      }
      updateFilters(updates);
    } else {
      // Apply filter
      const updates = {};
      for (const [key, value] of Object.entries(filter)) {
        if (key === 'kind') updates.kind = value;
        else if (key === 'state') updates.state = value;
        else if (key === 'entry') updates.entry = value;
        else if (key === 'maxCallers' && value === 0) updates.hasCallers = false;
      }
      updateFilters(updates);
    }
    
    this.renderSearchBar();
    this.renderQuickFilters();
  }
  
  /**
   * Check if a quick filter is active.
   * @param {Object} qf - Quick filter config
   * @returns {boolean}
   */
  isQuickFilterActive(qf) {
    const f = qf.filter;
    
    if (f.kind && filters.kind !== f.kind) return false;
    if (f.state && filters.state !== f.state) return false;
    if (f.entry !== undefined && filters.entry !== f.entry) return false;
    if (f.maxCallers === 0 && filters.hasCallers !== false) return false;
    
    // Check if ANY of the filter conditions match
    if (f.kind && filters.kind === f.kind) return true;
    if (f.state && filters.state === f.state) return true;
    if (f.entry !== undefined && filters.entry === f.entry) return true;
    if (f.maxCallers === 0 && filters.hasCallers === false) return true;
    
    return false;
  }
  
  /**
   * Update search clear button visibility.
   */
  updateSearchClearButton() {
    const clearBtn = this.searchContainer?.querySelector('.search-clear');
    const input = document.getElementById('search-input');
    if (clearBtn && input) {
      clearBtn.classList.toggle('hidden', !input.value);
    }
  }
  
  /**
   * Show dialog to save current filters as preset.
   */
  showSavePresetDialog() {
    const query = filtersToQueryString();
    if (!query) {
      alert('No active filters to save');
      return;
    }
    
    const name = prompt('Enter preset name:', '');
    if (name && name.trim()) {
      addPreset(name.trim(), query);
      this.renderPresets();
    }
  }
  
  /**
   * Get display label for current kind filter.
   * @returns {string}
   */
  getKindLabel() {
    const opt = KIND_OPTIONS.find(o => o.value === filters.kind);
    return opt?.label || filters.kind;
  }
  
  /**
   * Get display label for current state filter.
   * @returns {string}
   */
  getStateLabel() {
    const opt = STATE_OPTIONS.find(o => o.value === filters.state);
    return opt?.label || filters.state;
  }
  
  /**
   * Get display label for current refs filter.
   * @returns {string}
   */
  getRefsLabel() {
    if (filters.minRefs === null && filters.maxRefs === null) return 'all refs';
    if (filters.minRefs === 0 && filters.maxRefs === 0) return '0 refs';
    if (filters.minRefs === 1 && filters.maxRefs === 5) return '1-5 refs';
    if (filters.minRefs === 6 && filters.maxRefs === 20) return '6-20 refs';
    if (filters.minRefs === 20 && filters.maxRefs === null) return '20+ refs';
    return 'custom';
  }
  
  /**
   * Check if a refs option is selected.
   * @param {string} value
   * @returns {boolean}
   */
  isRefsSelected(value) {
    if (value === 'all') return filters.minRefs === null && filters.maxRefs === null;
    if (value === '0') return filters.minRefs === 0 && filters.maxRefs === 0;
    if (value === '1-5') return filters.minRefs === 1 && filters.maxRefs === 5;
    if (value === '6-20') return filters.minRefs === 6 && filters.maxRefs === 20;
    if (value === '20+') return filters.minRefs === 20 && filters.maxRefs === null;
    return false;
  }
}

// =============================================================================
// HELPERS
// =============================================================================

/**
 * Escape HTML special characters.
 * @param {string} str
 * @returns {string}
 */
function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
