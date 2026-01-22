/**
 * Timeline View Module
 *
 * Renders the timeline view with snapshots and trend charts.
 * Tracks codebase metrics over time.
 *
 * @module views/timeline
 */

/* global d3 */

import { fetchSnapshots, createSnapshot as apiCreateSnapshot, compareSnapshots } from '../api.js';
import { escapeHtml } from '../utils.js';
import { renderTimelineSkeleton } from '../components/skeleton.js';
import { emptyNoSnapshots } from '../components/empty.js';
import { errorTimelineLoad } from '../components/error.js';

// =============================================================================
// MODULE STATE
// =============================================================================

let snapshots = [];
let isLoading = false;
let selectedForCompare = [];

// =============================================================================
// LOADING STATE
// =============================================================================

/**
 * Show loading skeleton for timeline view.
 */
export function showTimelineLoading() {
  const container = document.getElementById('timeline-container');
  if (container) {
    container.innerHTML = renderTimelineSkeleton();
  }
}

/**
 * Show error state for timeline view.
 */
export function showTimelineError() {
  const container = document.getElementById('timeline-container');
  if (container) {
    container.innerHTML = errorTimelineLoad('window.refreshTimeline');
  }
}

// =============================================================================
// MAIN RENDER
// =============================================================================

/**
 * Render the timeline view.
 * @param {Object} state - App state
 */
export async function renderTimeline(state) {
  const container = document.getElementById('timeline-container');
  if (!container) return;
  
  // Show loading state
  showTimelineLoading();
  isLoading = true;
  
  try {
    const response = await fetchSnapshots();
    snapshots = response.snapshots || [];
    isLoading = false;
    
    // Handle no snapshots
    if (snapshots.length === 0) {
      container.innerHTML = emptyNoSnapshots();
      return;
    }
    
    // Sort by date (newest first for table, oldest first for chart)
    const sortedForTable = [...snapshots].sort((a, b) => 
      new Date(b.created_at) - new Date(a.created_at)
    );
    const sortedForChart = [...snapshots].sort((a, b) => 
      new Date(a.created_at) - new Date(b.created_at)
    );
    
    container.innerHTML = `
      <div class="timeline-view" role="region" aria-label="Codebase timeline">
        <!-- Header with create button -->
        <div class="timeline-header">
          <div class="timeline-title">
            <span class="timeline-title-text">CODEBASE TIMELINE</span>
            <span class="timeline-snapshot-count">${snapshots.length} snapshot${snapshots.length !== 1 ? 's' : ''}</span>
          </div>
          <div class="timeline-actions">
            ${selectedForCompare.length === 2 ? `
              <button class="btn btn-secondary" onclick="window.compareSelectedSnapshots()">
                Compare Selected (${selectedForCompare.length})
              </button>
            ` : ''}
            <button class="btn btn-primary" onclick="window.createSnapshot()" ${isLoading ? 'disabled' : ''}>
              + Create Snapshot
            </button>
          </div>
        </div>
        
        <!-- Trend Charts -->
        <div class="timeline-charts">
          <div class="timeline-chart-panel" role="region" aria-label="Metrics over time">
            <div class="chart-title">METRICS OVER TIME</div>
            <div id="timeline-chart" class="timeline-chart-area"></div>
          </div>
        </div>
        
        <!-- Snapshot Table -->
        <div class="timeline-table-panel">
          <div class="chart-title">SNAPSHOTS</div>
          <table class="timeline-table" role="grid" aria-label="Snapshot history">
            <thead>
              <tr>
                <th class="timeline-th-select"></th>
                <th class="timeline-th-date">Date</th>
                <th class="timeline-th-name">Name</th>
                <th class="timeline-th-files">Files</th>
                <th class="timeline-th-symbols">Symbols</th>
                <th class="timeline-th-dead">Dead</th>
                <th class="timeline-th-cycles">Cycles</th>
                <th class="timeline-th-actions">Actions</th>
              </tr>
            </thead>
            <tbody>
              ${sortedForTable.map((snap, idx) => renderSnapshotRow(snap, idx)).join('')}
            </tbody>
          </table>
        </div>
      </div>
      
      <!-- Compare Modal -->
      <div id="compare-modal" class="modal hidden">
        <div class="modal-backdrop" onclick="window.closeCompareModal()"></div>
        <div class="modal-content modal-lg">
          <div class="modal-header">
            <span class="modal-title">COMPARE SNAPSHOTS</span>
            <button class="btn-icon" onclick="window.closeCompareModal()">×</button>
          </div>
          <div id="compare-modal-body" class="modal-body">
            <!-- Comparison content loaded here -->
          </div>
        </div>
      </div>
    `;
    
    // Render D3 chart after DOM is ready
    requestAnimationFrame(() => {
      renderTrendChart(sortedForChart);
    });
    
  } catch (err) {
    console.error('Failed to load timeline:', err);
    isLoading = false;
    showTimelineError();
  }
}

// =============================================================================
// SNAPSHOT ROW
// =============================================================================

/**
 * Render a single snapshot row.
 * @param {Object} snap - Snapshot data
 * @param {number} idx - Row index
 * @returns {string} HTML string
 */
function renderSnapshotRow(snap, idx) {
  const date = new Date(snap.created_at);
  const dateStr = formatDate(date);
  const timeStr = formatTime(date);
  const isSelected = selectedForCompare.includes(snap.id);
  const deadPct = snap.symbols > 0 ? ((snap.dead / snap.symbols) * 100).toFixed(1) : 0;
  
  return `
    <tr class="timeline-row ${isSelected ? 'selected' : ''}" data-id="${escapeHtml(snap.id)}">
      <td class="timeline-td-select">
        <input type="checkbox" 
               class="timeline-checkbox" 
               ${isSelected ? 'checked' : ''} 
               onchange="window.toggleSnapshotSelect('${escapeHtml(snap.id)}')"
               aria-label="Select for comparison"
               ${selectedForCompare.length >= 2 && !isSelected ? 'disabled' : ''}>
      </td>
      <td class="timeline-td-date">
        <span class="timeline-date">${dateStr}</span>
        <span class="timeline-time">${timeStr}</span>
      </td>
      <td class="timeline-td-name">
        ${snap.name ? escapeHtml(snap.name) : '<span class="text-dim">-</span>'}
      </td>
      <td class="timeline-td-files">${snap.files.toLocaleString()}</td>
      <td class="timeline-td-symbols">${snap.symbols.toLocaleString()}</td>
      <td class="timeline-td-dead">
        <span class="dead">${snap.dead.toLocaleString()}</span>
        <span class="text-dim">(${deadPct}%)</span>
      </td>
      <td class="timeline-td-cycles">
        <span class="${snap.cycles > 0 ? 'warn' : ''}">${snap.cycles.toLocaleString()}</span>
      </td>
      <td class="timeline-td-actions">
        ${idx > 0 ? `
          <button class="btn-sm" onclick="window.compareWithPrevious('${escapeHtml(snap.id)}', '${escapeHtml(snapshots[idx + 1]?.id || '')}')">
            vs prev
          </button>
        ` : ''}
      </td>
    </tr>
  `;
}

// =============================================================================
// TREND CHART (D3)
// =============================================================================

/**
 * Render the trend line chart.
 * @param {Array} data - Sorted snapshot data (oldest first)
 */
function renderTrendChart(data) {
  const container = d3.select('#timeline-chart');
  if (!container.node() || data.length < 2) {
    if (data.length < 2) {
      container.html('<div class="chart-empty">Need at least 2 snapshots to show trends</div>');
    }
    return;
  }
  
  const rect = container.node().getBoundingClientRect();
  const width = rect.width || 600;
  const height = 250;
  const margin = { top: 20, right: 100, bottom: 40, left: 60 };
  
  container.html(''); // Clear previous
  
  const svg = container.append('svg')
    .attr('width', width)
    .attr('height', height)
    .attr('role', 'img')
    .attr('aria-label', 'Line chart showing codebase metrics over time');
  
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  
  const g = svg.append('g')
    .attr('transform', `translate(${margin.left},${margin.top})`);
  
  // Parse dates
  const parseDate = d => new Date(d.created_at);
  
  // X scale (time)
  const x = d3.scaleTime()
    .domain(d3.extent(data, parseDate))
    .range([0, innerWidth]);
  
  // Find max values for Y scales
  const maxSymbols = d3.max(data, d => d.symbols);
  const maxDead = d3.max(data, d => d.dead);
  const maxCycles = d3.max(data, d => d.cycles);
  const maxY = Math.max(maxSymbols, maxDead * 5, maxCycles * 10);
  
  // Y scale (shared, normalized)
  const y = d3.scaleLinear()
    .domain([0, maxY])
    .range([innerHeight, 0]);
  
  // Line generators
  const lineSymbols = d3.line()
    .x(d => x(parseDate(d)))
    .y(d => y(d.symbols))
    .curve(d3.curveMonotoneX);
  
  const lineDead = d3.line()
    .x(d => x(parseDate(d)))
    .y(d => y(d.dead))
    .curve(d3.curveMonotoneX);
  
  const lineCycles = d3.line()
    .x(d => x(parseDate(d)))
    .y(d => y(d.cycles))
    .curve(d3.curveMonotoneX);
  
  // X axis
  g.append('g')
    .attr('class', 'axis axis-x')
    .attr('transform', `translate(0,${innerHeight})`)
    .call(d3.axisBottom(x)
      .ticks(Math.min(data.length, 6))
      .tickFormat(d3.timeFormat('%m/%d')));
  
  // Y axis
  g.append('g')
    .attr('class', 'axis axis-y')
    .call(d3.axisLeft(y)
      .ticks(5)
      .tickFormat(d3.format('~s')));
  
  // Grid lines
  g.append('g')
    .attr('class', 'grid')
    .call(d3.axisLeft(y)
      .ticks(5)
      .tickSize(-innerWidth)
      .tickFormat(''))
    .selectAll('line')
    .attr('stroke', '#333')
    .attr('stroke-dasharray', '2,2');
  
  // Lines
  g.append('path')
    .datum(data)
    .attr('class', 'timeline-line timeline-line-symbols')
    .attr('fill', 'none')
    .attr('stroke', '#00d4d4')
    .attr('stroke-width', 2)
    .attr('d', lineSymbols);
  
  g.append('path')
    .datum(data)
    .attr('class', 'timeline-line timeline-line-dead')
    .attr('fill', 'none')
    .attr('stroke', '#ff3333')
    .attr('stroke-width', 2)
    .attr('d', lineDead);
  
  g.append('path')
    .datum(data)
    .attr('class', 'timeline-line timeline-line-cycles')
    .attr('fill', 'none')
    .attr('stroke', '#ffcc00')
    .attr('stroke-width', 2)
    .attr('d', lineCycles);
  
  // Points
  const addPoints = (dataset, color, key) => {
    g.selectAll(`.point-${key}`)
      .data(dataset)
      .join('circle')
      .attr('class', `timeline-point point-${key}`)
      .attr('cx', d => x(parseDate(d)))
      .attr('cy', d => y(d[key]))
      .attr('r', 4)
      .attr('fill', color)
      .attr('stroke', '#0a0a0a')
      .attr('stroke-width', 2)
      .style('cursor', 'pointer')
      .on('mouseover', function(event, d) {
        d3.select(this).attr('r', 6);
        showTooltip(event, d, key);
      })
      .on('mouseout', function() {
        d3.select(this).attr('r', 4);
        hideTooltip();
      });
  };
  
  addPoints(data, '#00d4d4', 'symbols');
  addPoints(data, '#ff3333', 'dead');
  addPoints(data, '#ffcc00', 'cycles');
  
  // Legend
  const legend = svg.append('g')
    .attr('transform', `translate(${width - margin.right + 10}, ${margin.top})`);
  
  const legendItems = [
    { label: 'symbols', color: '#00d4d4' },
    { label: 'dead', color: '#ff3333' },
    { label: 'cycles', color: '#ffcc00' }
  ];
  
  legendItems.forEach((item, i) => {
    const row = legend.append('g')
      .attr('transform', `translate(0, ${i * 20})`);
    
    row.append('line')
      .attr('x1', 0)
      .attr('x2', 20)
      .attr('y1', 0)
      .attr('y2', 0)
      .attr('stroke', item.color)
      .attr('stroke-width', 2);
    
    row.append('text')
      .attr('x', 25)
      .attr('y', 4)
      .attr('fill', '#888')
      .attr('font-size', '11px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(item.label);
  });
}

// =============================================================================
// TOOLTIP
// =============================================================================

/**
 * Show tooltip for chart point.
 */
function showTooltip(event, d, key) {
  let tooltip = document.getElementById('timeline-tooltip');
  if (!tooltip) {
    tooltip = document.createElement('div');
    tooltip.id = 'timeline-tooltip';
    tooltip.className = 'timeline-tooltip';
    document.body.appendChild(tooltip);
  }
  
  const date = new Date(d.created_at);
  tooltip.innerHTML = `
    <div class="tooltip-date">${formatDate(date)} ${formatTime(date)}</div>
    <div class="tooltip-value">${key}: ${d[key].toLocaleString()}</div>
  `;
  
  tooltip.style.left = `${event.pageX + 10}px`;
  tooltip.style.top = `${event.pageY - 10}px`;
  tooltip.style.display = 'block';
}

/**
 * Hide tooltip.
 */
function hideTooltip() {
  const tooltip = document.getElementById('timeline-tooltip');
  if (tooltip) {
    tooltip.style.display = 'none';
  }
}

// =============================================================================
// HELPERS
// =============================================================================

/**
 * Format date for display.
 * @param {Date} date
 * @returns {string}
 */
function formatDate(date) {
  return date.toLocaleDateString('en-US', { 
    month: 'short', 
    day: 'numeric',
    year: 'numeric'
  });
}

/**
 * Format time for display.
 * @param {Date} date
 * @returns {string}
 */
function formatTime(date) {
  return date.toLocaleTimeString('en-US', { 
    hour: '2-digit', 
    minute: '2-digit'
  });
}

// =============================================================================
// GLOBAL HANDLERS
// =============================================================================

/**
 * Create a new snapshot.
 */
window.createSnapshot = async function() {
  try {
    const name = prompt('Snapshot name (optional):');
    await apiCreateSnapshot(name || null);
    // Refresh the view
    window.refreshTimeline();
  } catch (err) {
    console.error('Failed to create snapshot:', err);
    alert('Failed to create snapshot');
  }
};

/**
 * Toggle snapshot selection for comparison.
 */
window.toggleSnapshotSelect = function(id) {
  const idx = selectedForCompare.indexOf(id);
  if (idx >= 0) {
    selectedForCompare.splice(idx, 1);
  } else if (selectedForCompare.length < 2) {
    selectedForCompare.push(id);
  }
  // Re-render to update UI
  window.refreshTimeline();
};

/**
 * Compare selected snapshots.
 */
window.compareSelectedSnapshots = async function() {
  if (selectedForCompare.length !== 2) return;
  await showComparison(selectedForCompare[0], selectedForCompare[1]);
};

/**
 * Compare with previous snapshot.
 */
window.compareWithPrevious = async function(currentId, previousId) {
  if (!previousId) return;
  await showComparison(previousId, currentId);
};

/**
 * Show comparison modal.
 */
async function showComparison(aId, bId) {
  const modal = document.getElementById('compare-modal');
  const body = document.getElementById('compare-modal-body');
  if (!modal || !body) return;
  
  modal.classList.remove('hidden');
  body.innerHTML = '<div class="loading-indicator">Loading comparison...</div>';
  
  try {
    const comparison = await compareSnapshots(aId, bId);
    body.innerHTML = renderComparison(comparison);
  } catch (err) {
    console.error('Failed to compare snapshots:', err);
    body.innerHTML = '<div class="error-state">Failed to load comparison</div>';
  }
}

/**
 * Render comparison content.
 */
function renderComparison(comp) {
  const { a, b, diff } = comp;
  
  const formatDiff = (val) => {
    if (val === 0) return '<span class="text-dim">-</span>';
    const sign = val > 0 ? '+' : '';
    const cls = val > 0 ? 'warn' : 'healthy';
    return `<span class="${cls}">${sign}${val.toLocaleString()}</span>`;
  };
  
  const formatDiffDead = (val) => {
    if (val === 0) return '<span class="text-dim">-</span>';
    const sign = val > 0 ? '+' : '';
    // More dead = bad (red), less dead = good (green)
    const cls = val > 0 ? 'dead' : 'healthy';
    return `<span class="${cls}">${sign}${val.toLocaleString()}</span>`;
  };
  
  return `
    <div class="compare-grid">
      <div class="compare-header">
        <div class="compare-label">Metric</div>
        <div class="compare-snap">
          <div class="compare-snap-label">Before</div>
          <div class="compare-snap-date">${formatDate(new Date(a.created_at))}</div>
        </div>
        <div class="compare-snap">
          <div class="compare-snap-label">After</div>
          <div class="compare-snap-date">${formatDate(new Date(b.created_at))}</div>
        </div>
        <div class="compare-diff">Change</div>
      </div>
      
      <div class="compare-row">
        <div class="compare-label">Files</div>
        <div class="compare-value">${a.files.toLocaleString()}</div>
        <div class="compare-value">${b.files.toLocaleString()}</div>
        <div class="compare-diff">${formatDiff(diff.files)}</div>
      </div>
      
      <div class="compare-row">
        <div class="compare-label">Symbols</div>
        <div class="compare-value">${a.symbols.toLocaleString()}</div>
        <div class="compare-value">${b.symbols.toLocaleString()}</div>
        <div class="compare-diff">${formatDiff(diff.symbols)}</div>
      </div>
      
      <div class="compare-row">
        <div class="compare-label">Dead Code</div>
        <div class="compare-value dead">${a.dead.toLocaleString()}</div>
        <div class="compare-value dead">${b.dead.toLocaleString()}</div>
        <div class="compare-diff">${formatDiffDead(diff.dead)}</div>
      </div>
      
      <div class="compare-row">
        <div class="compare-label">Cycles</div>
        <div class="compare-value ${a.cycles > 0 ? 'warn' : ''}">${a.cycles.toLocaleString()}</div>
        <div class="compare-value ${b.cycles > 0 ? 'warn' : ''}">${b.cycles.toLocaleString()}</div>
        <div class="compare-diff">${formatDiffDead(diff.cycles)}</div>
      </div>
      
      <div class="compare-summary">
        ${getSummaryText(diff)}
      </div>
    </div>
  `;
}

/**
 * Get summary text for comparison.
 */
function getSummaryText(diff) {
  const changes = [];
  
  if (diff.files !== 0) {
    changes.push(diff.files > 0 ? `${diff.files} new files` : `${Math.abs(diff.files)} files removed`);
  }
  if (diff.symbols !== 0) {
    changes.push(diff.symbols > 0 ? `${diff.symbols} new symbols` : `${Math.abs(diff.symbols)} symbols removed`);
  }
  if (diff.dead !== 0) {
    changes.push(diff.dead > 0 ? `${diff.dead} more dead code` : `${Math.abs(diff.dead)} dead code cleaned up`);
  }
  if (diff.cycles !== 0) {
    changes.push(diff.cycles > 0 ? `${diff.cycles} new cycles` : `${Math.abs(diff.cycles)} cycles resolved`);
  }
  
  if (changes.length === 0) {
    return '<span class="text-dim">No changes between snapshots</span>';
  }
  
  return changes.join(' | ');
}

/**
 * Close comparison modal.
 */
window.closeCompareModal = function() {
  const modal = document.getElementById('compare-modal');
  if (modal) {
    modal.classList.add('hidden');
  }
};

/**
 * Refresh timeline view.
 */
window.refreshTimeline = function() {
  renderTimeline({});
};
