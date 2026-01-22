/**
 * Cycles View Module
 *
 * Clean, spacious view for circular dependencies.
 * Each cycle gets room to breathe.
 *
 * @module views/cycles
 */

/* global d3 */

import { escapeHtml, truncatePath } from '../utils.js';
import { renderCyclesSkeleton } from '../components/skeleton.js';
import { emptyNoCycles } from '../components/empty.js';
import { errorCyclesLoad } from '../components/error.js';

// =============================================================================
// CONSTANTS
// =============================================================================

const SEVERITY_THRESHOLDS = {
  critical: 6,
  high: 4,
  medium: 3
};

const SORT_OPTIONS = [
  { value: 'size', label: 'Size (largest)' },
  { value: 'files', label: 'Files affected' },
  { value: 'alpha', label: 'Alphabetical' }
];

// =============================================================================
// STATE
// =============================================================================

let cyclesState = {
  data: null,
  sortBy: 'size',
  minSize: 2,
  expandedCycles: new Set(),
  loading: false,
  error: null
};

// =============================================================================
// API
// =============================================================================

/**
 * Fetch cycles data from the API.
 * @returns {Promise<Object|null>} Cycles data
 */
async function fetchCycles() {
  try {
    const res = await fetch('/api/cycles');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  } catch (err) {
    console.error('Failed to fetch cycles:', err);
    return null;
  }
}

// =============================================================================
// HELPERS
// =============================================================================

/**
 * Get severity based on cycle size.
 * @param {number} size - Number of symbols in cycle
 * @returns {string} Severity level
 */
function getSeverity(size) {
  if (size >= SEVERITY_THRESHOLDS.critical) return 'critical';
  if (size >= SEVERITY_THRESHOLDS.high) return 'high';
  return 'medium';
}

/**
 * Get files involved in a cycle.
 * @param {Object} cycle - Cycle data
 * @returns {string[]} Unique file paths
 */
function getCycleFiles(cycle) {
  const files = new Set();
  const symbols = cycle.symbols || [];
  for (const sym of symbols) {
    if (sym.file) files.add(sym.file);
  }
  return Array.from(files);
}

/**
 * Sort cycles based on current sort option.
 * @param {Object[]} cycles - Cycles to sort
 * @param {string} sortBy - Sort option
 * @returns {Object[]} Sorted cycles
 */
function sortCycles(cycles, sortBy) {
  const sorted = [...cycles];
  switch (sortBy) {
    case 'size':
      sorted.sort((a, b) => (b.size || b.symbols?.length || 0) - (a.size || a.symbols?.length || 0));
      break;
    case 'files':
      sorted.sort((a, b) => getCycleFiles(b).length - getCycleFiles(a).length);
      break;
    case 'alpha':
      sorted.sort((a, b) => {
        const aName = a.symbols?.[0]?.name || '';
        const bName = b.symbols?.[0]?.name || '';
        return aName.localeCompare(bName);
      });
      break;
  }
  return sorted;
}

// =============================================================================
// MAIN RENDER
// =============================================================================

/**
 * Render the cycles view.
 * @param {Object} state - App state
 */
export async function renderCycles(state) {
  const container = document.getElementById('cycles-container');
  if (!container) return;
  
  // Show loading skeleton
  if (!cyclesState.data) {
    container.innerHTML = renderCyclesSkeleton(4);
  }
  
  // Fetch data if not cached
  if (!cyclesState.data || cyclesState.loading) {
    cyclesState.loading = true;
    cyclesState.data = await fetchCycles();
    cyclesState.loading = false;
    
    if (!cyclesState.data) {
      container.innerHTML = errorCyclesLoad('window.refreshCycles');
      return;
    }
  }
  
  const { total_cycles, total_symbols_in_cycles, cycles } = cyclesState.data;
  
  // Handle no cycles
  if (!cycles || cycles.length === 0) {
    container.innerHTML = emptyNoCycles();
    return;
  }
  
  // Filter and sort cycles
  const filteredCycles = cycles.filter(c => (c.size || c.symbols?.length || 0) >= cyclesState.minSize);
  const sortedCycles = sortCycles(filteredCycles, cyclesState.sortBy);
  
  // Count unique files
  const allFiles = new Set();
  for (const cycle of cycles) {
    getCycleFiles(cycle).forEach(f => allFiles.add(f));
  }
  
  // Render view
  container.innerHTML = `
    <div class="cycles-view">
      <div class="cycles-header">
        <div class="cycles-header-left">
          <h2 class="cycles-title">Circular Dependencies</h2>
          <span class="cycles-summary">${total_cycles} cycles, ${total_symbols_in_cycles} symbols, ${allFiles.size} files</span>
        </div>
        <div class="cycles-header-right">
          <select id="cycles-sort" class="cycles-select">
            ${SORT_OPTIONS.map(opt => `
              <option value="${opt.value}" ${cyclesState.sortBy === opt.value ? 'selected' : ''}>
                ${opt.label}
              </option>
            `).join('')}
          </select>
        </div>
      </div>
      
      <div class="cycles-list">
        ${sortedCycles.length === 0 ? `
          <div class="cycles-empty">No cycles match the current filter</div>
        ` : sortedCycles.map((cycle, idx) => renderCycleCard(cycle, idx)).join('')}
      </div>
    </div>
  `;
  
  // Attach event handlers
  attachCyclesEventHandlers(state);
  
  // Render cycle diagrams after DOM is ready
  requestAnimationFrame(() => {
    sortedCycles.forEach((cycle, idx) => {
      if (cyclesState.expandedCycles.has(idx)) {
        const ringContainer = document.getElementById(`cycle-ring-${idx}`);
        if (ringContainer) {
          renderCycleRing(ringContainer, cycle);
        }
      }
    });
  });
}

/**
 * Render a single cycle card.
 * @param {Object} cycle - Cycle data
 * @param {number} idx - Cycle index
 * @returns {string} HTML
 */
function renderCycleCard(cycle, idx) {
  const symbols = cycle.symbols || [];
  const size = cycle.size || symbols.length;
  const severity = getSeverity(size);
  const files = getCycleFiles(cycle);
  const isExpanded = cyclesState.expandedCycles.has(idx);
  
  // Build simple path: A -> B -> C -> A
  const pathNames = symbols.map(s => s.name || 'unknown');
  
  return `
    <div class="cycle-card ${isExpanded ? 'expanded' : ''}" data-cycle-idx="${idx}">
      <div class="cycle-card-header" data-cycle-idx="${idx}">
        <div class="cycle-card-left">
          <span class="cycle-number">#${idx + 1}</span>
          <span class="cycle-severity ${severity}">${severity}</span>
          <span class="cycle-meta">${size} symbols in ${files.length} file${files.length !== 1 ? 's' : ''}</span>
        </div>
        <span class="cycle-toggle">${isExpanded ? '[-]' : '[+]'}</span>
      </div>
      
      ${isExpanded ? `
        <div class="cycle-card-content">
          <div class="cycle-flow">
            ${pathNames.map((name, i) => `<span class="cycle-node">${escapeHtml(name)}</span>${i < pathNames.length - 1 ? '<span class="cycle-arrow">-></span>' : ''}`).join('')}<span class="cycle-arrow">-></span><span class="cycle-node cycle-node-return">${escapeHtml(pathNames[0])}</span>
          </div>
          
          <div class="cycle-bottom">
            <div class="cycle-files">
              <div class="cycle-section-label">Files</div>
              ${files.map(f => `<div class="cycle-file">${escapeHtml(truncatePath(f))}</div>`).join('')}
            </div>
            <div class="cycle-diagram">
              <div id="cycle-ring-${idx}" class="cycle-ring"></div>
            </div>
          </div>
        </div>
      ` : ''}
    </div>
  `;
}

/**
 * Render a circular ring diagram for a cycle.
 * @param {HTMLElement} container - Container element
 * @param {Object} cycle - Cycle data
 */
function renderCycleRing(container, cycle) {
  const symbols = cycle.symbols || [];
  if (symbols.length < 2) return;
  
  const size = 180;
  const radius = 70;
  const nodeRadius = 6;
  
  container.innerHTML = '';
  
  const svg = d3.select(container)
    .append('svg')
    .attr('width', size)
    .attr('height', size);
  
  const g = svg.append('g')
    .attr('transform', `translate(${size/2}, ${size/2})`);
  
  // Calculate node positions in a circle
  const angleStep = (2 * Math.PI) / symbols.length;
  const nodes = symbols.map((sym, i) => ({
    x: radius * Math.cos(angleStep * i - Math.PI/2),
    y: radius * Math.sin(angleStep * i - Math.PI/2),
    name: sym.name || 'unknown'
  }));
  
  // Draw connecting lines
  for (let i = 0; i < nodes.length; i++) {
    const from = nodes[i];
    const to = nodes[(i + 1) % nodes.length];
    g.append('line')
      .attr('x1', from.x)
      .attr('y1', from.y)
      .attr('x2', to.x)
      .attr('y2', to.y)
      .attr('stroke', 'var(--cyan-dim)')
      .attr('stroke-width', 1.5);
  }
  
  // Draw nodes
  nodes.forEach((node, i) => {
    g.append('circle')
      .attr('cx', node.x)
      .attr('cy', node.y)
      .attr('r', nodeRadius)
      .attr('fill', i === 0 ? 'var(--yellow)' : 'var(--cyan)');
  });
  
  // Add center label
  g.append('text')
    .attr('text-anchor', 'middle')
    .attr('dy', '0.35em')
    .attr('fill', 'var(--text-dim)')
    .attr('font-size', '11px')
    .text(`${symbols.length} nodes`);
}

// =============================================================================
// EVENT HANDLERS
// =============================================================================

/**
 * Attach event handlers for cycles view.
 * @param {Object} state - App state
 */
function attachCyclesEventHandlers(state) {
  // Sort dropdown
  const sortSelect = document.getElementById('cycles-sort');
  if (sortSelect) {
    sortSelect.addEventListener('change', (e) => {
      cyclesState.sortBy = e.target.value;
      renderCycles(state);
    });
  }
  
  // Cycle card expansion
  document.querySelectorAll('.cycle-card-header').forEach(header => {
    header.addEventListener('click', () => {
      const idx = parseInt(header.dataset.cycleIdx, 10);
      toggleCycleExpansion(idx, state);
    });
  });
}

/**
 * Toggle cycle card expansion.
 * @param {number} idx - Cycle index
 * @param {Object} state - App state
 */
function toggleCycleExpansion(idx, state) {
  if (cyclesState.expandedCycles.has(idx)) {
    cyclesState.expandedCycles.delete(idx);
  } else {
    cyclesState.expandedCycles.add(idx);
  }
  renderCycles(state);
  
  // Render ring diagram if expanding
  if (cyclesState.expandedCycles.has(idx)) {
    requestAnimationFrame(() => {
      const ringContainer = document.getElementById(`cycle-ring-${idx}`);
      if (ringContainer && cyclesState.data?.cycles) {
        const filteredCycles = cyclesState.data.cycles.filter(c => 
          (c.size || c.symbols?.length || 0) >= cyclesState.minSize
        );
        const sortedCycles = sortCycles(filteredCycles, cyclesState.sortBy);
        const cycle = sortedCycles[idx];
        if (cycle) {
          renderCycleRing(ringContainer, cycle);
        }
      }
    });
  }
}

// =============================================================================
// REFRESH
// =============================================================================

/**
 * Force refresh cycles data.
 */
export function refreshCycles() {
  cyclesState.data = null;
  cyclesState.expandedCycles.clear();
}

// =============================================================================
// EXPORTS
// =============================================================================

export { fetchCycles, cyclesState };
