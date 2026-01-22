/**
 * Export Component
 *
 * Data export functionality for symbols, files, cycles, and graphs.
 * Supports JSON, CSV, and PNG screenshot exports.
 *
 * @module components/export
 */

import { fetchStats, fetchList, fetchTree, fetchGraph } from '../api.js';

// =============================================================================
// CONSTANTS
// =============================================================================

const EXPORT_FORMATS = [
  { id: 'symbols-json', label: 'Symbols (JSON)', handler: exportSymbolsJSON },
  { id: 'symbols-csv', label: 'Symbols (CSV)', handler: exportSymbolsCSV },
  { id: 'files-json', label: 'Files (JSON)', handler: exportFilesJSON },
  { id: 'files-csv', label: 'Files (CSV)', handler: exportFilesCSV },
  { id: 'cycles-json', label: 'Cycles (JSON)', handler: exportCyclesJSON },
  { id: 'graph-json', label: 'Graph (JSON)', handler: exportGraphJSON },
  { id: 'full-json', label: 'Full Index (JSON)', handler: exportFullIndexJSON },
  { id: 'screenshot', label: 'Current View (PNG)', handler: exportScreenshot }
];

// =============================================================================
// HELPERS
// =============================================================================

/**
 * Download a blob as a file.
 * @param {Blob} blob - Blob to download
 * @param {string} filename - File name
 */
function downloadBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

/**
 * Generate timestamp for filename.
 * @returns {string} Timestamp string
 */
function getTimestamp() {
  const now = new Date();
  return now.toISOString().slice(0, 19).replace(/[T:]/g, '-');
}

/**
 * Get project name for filename.
 * @returns {string} Project name
 */
function getProjectName() {
  const projectEl = document.querySelector('.dropdown-value');
  return projectEl?.textContent?.trim() ?? 'greppy';
}

/**
 * Export data as JSON file.
 * @param {*} data - Data to export
 * @param {string} prefix - Filename prefix
 */
function exportJSON(data, prefix) {
  const json = JSON.stringify(data, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  const filename = `${prefix}_${getProjectName()}_${getTimestamp()}.json`;
  downloadBlob(blob, filename);
}

/**
 * Export data as CSV file.
 * @param {Array} data - Array of objects
 * @param {Array<string>} columns - Column names
 * @param {string} prefix - Filename prefix
 */
function exportCSV(data, columns, prefix) {
  // Escape CSV field
  const escape = (val) => {
    if (val === null || val === undefined) return '';
    const str = String(val);
    if (str.includes(',') || str.includes('"') || str.includes('\n')) {
      return `"${str.replace(/"/g, '""')}"`;
    }
    return str;
  };
  
  const header = columns.join(',');
  const rows = data.map(row => 
    columns.map(col => escape(row[col])).join(',')
  );
  const csv = [header, ...rows].join('\n');
  
  const blob = new Blob([csv], { type: 'text/csv' });
  const filename = `${prefix}_${getProjectName()}_${getTimestamp()}.csv`;
  downloadBlob(blob, filename);
}

/**
 * Show loading state on export button.
 * @param {boolean} loading - Loading state
 */
function setExportLoading(loading) {
  const btn = document.getElementById('export-btn');
  if (btn) {
    btn.classList.toggle('loading', loading);
    btn.disabled = loading;
  }
}

// =============================================================================
// EXPORT HANDLERS
// =============================================================================

/**
 * Export symbols as JSON.
 */
async function exportSymbolsJSON() {
  setExportLoading(true);
  try {
    const data = await fetchList({});
    exportJSON({
      exported_at: new Date().toISOString(),
      project: getProjectName(),
      total: data.items?.length ?? 0,
      symbols: data.items ?? []
    }, 'symbols');
  } catch (err) {
    console.error('Export failed:', err);
    alert('Failed to export symbols');
  } finally {
    setExportLoading(false);
  }
}

/**
 * Export symbols as CSV.
 */
async function exportSymbolsCSV() {
  setExportLoading(true);
  try {
    const data = await fetchList({});
    const columns = ['name', 'kind', 'file', 'line', 'refs', 'dead', 'in_cycle'];
    const items = (data.items ?? []).map(s => ({
      name: s.name,
      kind: s.kind,
      file: s.file,
      line: s.line,
      refs: s.refs ?? 0,
      dead: s.dead ? 'yes' : 'no',
      in_cycle: s.in_cycle ? 'yes' : 'no'
    }));
    exportCSV(items, columns, 'symbols');
  } catch (err) {
    console.error('Export failed:', err);
    alert('Failed to export symbols');
  } finally {
    setExportLoading(false);
  }
}

/**
 * Export files as JSON.
 */
async function exportFilesJSON() {
  setExportLoading(true);
  try {
    const data = await fetchTree();
    exportJSON({
      exported_at: new Date().toISOString(),
      project: getProjectName(),
      tree: data
    }, 'files');
  } catch (err) {
    console.error('Export failed:', err);
    alert('Failed to export files');
  } finally {
    setExportLoading(false);
  }
}

/**
 * Export files as CSV.
 */
async function exportFilesCSV() {
  setExportLoading(true);
  try {
    const data = await fetchTree();
    
    // Flatten tree to file list
    const files = [];
    function traverse(node, path = '') {
      const currentPath = path ? `${path}/${node.name}` : node.name;
      if (node.type === 'file') {
        files.push({
          path: currentPath,
          symbols: node.symbols ?? 0,
          dead: node.dead ?? 0,
          health: node.symbols > 0 
            ? Math.round(((node.symbols - (node.dead ?? 0)) / node.symbols) * 100) 
            : 100
        });
      }
      if (node.children) {
        node.children.forEach(child => traverse(child, currentPath));
      }
    }
    
    if (data.children) {
      data.children.forEach(child => traverse(child));
    }
    
    const columns = ['path', 'symbols', 'dead', 'health'];
    exportCSV(files, columns, 'files');
  } catch (err) {
    console.error('Export failed:', err);
    alert('Failed to export files');
  } finally {
    setExportLoading(false);
  }
}

/**
 * Export cycles as JSON.
 */
async function exportCyclesJSON() {
  setExportLoading(true);
  try {
    const stats = await fetchStats();
    exportJSON({
      exported_at: new Date().toISOString(),
      project: getProjectName(),
      total_cycles: stats.cycles ?? 0,
      cycles: stats.cycle_details ?? []
    }, 'cycles');
  } catch (err) {
    console.error('Export failed:', err);
    alert('Failed to export cycles');
  } finally {
    setExportLoading(false);
  }
}

/**
 * Export graph as JSON.
 */
async function exportGraphJSON() {
  setExportLoading(true);
  try {
    const data = await fetchGraph({});
    exportJSON({
      exported_at: new Date().toISOString(),
      project: getProjectName(),
      nodes: data.nodes?.length ?? 0,
      edges: data.edges?.length ?? 0,
      graph: data
    }, 'graph');
  } catch (err) {
    console.error('Export failed:', err);
    alert('Failed to export graph');
  } finally {
    setExportLoading(false);
  }
}

/**
 * Export full index as JSON.
 */
async function exportFullIndexJSON() {
  setExportLoading(true);
  try {
    const [stats, list, tree, graph] = await Promise.all([
      fetchStats(),
      fetchList({}),
      fetchTree(),
      fetchGraph({})
    ]);
    
    exportJSON({
      exported_at: new Date().toISOString(),
      project: getProjectName(),
      stats: stats,
      symbols: list.items ?? [],
      files: tree,
      graph: graph
    }, 'full-index');
  } catch (err) {
    console.error('Export failed:', err);
    alert('Failed to export full index');
  } finally {
    setExportLoading(false);
  }
}

/**
 * Export current view as PNG screenshot.
 */
async function exportScreenshot() {
  setExportLoading(true);
  try {
    // Try to capture the active view
    const activeView = document.querySelector('.view.active');
    if (!activeView) {
      alert('No active view to capture');
      return;
    }
    
    // Check if html2canvas is available, if not use a simple fallback
    if (typeof html2canvas !== 'undefined') {
      const canvas = await html2canvas(activeView, {
        backgroundColor: '#0a0a0a',
        scale: 2
      });
      canvas.toBlob((blob) => {
        if (blob) {
          const filename = `screenshot_${getProjectName()}_${getTimestamp()}.png`;
          downloadBlob(blob, filename);
        }
      }, 'image/png');
    } else {
      // Fallback: Export SVG if available (for graph view)
      const svg = activeView.querySelector('svg');
      if (svg) {
        const svgData = new XMLSerializer().serializeToString(svg);
        const blob = new Blob([svgData], { type: 'image/svg+xml' });
        const filename = `screenshot_${getProjectName()}_${getTimestamp()}.svg`;
        downloadBlob(blob, filename);
      } else {
        alert('Screenshot requires html2canvas library or an SVG element.\nTry using browser\'s built-in screenshot feature (Ctrl+Shift+S).');
      }
    }
  } catch (err) {
    console.error('Screenshot failed:', err);
    alert('Failed to capture screenshot');
  } finally {
    setExportLoading(false);
  }
}

// =============================================================================
// UI RENDERING
// =============================================================================

/**
 * Render export dropdown button.
 * @returns {string} HTML string
 */
function renderExportButton() {
  return `
    <div class="export-dropdown dropdown">
      <button id="export-btn" class="btn export-btn" type="button">
        <span class="export-icon">&#8681;</span>
        <span class="export-label">Export</span>
        <span class="dropdown-arrow">&#9660;</span>
      </button>
      <div class="dropdown-menu export-menu">
        ${EXPORT_FORMATS.map(fmt => `
          <div class="dropdown-item export-item" data-export="${fmt.id}" tabindex="0">
            ${fmt.label}
          </div>
        `).join('')}
      </div>
    </div>
  `;
}

/**
 * Setup export dropdown handlers.
 * @param {HTMLElement} container - Container element
 */
function setupExportHandlers(container) {
  const dropdown = container.querySelector('.export-dropdown');
  const btn = container.querySelector('#export-btn');
  const menu = container.querySelector('.export-menu');
  
  if (!dropdown || !btn || !menu) return;
  
  // Toggle dropdown
  btn.addEventListener('click', (e) => {
    e.stopPropagation();
    dropdown.classList.toggle('open');
  });
  
  // Handle export item clicks
  menu.addEventListener('click', async (e) => {
    const item = e.target.closest('.export-item');
    if (!item) return;
    
    dropdown.classList.remove('open');
    
    const exportId = item.dataset.export;
    const format = EXPORT_FORMATS.find(f => f.id === exportId);
    if (format?.handler) {
      await format.handler();
    }
  });
  
  // Keyboard navigation
  menu.addEventListener('keydown', async (e) => {
    if (e.key === 'Enter') {
      const item = e.target.closest('.export-item');
      if (item) {
        dropdown.classList.remove('open');
        const exportId = item.dataset.export;
        const format = EXPORT_FORMATS.find(f => f.id === exportId);
        if (format?.handler) {
          await format.handler();
        }
      }
    }
  });
  
  // Close on outside click
  document.addEventListener('click', () => {
    dropdown.classList.remove('open');
  });
}

// =============================================================================
// INITIALIZATION
// =============================================================================

/**
 * Initialize export component.
 * Injects export button into toolbar.
 */
export function initExport() {
  // Find toolbar right section
  const toolbarRight = document.querySelector('.toolbar-right');
  if (!toolbarRight) {
    console.warn('Export: toolbar-right not found');
    return;
  }
  
  // Create export container
  const exportContainer = document.createElement('div');
  exportContainer.className = 'export-container';
  exportContainer.innerHTML = renderExportButton();
  
  // Insert before the search input
  const searchInput = toolbarRight.querySelector('.input');
  if (searchInput) {
    toolbarRight.insertBefore(exportContainer, searchInput);
  } else {
    toolbarRight.appendChild(exportContainer);
  }
  
  // Setup handlers
  setupExportHandlers(exportContainer);
}

// Export individual functions for programmatic use
export {
  exportSymbolsJSON,
  exportSymbolsCSV,
  exportFilesJSON,
  exportFilesCSV,
  exportCyclesJSON,
  exportGraphJSON,
  exportFullIndexJSON,
  exportScreenshot
};
