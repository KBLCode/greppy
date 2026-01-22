/**
 * Stats View Module
 *
 * Renders the stats dashboard with charts and metrics.
 *
 * @module views/stats
 */

/* global d3 */

import { fetchList } from '../api.js';
import { escapeHtml, truncatePath } from '../utils.js';
import { renderStatsSkeleton } from '../components/skeleton.js';
import { emptyNoSymbols } from '../components/empty.js';
import { errorStatsLoad } from '../components/error.js';

// =============================================================================
// LOADING STATE
// =============================================================================

/**
 * Show loading skeleton for stats view.
 */
export function showStatsLoading() {
  const container = document.getElementById('stats-container');
  if (container) {
    container.innerHTML = renderStatsSkeleton();
  }
}

/**
 * Show error state for stats view.
 */
export function showStatsError() {
  const container = document.getElementById('stats-container');
  if (container) {
    container.innerHTML = errorStatsLoad('window.refreshStats');
  }
}

// =============================================================================
// MAIN RENDER
// =============================================================================

/**
 * Render the stats view.
 * @param {Object} state - App state with stats data
 */
export async function renderStats(state) {
  const container = document.getElementById('stats-container');
  if (!container) return;
  
  // Handle no data state
  if (!state.stats) {
    container.innerHTML = emptyNoSymbols();
    return;
  }
  
  const s = state.stats;
  
  // Handle empty codebase
  if (s.symbols === 0) {
    container.innerHTML = emptyNoSymbols();
    return;
  }
  const deadPct = s.symbols > 0 ? ((s.dead / s.symbols) * 100).toFixed(1) : 0;
  const healthPct = s.symbols > 0 ? Math.round(((s.symbols - s.dead) / s.symbols) * 100) : 100;
  
  // Fetch list data for top referenced symbols
  let topSymbols = [];
  let fileHealth = [];
  
  try {
    const listData = await fetchList({ sort: 'refs' });
    if (listData?.items) {
      // Get top 5 most referenced symbols
      topSymbols = [...listData.items]
        .sort((a, b) => (b.refs || 0) - (a.refs || 0))
        .slice(0, 5);
      
      // Calculate file health
      fileHealth = calculateFileHealth(listData.items);
    }
  } catch (err) {
    console.warn('Failed to fetch list for stats:', err);
  }
  
  container.innerHTML = `
    <div class="stats-header" role="region" aria-label="Codebase metrics">
      <div class="stat-big">
        <span class="stat-big-value">${s.files.toLocaleString()}</span>
        <span class="stat-big-label">files</span>
      </div>
      <div class="stat-big">
        <span class="stat-big-value">${s.symbols.toLocaleString()}</span>
        <span class="stat-big-label">symbols</span>
      </div>
      <div class="stat-big">
        <span class="stat-big-value dead">${s.dead.toLocaleString()}</span>
        <span class="stat-big-label">dead (${deadPct}%)</span>
      </div>
      <div class="stat-big">
        <span class="stat-big-value warn">${s.cycles.toLocaleString()}</span>
        <span class="stat-big-label">cycles</span>
      </div>
      <div class="stat-big health">
        <span class="stat-big-value ${getHealthClass(healthPct)}">${healthPct}%</span>
        <span class="stat-big-label">health</span>
      </div>
    </div>
    
    <div class="stats-panels">
      <div class="stats-row">
        <div class="chart-panel" role="region" aria-label="Symbols by kind">
          <div class="chart-title">SYMBOLS BY KIND</div>
          <div id="chart-types" class="chart-area"></div>
        </div>
        
        <div class="chart-panel" role="region" aria-label="Symbols by state">
          <div class="chart-title">SYMBOLS BY STATE</div>
          <div id="chart-state" class="chart-area"></div>
        </div>
      </div>
      
      <div class="stats-row">
        <div class="chart-panel" role="region" aria-label="Top referenced symbols">
          <div class="chart-title">TOP REFERENCED SYMBOLS</div>
          <div id="top-symbols" class="stats-list-area">
            ${renderTopSymbols(topSymbols)}
          </div>
        </div>
        
        <div class="chart-panel" role="region" aria-label="Files by health">
          <div class="chart-title">FILES BY HEALTH</div>
          <div id="files-health" class="stats-list-area">
            ${renderFileHealth(fileHealth)}
          </div>
        </div>
      </div>
      
      ${s.cycles > 0 ? `
      <div class="stats-row">
        <div class="chart-panel cycles-panel" role="region" aria-label="Cycles summary">
          <div class="chart-title">CYCLES SUMMARY</div>
          <div class="cycles-summary">
            <div class="cycles-count">
              <span class="cycles-count-value">${s.cycles}</span>
              <span class="cycles-count-label">symbols in circular dependencies</span>
            </div>
            <div class="cycles-hint">
              Run <code>greppy trace --cycles</code> for detailed cycle analysis
            </div>
          </div>
        </div>
      </div>
      ` : ''}
    </div>
  `;
  
  // Render D3 charts after DOM is ready
  requestAnimationFrame(() => {
    renderTypesChart(s.breakdown);
    renderStateChart(s);
  });
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/**
 * Get health class based on percentage.
 * @param {number} pct - Health percentage
 * @returns {string} CSS class
 */
function getHealthClass(pct) {
  if (pct >= 70) return 'healthy';
  if (pct >= 40) return 'warn';
  return 'dead';
}

/**
 * Calculate health metrics per file.
 * @param {Array} items - Symbol items
 * @returns {Array} File health data sorted by health ascending (worst first)
 */
function calculateFileHealth(items) {
  const fileMap = new Map();
  
  for (const item of items) {
    if (!item.path) continue;
    
    if (!fileMap.has(item.path)) {
      fileMap.set(item.path, { path: item.path, total: 0, dead: 0 });
    }
    
    const file = fileMap.get(item.path);
    file.total++;
    if (item.state === 'dead') {
      file.dead++;
    }
  }
  
  return Array.from(fileMap.values())
    .map(f => ({
      ...f,
      healthPct: f.total > 0 ? Math.round(((f.total - f.dead) / f.total) * 100) : 100
    }))
    .filter(f => f.total >= 3) // Only show files with 3+ symbols
    .sort((a, b) => a.healthPct - b.healthPct) // Worst health first
    .slice(0, 5);
}

// =============================================================================
// TOP SYMBOLS SECTION
// =============================================================================

/**
 * Render top referenced symbols list.
 * @param {Array} symbols - Top symbols
 * @returns {string} HTML
 */
function renderTopSymbols(symbols) {
  if (!symbols.length) {
    return '<div class="stats-list-empty">No data available</div>';
  }
  
  return `
    <ol class="top-symbols-list" aria-label="Top referenced symbols">
      ${symbols.map((sym, idx) => `
        <li class="top-symbol-item">
          <span class="top-symbol-rank">${idx + 1}.</span>
          <span class="top-symbol-name" title="${escapeHtml(sym.path)}">${escapeHtml(sym.name)}</span>
          <span class="top-symbol-refs">${sym.refs || 0} refs</span>
        </li>
      `).join('')}
    </ol>
  `;
}

// =============================================================================
// FILE HEALTH SECTION
// =============================================================================

/**
 * Render file health list.
 * @param {Array} files - File health data
 * @returns {string} HTML
 */
function renderFileHealth(files) {
  if (!files.length) {
    return '<div class="stats-list-empty">No files with 3+ symbols</div>';
  }
  
  const maxSymbols = Math.max(...files.map(f => f.total), 1);
  
  return `
    <ul class="file-health-list" aria-label="Files by health">
      ${files.map(file => {
        const healthClass = getHealthClass(file.healthPct);
        const barWidth = (file.total / maxSymbols) * 100;
        
        return `
          <li class="file-health-item">
            <div class="file-health-info">
              <span class="file-health-path" title="${escapeHtml(file.path)}">${escapeHtml(truncatePath(file.path))}</span>
              <span class="file-health-stats">${file.total - file.dead}/${file.total}</span>
            </div>
            <div class="file-health-bar-container">
              <div class="file-health-bar ${healthClass}" style="width: ${barWidth}%">
                <div class="file-health-fill" style="width: ${file.healthPct}%"></div>
              </div>
              <span class="file-health-pct ${healthClass}">${file.healthPct}%</span>
            </div>
          </li>
        `;
      }).join('')}
    </ul>
  `;
}

// =============================================================================
// TYPES CHART (D3)
// =============================================================================

/**
 * Render horizontal bar chart of symbol types.
 * @param {Object} breakdown - Symbol counts by type
 */
function renderTypesChart(breakdown) {
  const container = d3.select('#chart-types');
  if (!container.node()) return;
  
  const rect = container.node().getBoundingClientRect();
  const width = rect.width || 300;
  const height = rect.height || 180;
  const margin = { top: 10, right: 60, bottom: 10, left: 10 };
  
  const data = Object.entries(breakdown || {})
    .filter(([_, v]) => v > 0)
    .map(([name, value]) => ({ name, value }))
    .sort((a, b) => b.value - a.value)
    .slice(0, 6);
  
  if (data.length === 0) {
    container.html('<div class="chart-empty">No data</div>');
    return;
  }
  
  container.html(''); // Clear previous
  
  const svg = container.append('svg')
    .attr('width', width)
    .attr('height', height)
    .attr('role', 'img')
    .attr('aria-label', 'Bar chart showing symbols by kind');
  
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  
  const x = d3.scaleLinear()
    .domain([0, d3.max(data, d => d.value)])
    .range([0, innerWidth]);
  
  const y = d3.scaleBand()
    .domain(data.map(d => d.name))
    .range([0, innerHeight])
    .padding(0.35);
  
  const g = svg.append('g')
    .attr('transform', `translate(${margin.left},${margin.top})`);
  
  // Bars
  g.selectAll('rect')
    .data(data)
    .join('rect')
    .attr('x', 0)
    .attr('y', d => y(d.name))
    .attr('width', d => x(d.value))
    .attr('height', y.bandwidth())
    .attr('fill', '#00d4d4');
  
  // Labels
  g.selectAll('.bar-label')
    .data(data)
    .join('text')
    .attr('class', 'bar-label')
    .attr('x', d => x(d.value) + 6)
    .attr('y', d => y(d.name) + y.bandwidth() / 2)
    .attr('dy', '0.35em')
    .attr('fill', '#666')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => `${d.name} ${d.value}`);
}

// =============================================================================
// STATE CHART (D3)
// =============================================================================

/**
 * Render horizontal bar chart of symbol states.
 * @param {Object} stats - Stats object with symbols, dead, cycles
 */
function renderStateChart(stats) {
  const container = d3.select('#chart-state');
  if (!container.node()) return;
  
  const rect = container.node().getBoundingClientRect();
  const width = rect.width || 300;
  const height = rect.height || 180;
  const margin = { top: 10, right: 80, bottom: 10, left: 10 };
  
  const used = stats.symbols - stats.dead;
  const data = [
    { name: 'used', value: used, color: '#00ff00' },
    { name: 'dead', value: stats.dead, color: '#ff3333' },
    { name: 'in cycle', value: stats.cycles, color: '#ffcc00' }
  ].filter(d => d.value > 0);
  
  if (data.length === 0) {
    container.html('<div class="chart-empty">No data</div>');
    return;
  }
  
  container.html(''); // Clear previous
  
  const svg = container.append('svg')
    .attr('width', width)
    .attr('height', height)
    .attr('role', 'img')
    .attr('aria-label', 'Bar chart showing symbols by state');
  
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  
  const maxValue = d3.max(data, d => d.value);
  
  const x = d3.scaleLinear()
    .domain([0, maxValue])
    .range([0, innerWidth]);
  
  const y = d3.scaleBand()
    .domain(data.map(d => d.name))
    .range([0, innerHeight])
    .padding(0.35);
  
  const g = svg.append('g')
    .attr('transform', `translate(${margin.left},${margin.top})`);
  
  // Bars
  g.selectAll('rect')
    .data(data)
    .join('rect')
    .attr('x', 0)
    .attr('y', d => y(d.name))
    .attr('width', d => x(d.value))
    .attr('height', y.bandwidth())
    .attr('fill', d => d.color);
  
  // Labels
  g.selectAll('.bar-label')
    .data(data)
    .join('text')
    .attr('class', 'bar-label')
    .attr('x', d => x(d.value) + 6)
    .attr('y', d => y(d.name) + y.bandwidth() / 2)
    .attr('dy', '0.35em')
    .attr('fill', '#666')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => `${d.name} ${d.value}`);
}
