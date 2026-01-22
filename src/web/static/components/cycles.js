/**
 * Cycles Visualization Component
 *
 * Displays circular dependencies in the codebase with severity
 * ratings and expandable cycle details.
 *
 * @module components/cycles
 */

import { escapeHtml } from '../utils.js';

// =============================================================================
// CONSTANTS
// =============================================================================

const SEVERITY_THRESHOLDS = {
  critical: 6,  // 6+ symbols in cycle
  high: 4,      // 4-5 symbols
  medium: 3     // 3 symbols (minimum cycle)
};

// =============================================================================
// API FUNCTIONS
// =============================================================================

/**
 * Fetch cycles data from the API.
 * @returns {Promise<Object>} Cycles data with total count and cycle list
 */
async function fetchCycles() {
  try {
    const res = await fetch('/api/cycles');
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

// =============================================================================
// SEVERITY HELPERS
// =============================================================================

/**
 * Calculate severity based on cycle size.
 * @param {number} size - Number of symbols in cycle
 * @returns {string} Severity level
 */
function getSeverity(size) {
  if (size >= SEVERITY_THRESHOLDS.critical) return 'critical';
  if (size >= SEVERITY_THRESHOLDS.high) return 'high';
  return 'medium';
}

/**
 * Get severity label with proper casing.
 * @param {string} severity - Severity level
 * @returns {string} Display label
 */
function getSeverityLabel(severity) {
  return severity.charAt(0).toUpperCase() + severity.slice(1);
}

// =============================================================================
// CYCLE STATE
// =============================================================================

const expandedCycles = new Set();

/**
 * Toggle cycle expansion.
 * @param {number} cycleIndex - Index of the cycle to toggle
 */
function toggleCycle(cycleIndex) {
  if (expandedCycles.has(cycleIndex)) {
    expandedCycles.delete(cycleIndex);
  } else {
    expandedCycles.add(cycleIndex);
  }
  
  const card = document.querySelector(`[data-cycle-index="${cycleIndex}"]`);
  if (card) {
    card.classList.toggle('expanded', expandedCycles.has(cycleIndex));
  }
}

// =============================================================================
// RENDERING
// =============================================================================

/**
 * Render the cycles panel.
 * @param {HTMLElement} container - Container element
 * @param {Object} data - Cycles data
 * @param {Object} options - Render options
 * @param {Function} options.onSymbolClick - Callback when symbol is clicked
 */
export function renderCyclesPanel(container, data, options = {}) {
  if (!container) return;
  
  if (!data || !data.cycles || data.cycles.length === 0) {
    container.innerHTML = `
      <div class="cycles-empty">
        <span class="cycles-empty-icon">&#x2713;</span>
        <span class="cycles-empty-text">No circular dependencies detected</span>
      </div>
    `;
    return;
  }
  
  const { cycles, total_symbols } = data;
  const totalSymbols = total_symbols || cycles.reduce((sum, c) => sum + (c.symbols?.length || c.length || 0), 0);
  
  // Sort cycles by size (largest first)
  const sortedCycles = [...cycles].sort((a, b) => {
    const sizeA = a.symbols?.length || a.length || 0;
    const sizeB = b.symbols?.length || b.length || 0;
    return sizeB - sizeA;
  });
  
  const html = `
    <div class="cycles-container">
      <div class="cycles-header">
        <span class="cycles-summary">
          <span class="cycles-summary-count">${totalSymbols}</span> symbols in 
          <span class="cycles-summary-count">${cycles.length}</span> cycles
        </span>
      </div>
      <div class="cycles-list">
        ${sortedCycles.map((cycle, index) => renderCycleCard(cycle, index, options)).join('')}
      </div>
    </div>
  `;
  
  container.innerHTML = html;
  
  // Register global toggle handler
  window.__toggleCycle = toggleCycle;
  
  // Register symbol click handler if provided
  if (options.onSymbolClick) {
    window.__selectCycleSymbol = options.onSymbolClick;
  }
}

/**
 * Render a single cycle card.
 * @param {Object|Array} cycle - Cycle data (either object with symbols array or raw array)
 * @param {number} index - Cycle index
 * @param {Object} options - Render options
 * @returns {string} HTML
 */
function renderCycleCard(cycle, index, options = {}) {
  // Handle both formats: { symbols: [...] } or raw array
  const symbols = cycle.symbols || cycle;
  const size = symbols.length;
  const severity = getSeverity(size);
  const isExpanded = expandedCycles.has(index);
  
  // Build the cycle path display
  const pathSymbols = symbols.slice(0, 6);
  const cyclePath = pathSymbols.map(s => {
    const name = typeof s === 'string' ? s : (s.name || s.id || 'unknown');
    return `<span class="cycle-symbol" onclick="window.__selectCycleSymbol && window.__selectCycleSymbol(${JSON.stringify(s).replace(/"/g, '&quot;')})">${escapeHtml(name)}</span>`;
  }).join('<span class="cycle-arrow"> &#x2192; </span>');
  
  // Close the cycle by showing first symbol again
  const firstName = typeof symbols[0] === 'string' ? symbols[0] : (symbols[0].name || symbols[0].id || 'unknown');
  const fullPath = cyclePath + `<span class="cycle-arrow"> &#x2192; </span><span class="cycle-symbol">${escapeHtml(firstName)}</span>`;
  
  // Build expanded details
  let expandedHtml = '';
  if (isExpanded && symbols.length > 0) {
    expandedHtml = `
      <div class="cycle-details">
        <div class="cycle-details-header">Symbols in this cycle:</div>
        <div class="cycle-details-list">
          ${symbols.map(s => {
            const sym = typeof s === 'string' ? { name: s } : s;
            return `
              <div class="cycle-detail-item" onclick="window.__selectCycleSymbol && window.__selectCycleSymbol(${JSON.stringify(sym).replace(/"/g, '&quot;')})">
                <span class="cycle-detail-name">${escapeHtml(sym.name || 'unknown')}</span>
                ${sym.path || sym.file ? `<span class="cycle-detail-path">${escapeHtml(sym.path || sym.file)}${sym.line ? ':' + sym.line : ''}</span>` : ''}
                ${sym.kind || sym.type ? `<span class="cycle-detail-kind">${escapeHtml(sym.kind || sym.type)}</span>` : ''}
              </div>
            `;
          }).join('')}
        </div>
      </div>
    `;
  }
  
  return `
    <div class="cycle-card ${severity} ${isExpanded ? 'expanded' : ''}" data-cycle-index="${index}">
      <div class="cycle-card-header" onclick="window.__toggleCycle(${index})">
        <span class="cycle-card-toggle">${isExpanded ? '[-]' : '[+]'}</span>
        <span class="cycle-card-title">
          CYCLE ${index + 1}
          <span class="cycle-card-severity ${severity}">(${getSeverityLabel(severity)} - ${size} symbols)</span>
        </span>
      </div>
      <div class="cycle-path">${fullPath}</div>
      ${expandedHtml}
    </div>
  `;
}

/**
 * Fetch cycles data and render the panel.
 * @param {HTMLElement} container - Container element
 * @param {Object} options - Render options
 * @returns {Promise<void>}
 */
export async function fetchAndRenderCycles(container, options = {}) {
  if (!container) return;
  
  // Show loading state
  container.innerHTML = `
    <div class="cycles-loading">
      <span class="text-dim">loading cycles...</span>
    </div>
  `;
  
  const data = await fetchCycles();
  
  if (data === null) {
    // API not available yet - show pending message
    container.innerHTML = `
      <div class="cycles-pending">
        <span class="text-dim">Cycles API endpoint pending (Agent 2)</span>
        <span class="text-muted">The /api/cycles endpoint is being implemented.</span>
      </div>
    `;
    return;
  }
  
  renderCyclesPanel(container, data, options);
}

// =============================================================================
// MINI CYCLES WIDGET (for stats view)
// =============================================================================

/**
 * Render a compact cycles widget for the stats dashboard.
 * @param {HTMLElement} container - Container element
 * @param {Object} data - Stats data with cycles info
 * @param {Object} options - Render options
 */
export function renderCyclesWidget(container, data, options = {}) {
  if (!container) return;
  
  const cyclesCount = data?.cycles || 0;
  const cyclesSymbols = data?.cycles_symbols || 0;
  
  if (cyclesCount === 0) {
    container.innerHTML = `
      <div class="cycles-widget-empty">
        <span class="cycles-widget-icon">&#x2713;</span>
        <span>No cycles</span>
      </div>
    `;
    return;
  }
  
  container.innerHTML = `
    <div class="cycles-widget">
      <div class="cycles-widget-stat">
        <span class="cycles-widget-value warn">${cyclesCount}</span>
        <span class="cycles-widget-label">cycles</span>
      </div>
      <div class="cycles-widget-stat">
        <span class="cycles-widget-value">${cyclesSymbols}</span>
        <span class="cycles-widget-label">symbols involved</span>
      </div>
      <button class="cycles-widget-btn" onclick="window.__showCyclesDetail && window.__showCyclesDetail()">
        view details
      </button>
    </div>
  `;
}

// =============================================================================
// EXPORTS
// =============================================================================

export {
  fetchCycles,
  getSeverity,
  toggleCycle
};
