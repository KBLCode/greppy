/**
 * Detail Panel Component
 *
 * Handles symbol selection and comprehensive detail panel rendering.
 * Shows callers, callees, references, paths, cycles, and impact analysis.
 *
 * @module components/detail
 */

import { escapeHtml, truncatePath } from '../utils.js';
import { trackRecentSymbol, getRecentSymbols, updateState } from '../lib/persistence.js';

// =============================================================================
// CONSTANTS
// =============================================================================

const LOADING_HTML = '<span class="text-dim">loading...</span>';
const ERROR_HTML = '<span class="text-dim">failed to load</span>';
const EMPTY_HTML = '<span class="text-dim">none</span>';
const MAX_HISTORY = 10;

// =============================================================================
// SYMBOL HISTORY (SESSION + PERSISTED)
// =============================================================================

// Session history for breadcrumb navigation (not persisted)
const sessionHistory = [];

/**
 * Add a symbol to the session history and persist to recent symbols.
 * @param {Object} item - Symbol to add
 */
function addToHistory(item) {
  if (!item || !item.name) return;
  
  // Session history for breadcrumb
  const idx = sessionHistory.findIndex(s => s.id === item.id || s.name === item.name);
  if (idx !== -1) {
    sessionHistory.splice(idx, 1);
  }
  sessionHistory.unshift(item);
  if (sessionHistory.length > MAX_HISTORY) {
    sessionHistory.pop();
  }
  
  // Persist to localStorage
  trackRecentSymbol({
    id: item.id,
    name: item.name,
    path: item.path || item.file || '',
    type: item.type || item.kind || ''
  });
  
  // Also persist selected symbol ID
  updateState('selectedSymbolId', item.id);
}

/**
 * Get recent symbol history (session).
 * @returns {Array} Recent symbols from current session
 */
export function getSymbolHistory() {
  return sessionHistory.slice();
}

/**
 * Get persisted recent symbols (across sessions).
 * @returns {Array} Recent symbols from persistence
 */
export function getPersistedRecentSymbols() {
  return getRecentSymbols();
}

// =============================================================================
// API FUNCTIONS
// =============================================================================

/**
 * Fetch symbol details by ID.
 * @param {string|number} symbolId - Symbol ID
 * @returns {Promise<Object|null>}
 */
async function fetchSymbolDetails(symbolId) {
  try {
    const res = await fetch(`/api/symbol/${encodeURIComponent(symbolId)}`);
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

/**
 * Fetch callers for a symbol.
 * @param {string|number} symbolId - Symbol ID
 * @returns {Promise<Array>}
 */
async function fetchSymbolCallers(symbolId) {
  try {
    const res = await fetch(`/api/symbol/${encodeURIComponent(symbolId)}/callers`);
    if (!res.ok) return [];
    const data = await res.json();
    return data.callers || [];
  } catch {
    return [];
  }
}

/**
 * Fetch callees for a symbol.
 * @param {string|number} symbolId - Symbol ID
 * @returns {Promise<Array>}
 */
async function fetchSymbolCallees(symbolId) {
  try {
    const res = await fetch(`/api/symbol/${encodeURIComponent(symbolId)}/callees`);
    if (!res.ok) return [];
    const data = await res.json();
    return data.callees || [];
  } catch {
    return [];
  }
}

/**
 * Fetch references for a symbol.
 * @param {string|number} symbolId - Symbol ID
 * @returns {Promise<Array>}
 */
async function fetchSymbolRefs(symbolId) {
  try {
    const res = await fetch(`/api/symbol/${encodeURIComponent(symbolId)}/refs`);
    if (!res.ok) return [];
    const data = await res.json();
    return data.refs || data.references || [];
  } catch {
    return [];
  }
}

/**
 * Fetch impact analysis for a symbol.
 * @param {string|number} symbolId - Symbol ID
 * @returns {Promise<Object|null>}
 */
async function fetchSymbolImpact(symbolId) {
  try {
    const res = await fetch(`/api/symbol/${encodeURIComponent(symbolId)}/impact`);
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

/**
 * Fetch cycles data.
 * @returns {Promise<Array>}
 */
async function fetchCycles() {
  try {
    const res = await fetch('/api/cycles');
    if (!res.ok) return [];
    const data = await res.json();
    return data.cycles || [];
  } catch {
    return [];
  }
}

// =============================================================================
// SECTION STATE MANAGEMENT
// =============================================================================

const sectionStates = {
  numbers: true,
  callers: true,
  callees: true,
  refs: false,
  paths: false,
  cycles: false
};

/**
 * Toggle a collapsible section.
 * @param {string} sectionId - Section identifier
 */
function toggleSection(sectionId) {
  sectionStates[sectionId] = !sectionStates[sectionId];
  const section = document.querySelector(`[data-section="${sectionId}"]`);
  if (section) {
    section.classList.toggle('collapsed', !sectionStates[sectionId]);
    const toggle = section.querySelector('.detail-section-toggle');
    if (toggle) {
      toggle.textContent = sectionStates[sectionId] ? '[-]' : '[+]';
    }
  }
}

// =============================================================================
// NAVIGATION HELPERS
// =============================================================================

let currentState = null;

/**
 * Navigate to a symbol from caller/callee/ref click.
 * @param {Object} symbol - Symbol data
 */
function navigateToSymbol(symbol) {
  if (!symbol || !currentState) return;
  selectSymbol(symbol, currentState);
}

// Register global handler for onclick in HTML
window.__navigateToSymbol = navigateToSymbol;
window.__toggleDetailSection = toggleSection;

// =============================================================================
// SYMBOL SELECTION (ENHANCED)
// =============================================================================

/**
 * Select a symbol and display comprehensive details.
 * @param {Object} item - Symbol item
 * @param {Object} state - App state (to update state.selected)
 */
export function selectSymbol(item, state) {
  state.selected = item;
  currentState = state;
  
  // Add to history (also persists)
  addToHistory(item);
  
  // Persist detail panel open state
  updateState('detailPanelOpen', true);
  
  const detail = document.getElementById('detail');
  const title = document.getElementById('detail-title');
  const content = document.getElementById('detail-content');
  
  detail.classList.remove('collapsed');
  title.textContent = item.name;
  
  // Calculate line count if available
  const lineCount = item.end_line && item.start_line 
    ? item.end_line - item.start_line + 1 
    : null;
  const lineInfo = lineCount ? ` (${lineCount} lines)` : '';
  
  // Determine if symbol is entry point or in cycle
  const isEntryPoint = item.is_entry_point || item.entry_point || false;
  const inCycle = item.in_cycle || item.cycle || false;
  
  content.innerHTML = `
    ${renderSymbolHistory()}
    
    <div class="detail-meta">
      <span class="detail-meta-item">
        <span class="detail-meta-label">kind:</span>
        <span class="detail-meta-value">${escapeHtml(item.type || item.kind || 'unknown')}</span>
      </span>
      <span class="detail-meta-sep">|</span>
      <span class="detail-meta-item">
        <span class="detail-meta-label">file:</span>
        <span class="detail-meta-value mono clickable" onclick="window.__navigateToFile('${escapeHtml(item.path || '')}')">${escapeHtml(item.path || '')}:${item.line || item.start_line || '?'}${lineInfo}</span>
      </span>
      ${isEntryPoint ? '<span class="detail-meta-sep">|</span><span class="detail-meta-badge entry">ENTRY</span>' : ''}
      ${inCycle ? '<span class="detail-meta-sep">|</span><span class="detail-meta-badge cycle">IN CYCLE</span>' : ''}
    </div>
    
    <div class="detail-numbers" data-section="numbers">
      <div class="detail-number-grid" id="detail-numbers-grid">
        ${renderNumbersPlaceholder()}
      </div>
    </div>
    
    <div class="detail-sections-grid">
      <div class="detail-section" data-section="callers">
        <div class="detail-section-header" onclick="window.__toggleDetailSection('callers')">
          <span class="detail-section-title">CALLERS <span class="detail-section-count" id="callers-count">(${item.callers || '?'})</span></span>
          <span class="detail-section-toggle">[-]</span>
        </div>
        <div class="detail-section-content" id="detail-callers">${LOADING_HTML}</div>
      </div>
      
      <div class="detail-section" data-section="callees">
        <div class="detail-section-header" onclick="window.__toggleDetailSection('callees')">
          <span class="detail-section-title">CALLEES <span class="detail-section-count" id="callees-count">(${item.callees || '?'})</span></span>
          <span class="detail-section-toggle">[-]</span>
        </div>
        <div class="detail-section-content" id="detail-callees">${LOADING_HTML}</div>
      </div>
      
      <div class="detail-section ${sectionStates.refs ? '' : 'collapsed'}" data-section="refs">
        <div class="detail-section-header" onclick="window.__toggleDetailSection('refs')">
          <span class="detail-section-title">REFERENCES <span class="detail-section-count" id="refs-count">(${item.refs || '?'})</span></span>
          <span class="detail-section-toggle">${sectionStates.refs ? '[-]' : '[+]'}</span>
        </div>
        <div class="detail-section-content" id="detail-refs">${LOADING_HTML}</div>
      </div>
      
      <div class="detail-section ${sectionStates.paths ? '' : 'collapsed'}" data-section="paths">
        <div class="detail-section-header" onclick="window.__toggleDetailSection('paths')">
          <span class="detail-section-title">PATHS FROM ENTRY</span>
          <span class="detail-section-toggle">${sectionStates.paths ? '[-]' : '[+]'}</span>
        </div>
        <div class="detail-section-content" id="detail-paths">${LOADING_HTML}</div>
      </div>
      
      ${inCycle ? `
      <div class="detail-section ${sectionStates.cycles ? '' : 'collapsed'}" data-section="cycles">
        <div class="detail-section-header" onclick="window.__toggleDetailSection('cycles')">
          <span class="detail-section-title">CYCLES</span>
          <span class="detail-section-toggle">${sectionStates.cycles ? '[-]' : '[+]'}</span>
        </div>
        <div class="detail-section-content" id="detail-cycles">${LOADING_HTML}</div>
      </div>
      ` : ''}
    </div>
  `;
  
  // Fetch additional data asynchronously
  loadDetailData(item, state);
}

/**
 * Render symbol history breadcrumb.
 * @returns {string} HTML
 */
function renderSymbolHistory() {
  if (sessionHistory.length <= 1) return '';
  
  const items = sessionHistory.slice(0, 5).map((s, i) => {
    const isCurrent = i === 0;
    return `<span class="history-item ${isCurrent ? 'current' : ''}" 
                  onclick="window.__navigateToSymbol(${JSON.stringify(s).replace(/"/g, '&quot;')})"
                  title="${escapeHtml(s.path || '')}">${escapeHtml(s.name)}</span>`;
  });
  
  return `
    <div class="detail-history">
      <span class="history-label">recent:</span>
      ${items.join('<span class="history-sep">→</span>')}
    </div>
  `;
}

/**
 * Render placeholder numbers grid.
 * @returns {string} HTML
 */
function renderNumbersPlaceholder() {
  return `
    <div class="detail-number-box">
      <span class="detail-number-value">-</span>
      <span class="detail-number-label">refs</span>
    </div>
    <div class="detail-number-box">
      <span class="detail-number-value">-</span>
      <span class="detail-number-label">callers</span>
    </div>
    <div class="detail-number-box">
      <span class="detail-number-value">-</span>
      <span class="detail-number-label">callees</span>
    </div>
    <div class="detail-number-box">
      <span class="detail-number-value">-</span>
      <span class="detail-number-label">files</span>
    </div>
  `;
}

/**
 * Render numbers grid with actual data.
 * @param {Object} data - Numbers data
 * @returns {string} HTML
 */
function renderNumbersGrid(data) {
  const refs = data.refs ?? '-';
  const callers = data.callers ?? '-';
  const callees = data.callees ?? '-';
  const files = data.files ?? '-';
  
  return `
    <div class="detail-number-box">
      <span class="detail-number-value">${refs}</span>
      <span class="detail-number-label">refs</span>
    </div>
    <div class="detail-number-box">
      <span class="detail-number-value">${callers}</span>
      <span class="detail-number-label">callers</span>
    </div>
    <div class="detail-number-box">
      <span class="detail-number-value">${callees}</span>
      <span class="detail-number-label">callees</span>
    </div>
    <div class="detail-number-box">
      <span class="detail-number-value">${files}</span>
      <span class="detail-number-label">files</span>
    </div>
  `;
}

/**
 * Load callers, callees, refs, and impact data asynchronously.
 * @param {Object} item - Symbol item
 * @param {Object} state - App state
 */
async function loadDetailData(item, state) {
  const symbolId = item.id || item.name;
  
  // Fetch all data in parallel
  const [callers, callees, refs, impact, cycles] = await Promise.all([
    fetchSymbolCallers(symbolId),
    fetchSymbolCallees(symbolId),
    fetchSymbolRefs(symbolId),
    fetchSymbolImpact(symbolId),
    item.in_cycle ? fetchCycles() : Promise.resolve([])
  ]);
  
  // Update numbers grid
  const numbersEl = document.getElementById('detail-numbers-grid');
  if (numbersEl) {
    // Count unique files from refs
    const uniqueFiles = new Set();
    refs.forEach(r => {
      if (r.file || r.path) uniqueFiles.add(r.file || r.path);
    });
    
    numbersEl.innerHTML = renderNumbersGrid({
      refs: refs.length || item.refs || 0,
      callers: callers.length || item.callers || 0,
      callees: callees.length || item.callees || 0,
      files: uniqueFiles.size || impact?.blast_radius?.files || 0
    });
  }
  
  // Update counts in section headers
  document.getElementById('callers-count')?.replaceWith(
    Object.assign(document.createElement('span'), {
      className: 'detail-section-count',
      id: 'callers-count',
      textContent: `(${callers.length})`
    })
  );
  document.getElementById('callees-count')?.replaceWith(
    Object.assign(document.createElement('span'), {
      className: 'detail-section-count',
      id: 'callees-count',
      textContent: `(${callees.length})`
    })
  );
  document.getElementById('refs-count')?.replaceWith(
    Object.assign(document.createElement('span'), {
      className: 'detail-section-count',
      id: 'refs-count',
      textContent: `(${refs.length})`
    })
  );
  
  // Render callers
  const callersEl = document.getElementById('detail-callers');
  if (callersEl) {
    if (callers.length > 0) {
      callersEl.innerHTML = renderCallersList(callers, state);
    } else if (item.callers > 0) {
      callersEl.innerHTML = `<span class="text-dim">${item.callers} callers (details pending)</span>`;
    } else {
      callersEl.innerHTML = EMPTY_HTML;
    }
  }
  
  // Render callees
  const calleesEl = document.getElementById('detail-callees');
  if (calleesEl) {
    if (callees.length > 0) {
      calleesEl.innerHTML = renderCalleesList(callees, state);
    } else if (item.callees > 0) {
      calleesEl.innerHTML = `<span class="text-dim">${item.callees} callees (details pending)</span>`;
    } else {
      calleesEl.innerHTML = EMPTY_HTML;
    }
  }
  
  // Render references
  const refsEl = document.getElementById('detail-refs');
  if (refsEl) {
    if (refs.length > 0) {
      refsEl.innerHTML = renderRefsList(refs);
    } else {
      refsEl.innerHTML = EMPTY_HTML;
    }
  }
  
  // Render paths from entry
  const pathsEl = document.getElementById('detail-paths');
  if (pathsEl) {
    if (impact && impact.paths_to_entry && impact.paths_to_entry.length > 0) {
      pathsEl.innerHTML = renderPathsToEntry(impact.paths_to_entry);
    } else {
      pathsEl.innerHTML = '<span class="text-dim">no paths to entry points</span>';
    }
  }
  
  // Render cycles if applicable
  const cyclesEl = document.getElementById('detail-cycles');
  if (cyclesEl && item.in_cycle) {
    const symbolCycles = cycles.filter(c => 
      c.symbols?.some(s => s.name === item.name || s.id === item.id)
    );
    if (symbolCycles.length > 0) {
      cyclesEl.innerHTML = renderCyclesInvolving(symbolCycles, item);
    } else {
      cyclesEl.innerHTML = '<span class="text-dim">cycle data unavailable</span>';
    }
  }
}

/**
 * Render callers list with depth info.
 * @param {Array} callers - List of caller symbols
 * @param {Object} state - App state
 * @returns {string} HTML
 */
function renderCallersList(callers, state) {
  if (!callers || callers.length === 0) return EMPTY_HTML;
  
  const maxShow = 15;
  const shown = callers.slice(0, maxShow);
  const remaining = callers.length - maxShow;
  
  let html = '<div class="detail-items">';
  for (const caller of shown) {
    const depth = caller.depth ? `<span class="detail-item-depth">depth: ${caller.depth}</span>` : 
                  '<span class="detail-item-depth">direct</span>';
    const via = caller.via ? `<span class="detail-item-via">via ${escapeHtml(caller.via)}</span>` : '';
    const dataAttr = JSON.stringify(caller).replace(/"/g, '&quot;');
    html += `
      <div class="detail-item caller-item" data-symbol-id="${caller.id || ''}" onclick="window.__navigateToSymbol(${dataAttr})">
        <span class="detail-item-name">${escapeHtml(caller.name)}</span>
        <span class="detail-item-path">${escapeHtml(truncatePath(caller.path || caller.file || ''))}:${caller.line || '?'}</span>
        ${depth}${via}
      </div>
    `;
  }
  
  if (remaining > 0) {
    html += `<div class="detail-item-more">+${remaining} more...</div>`;
  }
  
  html += '</div>';
  return html;
}

/**
 * Render callees list.
 * @param {Array} callees - List of callee symbols
 * @param {Object} state - App state
 * @returns {string} HTML
 */
function renderCalleesList(callees, state) {
  if (!callees || callees.length === 0) return EMPTY_HTML;
  
  const maxShow = 15;
  const shown = callees.slice(0, maxShow);
  const remaining = callees.length - maxShow;
  
  let html = '<div class="detail-items">';
  for (const callee of shown) {
    const dataAttr = JSON.stringify(callee).replace(/"/g, '&quot;');
    html += `
      <div class="detail-item callee-item" data-symbol-id="${callee.id || ''}" onclick="window.__navigateToSymbol(${dataAttr})">
        <span class="detail-item-name">${escapeHtml(callee.name)}</span>
        <span class="detail-item-path">${escapeHtml(truncatePath(callee.path || callee.file || ''))}:${callee.line || '?'}</span>
      </div>
    `;
  }
  
  if (remaining > 0) {
    html += `<div class="detail-item-more">+${remaining} more...</div>`;
  }
  
  html += '</div>';
  return html;
}

/**
 * Render references list with context.
 * @param {Array} refs - List of references
 * @returns {string} HTML
 */
function renderRefsList(refs) {
  if (!refs || refs.length === 0) return EMPTY_HTML;
  
  const maxShow = 20;
  const shown = refs.slice(0, maxShow);
  const remaining = refs.length - maxShow;
  
  // Group by kind if available
  const byKind = {};
  for (const ref of shown) {
    const kind = ref.kind || ref.type || 'usage';
    if (!byKind[kind]) byKind[kind] = [];
    byKind[kind].push(ref);
  }
  
  let html = '<div class="detail-refs-list">';
  
  for (const [kind, items] of Object.entries(byKind)) {
    html += `<div class="detail-refs-group">`;
    html += `<div class="detail-refs-kind">${escapeHtml(kind.toUpperCase())}</div>`;
    
    for (const ref of items) {
      const context = ref.context || ref.snippet || '';
      html += `
        <div class="detail-ref-item">
          <span class="detail-ref-loc">${escapeHtml(truncatePath(ref.file || ref.path || ''))}:${ref.line || '?'}</span>
          ${context ? `<code class="detail-ref-context">${escapeHtml(context.trim().substring(0, 60))}</code>` : ''}
        </div>
      `;
    }
    html += '</div>';
  }
  
  if (remaining > 0) {
    html += `<div class="detail-item-more">+${remaining} more references...</div>`;
  }
  
  html += '</div>';
  return html;
}

/**
 * Render paths to entry points visualization.
 * @param {Array} paths - Paths to entry points
 * @returns {string} HTML
 */
function renderPathsToEntry(paths) {
  if (!paths || paths.length === 0) return '<span class="text-dim">no paths</span>';
  
  const maxPaths = 5;
  const shown = paths.slice(0, maxPaths);
  const remaining = paths.length - maxPaths;
  
  let html = '<div class="detail-paths-list">';
  
  for (const path of shown) {
    const symbols = path.symbols || path;
    if (!Array.isArray(symbols)) continue;
    
    html += '<div class="detail-path-row">';
    html += '<div class="detail-path-visual">';
    
    for (let i = 0; i < symbols.length; i++) {
      const sym = symbols[i];
      const isLast = i === symbols.length - 1;
      const dataAttr = JSON.stringify(sym).replace(/"/g, '&quot;');
      
      html += `<span class="path-node ${isLast ? 'current' : ''}" onclick="window.__navigateToSymbol(${dataAttr})" title="${escapeHtml(sym.path || '')}">${escapeHtml(sym.name)}</span>`;
      
      if (!isLast) {
        html += '<span class="path-arrow">→</span>';
      }
    }
    
    html += '</div>';
    html += '</div>';
  }
  
  if (remaining > 0) {
    html += `<div class="detail-item-more">+${remaining} more paths...</div>`;
  }
  
  html += '</div>';
  return html;
}

/**
 * Render cycles involving this symbol.
 * @param {Array} cycles - Cycles containing the symbol
 * @param {Object} item - Current symbol
 * @returns {string} HTML
 */
function renderCyclesInvolving(cycles, item) {
  if (!cycles || cycles.length === 0) return '<span class="text-dim">no cycles</span>';
  
  let html = '<div class="detail-cycles-list">';
  
  for (const cycle of cycles.slice(0, 3)) {
    const symbols = cycle.symbols || [];
    
    html += '<div class="detail-cycle-item">';
    html += '<div class="detail-cycle-path">';
    
    for (let i = 0; i < symbols.length; i++) {
      const sym = symbols[i];
      const isCurrentSymbol = sym.name === item.name || sym.id === item.id;
      const dataAttr = JSON.stringify(sym).replace(/"/g, '&quot;');
      
      html += `<span class="cycle-node ${isCurrentSymbol ? 'highlight' : ''}" onclick="window.__navigateToSymbol(${dataAttr})">${escapeHtml(sym.name)}</span>`;
      
      html += '<span class="cycle-arrow">→</span>';
    }
    
    // Close the cycle back to first
    if (symbols.length > 0) {
      const first = symbols[0];
      html += `<span class="cycle-node" onclick="window.__navigateToSymbol(${JSON.stringify(first).replace(/"/g, '&quot;')})">${escapeHtml(first.name)}</span>`;
    }
    
    html += '</div>';
    html += '</div>';
  }
  
  html += '</div>';
  return html;
}

// =============================================================================
// TREEMAP NODE SELECTION
// =============================================================================

/**
 * Select a treemap node and display its details.
 * @param {Object} data - Treemap node data
 * @param {Object} state - App state
 */
export function selectTreemapNode(data, state) {
  state.selected = data;
  const detail = document.getElementById('detail');
  const title = document.getElementById('detail-title');
  const content = document.getElementById('detail-content');
  
  detail.classList.remove('collapsed');
  title.textContent = data.name;
  
  const hColor = data.health >= 70 ? '#00d4d4' : data.health >= 40 ? '#ffcc00' : '#ff3333';
  content.innerHTML = `
    <div class="detail-cols">
      <div class="detail-col">
        <div class="detail-field"><span class="detail-label">type</span><span class="detail-value">${data.type}</span></div>
        <div class="detail-field"><span class="detail-label">path</span><span class="detail-value mono">${escapeHtml(data.path)}</span></div>
      </div>
      <div class="detail-col">
        <div class="detail-field"><span class="detail-label">symbols</span><span class="detail-value">${data.value || 0}</span></div>
        <div class="detail-field"><span class="detail-label">dead</span><span class="detail-value ${data.dead ? 'dead' : ''}">${data.dead || 0}</span></div>
        <div class="detail-field"><span class="detail-label">health</span><span class="detail-value" style="color:${hColor}">${data.health}%</span></div>
        ${data.file_count ? `<div class="detail-field"><span class="detail-label">files</span><span class="detail-value">${data.file_count}</span></div>` : ''}
      </div>
    </div>
  `;
}

// =============================================================================
// GRAPH NODE SELECTION
// =============================================================================

/**
 * Select a graph node and display its details.
 * Fetches file symbols and shows aggregated stats.
 * @param {Object} node - Graph node
 * @param {Object} state - App state
 * @param {Object} d3 - D3 library reference
 */
export async function selectGraphNode(node, state, d3) {
  state.selected = node;
  currentState = state;
  d3.selectAll('.node').classed('selected', d => d.id === node.id);
  
  const detail = document.getElementById('detail');
  const title = document.getElementById('detail-title');
  const content = document.getElementById('detail-content');
  
  detail.classList.remove('collapsed');
  title.textContent = node.name || node.id;
  
  // Show initial stats with loading state for symbols
  content.innerHTML = `
    <div class="detail-numbers">
      <div class="detail-number-grid">
        <div class="detail-number-box"><span class="detail-number-value">${node.symbols || 0}</span><span class="detail-number-label">symbols</span></div>
        <div class="detail-number-box"><span class="detail-number-value ${node.dead > 0 ? 'dead' : ''}">${node.dead || 0}</span><span class="detail-number-label">dead</span></div>
        <div class="detail-number-box"><span class="detail-number-value">${node.imports || 0}</span><span class="detail-number-label">imports</span></div>
        <div class="detail-number-box"><span class="detail-number-value">${node.exports || 0}</span><span class="detail-number-label">exports</span></div>
      </div>
    </div>
    <div class="detail-numbers" id="file-aggregate-stats" style="display: none;">
      <div class="detail-number-grid">
        <div class="detail-number-box"><span class="detail-number-value" id="file-total-refs">-</span><span class="detail-number-label">total refs</span></div>
        <div class="detail-number-box"><span class="detail-number-value" id="file-total-callers">-</span><span class="detail-number-label">total callers</span></div>
        <div class="detail-number-box"><span class="detail-number-value" id="file-total-callees">-</span><span class="detail-number-label">total callees</span></div>
      </div>
    </div>
    <div class="detail-section">
      <div class="detail-section-header">Symbols in File</div>
      <div class="detail-section-content" id="file-symbols-list">
        ${LOADING_HTML}
      </div>
    </div>
  `;
  
  // Fetch file symbols
  try {
    const path = encodeURIComponent(node.id);
    const res = await fetch(`/api/file/${path}`);
    if (res.ok) {
      const data = await res.json();
      const listEl = document.getElementById('file-symbols-list');
      const statsEl = document.getElementById('file-aggregate-stats');
      
      if (data.symbols && data.symbols.length > 0) {
        // Calculate aggregated stats
        let totalRefs = 0;
        let totalCallers = 0;
        let totalCallees = 0;
        
        for (const s of data.symbols) {
          totalRefs += s.refs || 0;
          totalCallers += s.callers || 0;
          totalCallees += s.callees || 0;
        }
        
        // Update aggregate stats display
        if (statsEl) {
          statsEl.style.display = 'block';
          document.getElementById('file-total-refs').textContent = totalRefs;
          document.getElementById('file-total-callers').textContent = totalCallers;
          document.getElementById('file-total-callees').textContent = totalCallees;
        }
        
        // Render symbols list
        listEl.innerHTML = data.symbols.map(s => `
          <div class="detail-ref-item" data-symbol-id="${s.id}">
            <span class="symbol-name clickable">${escapeHtml(s.name)}</span>
            <span class="detail-ref-loc">${escapeHtml(s.type || s.kind || '')} :${s.line || s.start_line || '?'}</span>
            <span class="detail-ref-stats">${s.refs || 0} refs, ${s.callers || 0} callers</span>
          </div>
        `).join('');
        
        // Add click handlers to drill into symbols
        listEl.querySelectorAll('[data-symbol-id]').forEach(el => {
          el.addEventListener('click', () => {
            const symbolId = el.dataset.symbolId;
            const symbol = data.symbols.find(s => String(s.id) === symbolId);
            if (symbol) selectSymbol(symbol, state);
          });
        });
      } else {
        listEl.innerHTML = EMPTY_HTML;
      }
    } else {
      document.getElementById('file-symbols-list').innerHTML = ERROR_HTML;
    }
  } catch (err) {
    console.error('Failed to load file symbols:', err);
    const listEl = document.getElementById('file-symbols-list');
    if (listEl) listEl.innerHTML = ERROR_HTML;
  }
}

// =============================================================================
// CLOSE PANEL
// =============================================================================

/**
 * Close the detail panel.
 */
export function closeDetailPanel() {
  const detail = document.getElementById('detail');
  if (detail) {
    detail.classList.add('collapsed');
  }
}
