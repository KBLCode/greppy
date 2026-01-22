/**
 * Greppy Web - Codebase Observatory
 *
 * Main application entry point. Coordinates state, routing, and event handling.
 * View rendering is delegated to view modules.
 *
 * @module app
 */

// =============================================================================
// IMPORTS
// =============================================================================

import { fetchStats, fetchProjects, switchProject as apiSwitchProject, fetchSettingsFromServer, saveSettingsToServer } from './api.js';
import { escapeHtml, truncatePath } from './utils.js';
import { Dropdown } from './components/dropdown.js';
import { connectSSE, updateReindexStatus } from './components/sse.js';
import { initSettings, getSettings } from './components/settings.js';
import { initExport } from './components/export.js';
import { SearchComponent, filters as searchFilters, onFilterChange } from './components/search.js';
import { renderStats } from './views/stats.js';
import { renderList, setSortState as setListSortState, getSortState as getListSortState } from './views/list.js';
import { renderGraph } from './views/graph.js';
import { renderTreeView } from './views/tree.js';
import { renderTables } from './views/tables.js';
import { renderCycles, refreshCycles } from './views/cycles.js';
import { renderTimeline } from './views/timeline.js';
import {
  loadState,
  saveState,
  debouncedSave,
  updateState,
  saveScrollPosition,
  restoreScrollPosition,
  trackRecentSymbol,
  getRecentSymbols
} from './lib/persistence.js';

// =============================================================================
// STATE
// =============================================================================

// Load persisted state
const persistedState = loadState();

const state = {
  view: persistedState.currentView || 'stats',
  stats: null,
  list: null,
  graph: null,
  tree: null,
  treemap: null,
  graphMode: persistedState.graphMode || 'treemap',
  treemapPath: persistedState.treemapPath || '',
  selected: null,
  selectedFile: null,
  filters: {
    type: persistedState.filters?.kind || 'all',
    state: persistedState.filters?.state || 'all',
    search: persistedState.filters?.search || '',
    sort: 'name',
    file: persistedState.filters?.file || '',
    minRefs: null,
    maxRefs: null,
    hasCallers: null,
    hasCallees: null,
    entry: null
  },
  indexedAt: Date.now(),
  treeCollapsed: persistedState.treePanelCollapsed || false,
  // SSE state
  isReindexing: false,
  reindexProgress: null,
  daemonConnected: false,
  // Persistence reference
  _persisted: persistedState
};

// =============================================================================
// SETTINGS
// =============================================================================

const settings = {
  streamerMode: false,
  hiddenPatterns: ['.env*', '*secret*', '*credential*', '**/config/production.*'],
  showDeadBadges: true,
  showCycleIndicators: true,
  maxGraphNodes: 100,
  maxListItems: 500,
  compactMode: false,
  theme: 'dark'
};

// =============================================================================
// LIVE TIMER
// =============================================================================

function startLiveTimer() {
  setInterval(updateIndexedTime, 1000);
}

function updateIndexedTime() {
  // Don't update if reindexing
  if (state.isReindexing) return;
  
  const elapsed = Math.floor((Date.now() - state.indexedAt) / 1000);
  const el = document.getElementById('status-index');
  if (!el) return;
  
  if (elapsed < 60) el.textContent = `indexed ${elapsed}s ago`;
  else if (elapsed < 3600) el.textContent = `indexed ${Math.floor(elapsed / 60)}m ago`;
  else el.textContent = `indexed ${Math.floor(elapsed / 3600)}h ago`;
}

// =============================================================================
// VIEW SWITCHING
// =============================================================================

function switchView(view) {
  // Save scroll position of current view before switching
  if (state.view) {
    saveScrollPosition(state.view, window.scrollY);
  }
  
  const oldView = state.view;
  state.view = view;
  
  // Persist current view
  updateState('currentView', view);
  
  // Update toolbar button states
  document.querySelectorAll('[data-view]').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.view === view);
  });
  
  // Apply exit transition to old view
  document.querySelectorAll('.view').forEach(v => {
    const isActive = v.id === `view-${view}`;
    if (v.id === `view-${oldView}` && oldView !== view) {
      v.classList.add('view-exit');
    }
    v.classList.toggle('active', isActive);
    if (isActive) {
      v.classList.add('view-enter');
      // Remove transition class after animation
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          v.classList.remove('view-enter');
          v.classList.add('view-enter-active');
          setTimeout(() => {
            v.classList.remove('view-enter-active');
          }, 200);
        });
      });
    }
  });
  
  // Remove exit class from old views
  setTimeout(() => {
    document.querySelectorAll('.view-exit').forEach(v => {
      v.classList.remove('view-exit');
    });
  }, 150);
  
  // Render the new view
  if (view === 'stats') { renderStats(state); }
  else if (view === 'graph') { renderGraph(state); }
  else if (view === 'list') { renderList(state); }
  else if (view === 'tree') { renderTreeView(state); }
  else if (view === 'tables') { renderTables(state); }
  else if (view === 'cycles') { renderCycles(state); }
  else if (view === 'timeline') { renderTimeline(state); }
  
  // Restore scroll position for the new view (after render)
  requestAnimationFrame(() => {
    restoreScrollPosition(view);
  });
}

// =============================================================================
// STATUS BAR
// =============================================================================

function updateStatusBar() {
  if (!state.stats) return;
  
  document.getElementById('stat-files').textContent = state.stats.files.toLocaleString();
  document.getElementById('stat-symbols').textContent = state.stats.symbols.toLocaleString();
  document.getElementById('stat-dead').textContent = state.stats.dead.toLocaleString();
  document.getElementById('stat-cycles').textContent = state.stats.cycles.toLocaleString();
  
  state.indexedAt = Date.now();
  updateIndexedTime();
}

// =============================================================================
// DATA REFRESH
// =============================================================================

async function refreshAllData() {
  try {
    // Reload stats
    state.stats = await fetchStats();
    updateStatusBar();
    
    // Refresh current view
    if (state.view === 'stats') renderStats(state);
    else if (state.view === 'graph') renderGraph(state);
    else if (state.view === 'list') renderList(state);
    else if (state.view === 'tree') renderTreeView(state);
    else if (state.view === 'tables') renderTables(state);
    else if (state.view === 'cycles') { refreshCycles(); renderCycles(state); }
    else if (state.view === 'timeline') renderTimeline(state);
    
  } catch (err) {
    console.error('Error refreshing data:', err);
  }
}

function refreshCurrentView() {
  if (state.view === 'graph') renderGraph(state);
  else if (state.view === 'list') renderList(state);
}

// =============================================================================
// EVENT HANDLERS
// =============================================================================

function setupEventHandlers() {
  document.querySelectorAll('[data-view]').forEach(btn => {
    btn.addEventListener('click', () => switchView(btn.dataset.view));
  });
  
  document.getElementById('detail-close').addEventListener('click', () => {
    document.getElementById('detail').classList.add('collapsed');
    state.selected = null;
    updateState('detailPanelOpen', false);
    updateState('selectedSymbolId', null);
  });
  
  document.addEventListener('keydown', (e) => {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    switch (e.key) {
      case 'g': switchView('graph'); break;
      case 'l': switchView('list'); break;
      case 'd': switchView('tables'); break;
      case 't': switchView('tree'); break;
      case 's': switchView('stats'); break;
      case 'c': switchView('cycles'); break;
      case 'h': switchView('timeline'); break;
      case '/': e.preventDefault(); document.getElementById('search-input')?.focus(); break;
      case 'Escape': 
        document.getElementById('detail').classList.add('collapsed'); 
        document.getElementById('search-input')?.blur(); 
        break;
    }
  });
  
  window.addEventListener('resize', () => {
    if (state.view === 'graph') renderGraph(state);
    if (state.view === 'stats') renderStats(state);
  });
  
  // Track scroll position (debounced)
  let scrollTimeout = null;
  window.addEventListener('scroll', () => {
    if (scrollTimeout) clearTimeout(scrollTimeout);
    scrollTimeout = setTimeout(() => {
      if (state.view) {
        saveScrollPosition(state.view, window.scrollY);
      }
    }, 150);
  }, { passive: true });
}

function setupSearchComponent() {
  /**
   * Sync search component filters with app state.
   * @param {Object} newFilters - New filter values from search component
   */
  const syncFilters = (newFilters) => {
    state.filters.type = newFilters.kind !== 'all' ? newFilters.kind : 'all';
    state.filters.state = newFilters.state !== 'all' ? newFilters.state : 'all';
    state.filters.search = newFilters.search || '';
    state.filters.file = newFilters.file || '';
    state.filters.minRefs = newFilters.minRefs;
    state.filters.maxRefs = newFilters.maxRefs;
    state.filters.hasCallers = newFilters.hasCallers;
    state.filters.hasCallees = newFilters.hasCallees;
    state.filters.entry = newFilters.entry;
  };
  
  // Initialize the advanced search component
  new SearchComponent({
    searchContainer: document.getElementById('search-bar-container'),
    quickFilterContainer: document.getElementById('quick-filters-container'),
    activeFiltersContainer: document.getElementById('active-filters-container'),
    onFilterChange: (newFilters) => {
      syncFilters(newFilters);
      refreshCurrentView();
    }
  });
}

// =============================================================================
// PROJECT SELECTOR
// =============================================================================

async function switchProject(path) {
  try {
    const data = await apiSwitchProject(path);
    
    if (data.success) {
      // Reload the page to load new project data
      window.location.reload();
    } else {
      alert(data.message || 'Failed to switch project');
    }
  } catch (err) {
    console.error('Failed to switch project:', err);
    alert('Failed to switch project');
  }
}

async function setupProjectSelector() {
  const container = document.getElementById('project-selector-container');
  if (!container) return;
  
  try {
    const { projects } = await fetchProjects();
    
    if (!projects || projects.length === 0) {
      container.innerHTML = `<span class="project-name">${state.stats?.project || 'no projects'}</span>`;
      return;
    }
    
    // Create custom dropdown for project selector
    const activeProject = projects.find(p => p.active);
    const options = projects.map(p => ({
      value: p.path,
      label: p.name,
      path: p.path,
      indexed: p.indexed,
      active: p.active
    }));
    
    // Custom render for project dropdown
    container.innerHTML = `
      <div class="dropdown project-dropdown" data-value="${activeProject?.path || ''}">
        <button class="dropdown-trigger" type="button">
          <span class="dropdown-value">${activeProject?.name || 'Select project'}</span>
          <span class="dropdown-arrow">&#9660;</span>
        </button>
        <div class="dropdown-menu">
          ${options.map(opt => `
            <div class="dropdown-item ${opt.active ? 'selected' : ''} ${!opt.indexed ? 'not-indexed' : ''}" 
                 data-value="${opt.value}" 
                 tabindex="0"
                 ${!opt.indexed ? 'title="Run greppy index first"' : ''}>
              <span class="dropdown-check">${opt.active ? '●' : '○'}</span>
              <div class="dropdown-item-content">
                <span class="dropdown-item-name">${escapeHtml(opt.label)}</span>
                <span class="dropdown-item-path">${truncatePath(opt.path)}</span>
              </div>
            </div>
          `).join('')}
        </div>
      </div>
    `;
    
    const dropdown = container.querySelector('.dropdown');
    const trigger = container.querySelector('.dropdown-trigger');
    const menu = container.querySelector('.dropdown-menu');
    
    trigger.addEventListener('click', (e) => {
      e.stopPropagation();
      dropdown.classList.toggle('open');
    });
    
    menu.addEventListener('click', (e) => {
      const item = e.target.closest('.dropdown-item');
      if (item && !item.classList.contains('not-indexed')) {
        const path = item.dataset.value;
        if (path !== activeProject?.path) {
          switchProject(path);
        }
        dropdown.classList.remove('open');
      }
    });
    
    document.addEventListener('click', () => {
      dropdown.classList.remove('open');
    });
    
  } catch (err) {
    console.error('Failed to load projects:', err);
    container.innerHTML = `<span class="project-name">${state.stats?.project || 'error'}</span>`;
  }
}

// =============================================================================
// SETTINGS
// =============================================================================

async function fetchSettings() {
  try {
    const data = await fetchSettingsFromServer();
    Object.assign(settings, data);
    applySettings();
  } catch (err) {
    console.error('Failed to load settings:', err);
  }
}

async function saveSettings() {
  try {
    const data = await saveSettingsToServer(settings);
    Object.assign(settings, data);
    applySettings();
    
    // Also save to localStorage for immediate access
    localStorage.setItem('greppy-settings', JSON.stringify(settings));
    
    closeSettingsModal();
  } catch (err) {
    console.error('Failed to save settings:', err);
    alert('Failed to save settings');
  }
}

function applySettings() {
  // Apply streamer mode to body
  document.body.classList.toggle('streamer-mode', settings.streamerMode);
  
  // Apply compact mode
  document.body.classList.toggle('compact-mode', settings.compactMode);
  
  // Refresh views if needed
  if (state.view === 'list') renderList(state);
  if (state.view === 'graph') renderGraph(state);
}

function openSettingsModal() {
  const modal = document.getElementById('settings-modal');
  if (!modal) return;
  
  // Populate form with current settings
  document.getElementById('setting-streamer-mode').checked = settings.streamerMode;
  document.getElementById('setting-hidden-patterns').value = settings.hiddenPatterns.join('\n');
  document.getElementById('setting-dead-badges').checked = settings.showDeadBadges;
  document.getElementById('setting-cycle-indicators').checked = settings.showCycleIndicators;
  document.getElementById('setting-compact-mode').checked = settings.compactMode;
  document.getElementById('setting-max-graph-nodes').value = settings.maxGraphNodes;
  document.getElementById('setting-max-list-items').value = settings.maxListItems;
  
  modal.classList.remove('hidden');
}

function closeSettingsModal() {
  const modal = document.getElementById('settings-modal');
  if (modal) {
    modal.classList.add('hidden');
  }
}

function collectSettingsFromForm() {
  settings.streamerMode = document.getElementById('setting-streamer-mode').checked;
  settings.hiddenPatterns = document.getElementById('setting-hidden-patterns').value
    .split('\n')
    .map(s => s.trim())
    .filter(s => s.length > 0);
  settings.showDeadBadges = document.getElementById('setting-dead-badges').checked;
  settings.showCycleIndicators = document.getElementById('setting-cycle-indicators').checked;
  settings.compactMode = document.getElementById('setting-compact-mode').checked;
  settings.maxGraphNodes = parseInt(document.getElementById('setting-max-graph-nodes').value, 10) || 100;
  settings.maxListItems = parseInt(document.getElementById('setting-max-list-items').value, 10) || 500;
}

function setupSettingsHandlers() {
  // Settings button
  const settingsBtn = document.getElementById('settings-btn');
  if (settingsBtn) {
    settingsBtn.addEventListener('click', openSettingsModal);
  }
  
  // Close button
  const closeBtn = document.getElementById('settings-close');
  if (closeBtn) {
    closeBtn.addEventListener('click', closeSettingsModal);
  }
  
  // Backdrop click
  const backdrop = document.querySelector('#settings-modal .modal-backdrop');
  if (backdrop) {
    backdrop.addEventListener('click', closeSettingsModal);
  }
  
  // Save button
  const saveBtn = document.getElementById('settings-save');
  if (saveBtn) {
    saveBtn.addEventListener('click', () => {
      collectSettingsFromForm();
      saveSettings();
    });
  }
  
  // Keyboard shortcut
  document.addEventListener('keydown', (e) => {
    if (e.key === ',' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      openSettingsModal();
    }
    if (e.key === 'Escape') {
      closeSettingsModal();
    }
  });
}

// =============================================================================
// INIT
// =============================================================================

async function init() {
  setupEventHandlers();
  setupSearchComponent();
  setupSettingsHandlers();
  startLiveTimer();
  
  // Initialize enhanced settings component (handles theme, density, font size)
  initSettings((newSettings) => {
    // Sync legacy settings object with new settings
    Object.assign(settings, newSettings);
    applySettings();
  });
  
  // Initialize export dropdown
  initExport();
  
  // Connect to SSE for live updates
  connectSSE(state, refreshAllData);
  
  // Restore sort state from persistence
  if (persistedState.sortState?.list) {
    setListSortState(persistedState.sortState.list);
  }
  
  try {
    // Load settings first
    await fetchSettings();
    
    // Then load stats and setup project selector
    state.stats = await fetchStats();
    updateStatusBar();
    await setupProjectSelector();
    
    // Switch to persisted view (or default to stats)
    const targetView = persistedState.currentView || 'stats';
    switchView(targetView);
  } catch (err) {
    console.error('Failed to initialize:', err);
  }
}

// =============================================================================
// GLOBAL FUNCTIONS FOR ERROR STATE RETRIES
// =============================================================================

/**
 * Clear all filters and refresh.
 */
window.clearFilters = function() {
  state.filters = {
    type: 'all',
    state: 'all',
    search: '',
    sort: 'name',
    file: '',
    minRefs: null,
    maxRefs: null,
    hasCallers: null,
    hasCallees: null,
    entry: null
  };
  refreshCurrentView();
};

/**
 * Refresh stats view.
 */
window.refreshStats = function() {
  renderStats(state);
};

/**
 * Refresh list view.
 */
window.refreshList = function() {
  renderList(state);
};

/**
 * Refresh graph view.
 */
window.refreshGraph = function() {
  renderGraph(state);
};

/**
 * Refresh cycles view.
 */
window.refreshCycles = function() {
  refreshCycles();
  renderCycles(state);
};

/**
 * Refresh tables view.
 */
window.refreshTables = function() {
  renderTables(state);
};

/**
 * Refresh timeline view (when implemented).
 */
window.refreshTimeline = function() {
  // Will be implemented with timeline feature
  location.reload();
};

/**
 * Refresh detail panel.
 */
window.refreshDetail = function() {
  if (state.selected) {
    // Re-select the current symbol to refresh
    const selectSymbol = window.selectSymbol;
    if (selectSymbol) {
      selectSymbol(state.selected, state);
    }
  }
};

init();
