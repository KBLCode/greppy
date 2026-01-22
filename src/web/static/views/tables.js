/**
 * Tables View Module
 *
 * Renders tabbed data tables for symbols, files, cycles, entry points, and connections.
 * Pure data display - no opinions, no risk scores, just sortable raw data.
 *
 * @module views/tables
 */

import { fetchList, fetchStats, fetchTree } from '../api.js';
import { escapeHtml, truncatePath } from '../utils.js';
import { selectSymbol } from '../components/detail.js';
import { renderTablesSkeleton } from '../components/skeleton.js';
import { emptyNoSymbols, emptyNoResults } from '../components/empty.js';
import { errorLoadFailed } from '../components/error.js';

// =============================================================================
// STATE
// =============================================================================

/** @type {'symbols' | 'files' | 'cycles' | 'entries' | 'connections'} */
let activeTab = 'symbols';

/** @type {{ column: string, direction: 'asc' | 'desc' }} */
let sortState = { column: 'name', direction: 'asc' };

/** @type {Object|null} */
let cachedData = null;

// =============================================================================
// CONSTANTS
// =============================================================================

const TABS = [
  { id: 'symbols', label: 'Symbols' },
  { id: 'files', label: 'Files' },
  { id: 'cycles', label: 'Cycles' },
  { id: 'entries', label: 'Entry Points' },
  { id: 'connections', label: 'Connections' }
];

const SYMBOLS_COLUMNS = [
  { key: 'name', label: 'NAME', sortable: true },
  { key: 'kind', label: 'KIND', sortable: true },
  { key: 'file', label: 'FILE', sortable: true },
  { key: 'line', label: 'LINE', sortable: true, numeric: true },
  { key: 'refs', label: 'REFS', sortable: true, numeric: true },
  { key: 'callers', label: 'CALLERS', sortable: true, numeric: true },
  { key: 'callees', label: 'CALLEES', sortable: true, numeric: true },
  { key: 'isEntry', label: 'ENTRY?', sortable: true },
  { key: 'isDead', label: 'DEAD?', sortable: true },
  { key: 'inCycle', label: 'IN CYCLE?', sortable: true }
];

const FILES_COLUMNS = [
  { key: 'file', label: 'FILE', sortable: true },
  { key: 'symbols', label: 'SYMBOLS', sortable: true, numeric: true },
  { key: 'refsIn', label: 'REFS IN', sortable: true, numeric: true },
  { key: 'refsOut', label: 'REFS OUT', sortable: true, numeric: true },
  { key: 'entryPoints', label: 'ENTRY PTS', sortable: true, numeric: true },
  { key: 'deadSymbols', label: 'DEAD', sortable: true, numeric: true },
  { key: 'inCycles', label: 'IN CYCLES', sortable: true, numeric: true }
];

const CYCLES_COLUMNS = [
  { key: 'id', label: 'CYCLE #', sortable: true, numeric: true },
  { key: 'size', label: 'SIZE', sortable: true, numeric: true },
  { key: 'symbols', label: 'SYMBOLS', sortable: false },
  { key: 'path', label: 'PATH', sortable: false }
];

const ENTRIES_COLUMNS = [
  { key: 'name', label: 'ENTRY POINT', sortable: true },
  { key: 'file', label: 'FILE', sortable: true },
  { key: 'line', label: 'LINE', sortable: true, numeric: true },
  { key: 'callees', label: 'CALLEES', sortable: true, numeric: true },
  { key: 'reachable', label: 'REACHABLE', sortable: true, numeric: true }
];

const CONNECTIONS_COLUMNS = [
  { key: 'from', label: 'FROM', sortable: true },
  { key: 'to', label: 'TO', sortable: true },
  { key: 'kind', label: 'KIND', sortable: true },
  { key: 'count', label: 'COUNT', sortable: true, numeric: true },
  { key: 'files', label: 'FILES', sortable: true, numeric: true }
];

// =============================================================================
// MAIN RENDER
// =============================================================================

/**
 * Render the tables view with tabs and data table.
 * @param {Object} state - App state
 */
export async function renderTables(state) {
  const container = document.getElementById('tables-container');
  if (!container) return;
  
  container.innerHTML = `
    <div class="tables-view">
      <div class="tables-tabs" role="tablist">
        ${TABS.map(tab => `
          <button class="tables-tab ${activeTab === tab.id ? 'active' : ''}" 
                  data-tab="${tab.id}"
                  role="tab"
                  aria-selected="${activeTab === tab.id}"
                  tabindex="${activeTab === tab.id ? '0' : '-1'}">
            ${tab.label}
          </button>
        `).join('')}
      </div>
      <div class="tables-content" id="tables-content">
        ${renderTablesSkeleton(12)}
      </div>
    </div>
  `;
  
  // Attach tab listeners
  container.querySelectorAll('.tables-tab').forEach(tab => {
    tab.addEventListener('click', () => {
      activeTab = tab.dataset.tab;
      sortState = { column: getDefaultSortColumn(activeTab), direction: 'asc' };
      renderTables(state);
    });
  });
  
  // Load and render data
  await loadAndRenderTable(state);
}

/**
 * Get default sort column for a tab.
 * @param {string} tab - Tab ID
 * @returns {string} Default column key
 */
function getDefaultSortColumn(tab) {
  switch (tab) {
    case 'symbols': return 'name';
    case 'files': return 'file';
    case 'cycles': return 'id';
    case 'entries': return 'name';
    case 'connections': return 'count';
    default: return 'name';
  }
}

// =============================================================================
// DATA LOADING
// =============================================================================

/**
 * Load data and render the active table.
 * @param {Object} state - App state
 */
async function loadAndRenderTable(state) {
  const content = document.getElementById('tables-content');
  if (!content) return;
  
  try {
    // Fetch all data we need
    const [listData, statsData, treeData, cyclesData] = await Promise.all([
      fetchList(state.filters),
      fetchStats(),
      fetchTree(),
      fetchCycles()
    ]);
    
    cachedData = {
      list: listData,
      stats: statsData,
      tree: treeData,
      cycles: cyclesData
    };
    
    renderActiveTable(content, state);
    
  } catch (err) {
    console.error('Failed to load tables data:', err);
    content.innerHTML = errorLoadFailed('table data', 'window.refreshTables');
  }
}

/**
 * Fetch cycles data from API.
 * @returns {Promise<Object>} Cycles response
 */
async function fetchCycles() {
  const res = await fetch('/api/cycles');
  return res.json();
}

// =============================================================================
// TABLE RENDERING
// =============================================================================

/**
 * Render the active table based on current tab.
 * @param {HTMLElement} content - Content container
 * @param {Object} state - App state
 */
function renderActiveTable(content, state) {
  if (!cachedData) {
    content.innerHTML = '<div class="tables-error">No data loaded</div>';
    return;
  }
  
  switch (activeTab) {
    case 'symbols':
      renderSymbolsTable(content, state);
      break;
    case 'files':
      renderFilesTable(content, state);
      break;
    case 'cycles':
      renderCyclesTable(content, state);
      break;
    case 'entries':
      renderEntriesTable(content, state);
      break;
    case 'connections':
      renderConnectionsTable(content, state);
      break;
  }
}

// =============================================================================
// SYMBOLS TABLE
// =============================================================================

/**
 * Render symbols table.
 * @param {HTMLElement} content - Content container
 * @param {Object} state - App state
 */
function renderSymbolsTable(content, state) {
  const items = cachedData.list?.items || [];
  
  if (!items.length) {
    content.innerHTML = '<div class="tables-empty">No symbols found</div>';
    return;
  }
  
  // Transform to table format
  const rows = items.map(item => ({
    id: item.id,
    name: item.name,
    kind: item.type || 'unknown',
    file: item.path || '',
    line: item.line || 0,
    refs: item.refs || 0,
    callers: item.callers || 0,
    callees: item.callees || 0,
    isEntry: item.isEntry ? 'Yes' : 'No',
    isDead: item.state === 'dead' ? 'Yes' : 'No',
    inCycle: item.state === 'cycle' ? 'Yes' : 'No',
    _raw: item
  }));
  
  // Sort
  const sorted = sortData(rows, SYMBOLS_COLUMNS);
  
  content.innerHTML = renderTable(SYMBOLS_COLUMNS, sorted, 'symbols');
  attachTableListeners(content, state, sorted, 'symbols');
}

// =============================================================================
// FILES TABLE
// =============================================================================

/**
 * Render files table with aggregated data.
 * @param {HTMLElement} content - Content container
 * @param {Object} state - App state
 */
function renderFilesTable(content, state) {
  const items = cachedData.list?.items || [];
  const cycles = cachedData.cycles?.cycles || [];
  
  // Aggregate by file
  const fileMap = new Map();
  
  items.forEach(item => {
    const path = item.path || 'unknown';
    if (!fileMap.has(path)) {
      fileMap.set(path, {
        file: path,
        symbols: 0,
        refsIn: 0,
        refsOut: 0,
        entryPoints: 0,
        deadSymbols: 0,
        inCycles: 0,
        _symbols: []
      });
    }
    
    const file = fileMap.get(path);
    file.symbols++;
    file.refsIn += item.refs || 0;
    file.refsOut += item.callees || 0;
    if (item.isEntry) file.entryPoints++;
    if (item.state === 'dead') file.deadSymbols++;
    file._symbols.push(item);
  });
  
  // Count symbols in cycles per file
  cycles.forEach(cycle => {
    cycle.symbols?.forEach(sym => {
      const file = fileMap.get(sym.file);
      if (file) file.inCycles++;
    });
  });
  
  const rows = Array.from(fileMap.values());
  
  if (!rows.length) {
    content.innerHTML = '<div class="tables-empty">No files found</div>';
    return;
  }
  
  // Sort
  const sorted = sortData(rows, FILES_COLUMNS);
  
  content.innerHTML = renderTable(FILES_COLUMNS, sorted, 'files');
  attachTableListeners(content, state, sorted, 'files');
}

// =============================================================================
// CYCLES TABLE
// =============================================================================

/**
 * Render cycles table.
 * @param {HTMLElement} content - Content container
 * @param {Object} state - App state
 */
function renderCyclesTable(content, state) {
  const cycles = cachedData.cycles?.cycles || [];
  
  if (!cycles.length) {
    content.innerHTML = `
      <div class="tables-empty tables-empty-good">
        <span class="tables-empty-icon">&#10003;</span>
        No circular dependencies detected
      </div>
    `;
    return;
  }
  
  // Transform to table format
  const rows = cycles.map((cycle, idx) => ({
    id: cycle.id || idx + 1,
    size: cycle.size || cycle.symbols?.length || 0,
    symbols: (cycle.symbols || []).map(s => s.name).join(', '),
    path: (cycle.path || []).join(' -> '),
    _raw: cycle
  }));
  
  // Sort
  const sorted = sortData(rows, CYCLES_COLUMNS);
  
  content.innerHTML = renderTable(CYCLES_COLUMNS, sorted, 'cycles');
  attachTableListeners(content, state, sorted, 'cycles');
}

// =============================================================================
// ENTRY POINTS TABLE
// =============================================================================

/**
 * Render entry points table.
 * @param {HTMLElement} content - Content container
 * @param {Object} state - App state
 */
function renderEntriesTable(content, state) {
  const items = cachedData.list?.items || [];
  
  // Filter to entry points (symbols with 0 callers or marked as entry)
  const entries = items.filter(item => item.isEntry || (item.callers === 0 && item.refs === 0));
  
  if (!entries.length) {
    content.innerHTML = '<div class="tables-empty">No entry points detected</div>';
    return;
  }
  
  // Transform to table format
  const rows = entries.map(item => ({
    id: item.id,
    name: item.name,
    file: item.path || '',
    line: item.line || 0,
    callees: item.callees || 0,
    reachable: item.reachable || item.callees || 0,
    _raw: item
  }));
  
  // Sort
  const sorted = sortData(rows, ENTRIES_COLUMNS);
  
  content.innerHTML = renderTable(ENTRIES_COLUMNS, sorted, 'entries');
  attachTableListeners(content, state, sorted, 'entries');
}

// =============================================================================
// CONNECTIONS TABLE
// =============================================================================

/**
 * Render connections table.
 * @param {HTMLElement} content - Content container
 * @param {Object} state - App state
 */
function renderConnectionsTable(content, state) {
  const items = cachedData.list?.items || [];
  
  // Build connection map (from -> to with counts)
  const connectionMap = new Map();
  
  items.forEach(item => {
    if (!item.edges) return;
    
    item.edges?.forEach(edge => {
      const key = `${item.name}::${edge.to}::${edge.kind || 'call'}`;
      if (!connectionMap.has(key)) {
        connectionMap.set(key, {
          from: item.name,
          to: edge.to || 'unknown',
          kind: edge.kind || 'call',
          count: 0,
          files: new Set()
        });
      }
      const conn = connectionMap.get(key);
      conn.count++;
      conn.files.add(item.path);
    });
  });
  
  // If no explicit edges, create synthetic connections from refs
  if (connectionMap.size === 0) {
    // Group by caller/callee relationships
    const callerMap = new Map();
    
    items.forEach(item => {
      if (item.callers > 0 || item.callees > 0) {
        const key = `${item.name}`;
        if (!callerMap.has(key)) {
          callerMap.set(key, {
            from: item.name,
            to: '(various)',
            kind: 'reference',
            count: (item.callers || 0) + (item.callees || 0),
            files: new Set([item.path])
          });
        }
      }
    });
    
    callerMap.forEach((v, k) => connectionMap.set(k, v));
  }
  
  const rows = Array.from(connectionMap.values()).map(conn => ({
    from: conn.from,
    to: conn.to,
    kind: conn.kind,
    count: conn.count,
    files: conn.files.size
  }));
  
  if (!rows.length) {
    content.innerHTML = '<div class="tables-empty">No connections found</div>';
    return;
  }
  
  // Sort (default by count descending for connections)
  if (sortState.column === 'count' && sortState.direction === 'asc') {
    sortState.direction = 'desc';
  }
  const sorted = sortData(rows, CONNECTIONS_COLUMNS);
  
  content.innerHTML = renderTable(CONNECTIONS_COLUMNS, sorted, 'connections');
  attachTableListeners(content, state, sorted, 'connections');
}

// =============================================================================
// TABLE RENDERING HELPERS
// =============================================================================

/**
 * Render a generic data table.
 * @param {Array} columns - Column definitions
 * @param {Array} rows - Data rows
 * @param {string} tableId - Table identifier
 * @returns {string} HTML string
 */
function renderTable(columns, rows, tableId) {
  return `
    <div class="tables-table-wrapper">
      <table class="tables-table" role="grid" aria-label="${tableId} table" data-table="${tableId}">
        <thead>
          <tr role="row">
            ${columns.map(col => renderColumnHeader(col)).join('')}
          </tr>
        </thead>
        <tbody>
          ${rows.map((row, idx) => renderTableRow(columns, row, idx, tableId)).join('')}
        </tbody>
      </table>
    </div>
    <div class="tables-footer">
      <span class="tables-count">${rows.length} ${tableId}</span>
    </div>
  `;
}

/**
 * Render a column header with sort indicator.
 * @param {Object} col - Column definition
 * @returns {string} HTML string
 */
function renderColumnHeader(col) {
  const isSorted = sortState.column === col.key;
  const sortIcon = isSorted 
    ? (sortState.direction === 'asc' ? '▲' : '▼') 
    : '';
  const ariaSort = isSorted 
    ? (sortState.direction === 'asc' ? 'ascending' : 'descending')
    : 'none';
  
  if (!col.sortable) {
    return `<th scope="col" class="tables-th">${col.label}</th>`;
  }
  
  return `
    <th scope="col" 
        class="tables-th sortable ${isSorted ? 'sorted' : ''}"
        data-column="${col.key}"
        data-numeric="${col.numeric || false}"
        role="columnheader"
        aria-sort="${ariaSort}"
        tabindex="0">
      <span class="th-content">
        <span class="th-label">${col.label}</span>
        <span class="sort-icon" aria-hidden="true">${sortIcon}</span>
      </span>
    </th>
  `;
}

/**
 * Render a table row.
 * @param {Array} columns - Column definitions
 * @param {Object} row - Row data
 * @param {number} idx - Row index
 * @param {string} tableId - Table identifier
 * @returns {string} HTML string
 */
function renderTableRow(columns, row, idx, tableId) {
  const rowClass = [
    'tables-row',
    idx % 2 === 1 ? 'zebra' : '',
    row.isDead === 'Yes' ? 'dead' : '',
    row.inCycle === 'Yes' ? 'cycle' : ''
  ].filter(Boolean).join(' ');
  
  return `
    <tr class="${rowClass}" 
        data-id="${row.id || idx}"
        data-table="${tableId}"
        role="row"
        tabindex="0">
      ${columns.map(col => renderTableCell(col, row)).join('')}
    </tr>
  `;
}

/**
 * Render a table cell.
 * @param {Object} col - Column definition
 * @param {Object} row - Row data
 * @returns {string} HTML string
 */
function renderTableCell(col, row) {
  let value = row[col.key];
  let displayValue = value;
  let cellClass = `tables-td tables-td-${col.key}`;
  
  // Format display value
  if (value === undefined || value === null) {
    displayValue = '-';
  } else if (col.key === 'file') {
    displayValue = `<span class="cell-path" title="${escapeHtml(value)}">${escapeHtml(truncatePath(value))}</span>`;
  } else if (col.key === 'name' || col.key === 'from' || col.key === 'to') {
    displayValue = `<span class="cell-name">${escapeHtml(String(value))}</span>`;
  } else if (col.key === 'symbols' || col.key === 'path') {
    // Truncate long lists
    const str = String(value);
    displayValue = str.length > 60 
      ? `<span title="${escapeHtml(str)}">${escapeHtml(str.slice(0, 60))}...</span>`
      : escapeHtml(str);
  } else if (col.key === 'isDead' || col.key === 'inCycle' || col.key === 'isEntry') {
    cellClass += value === 'Yes' ? ' cell-yes' : ' cell-no';
    displayValue = value;
  } else if (col.numeric) {
    cellClass += ' cell-numeric';
    displayValue = typeof value === 'number' ? value.toLocaleString() : value;
  } else {
    displayValue = escapeHtml(String(value));
  }
  
  return `<td class="${cellClass}">${displayValue}</td>`;
}

// =============================================================================
// SORTING
// =============================================================================

/**
 * Sort data by current sort state.
 * @param {Array} data - Data rows
 * @param {Array} columns - Column definitions
 * @returns {Array} Sorted data
 */
function sortData(data, columns) {
  const col = columns.find(c => c.key === sortState.column);
  if (!col || !col.sortable) return data;
  
  const sorted = [...data].sort((a, b) => {
    let aVal = a[sortState.column];
    let bVal = b[sortState.column];
    
    // Handle nullish
    if (aVal == null) aVal = col.numeric ? 0 : '';
    if (bVal == null) bVal = col.numeric ? 0 : '';
    
    // Numeric comparison
    if (col.numeric) {
      aVal = Number(aVal) || 0;
      bVal = Number(bVal) || 0;
      return aVal - bVal;
    }
    
    // String comparison
    return String(aVal).localeCompare(String(bVal));
  });
  
  return sortState.direction === 'desc' ? sorted.reverse() : sorted;
}

// =============================================================================
// EVENT LISTENERS
// =============================================================================

/**
 * Attach event listeners to table.
 * @param {HTMLElement} content - Content container
 * @param {Object} state - App state
 * @param {Array} rows - Data rows
 * @param {string} tableId - Table identifier
 */
function attachTableListeners(content, state, rows, tableId) {
  // Header sort listeners
  content.querySelectorAll('.tables-th.sortable').forEach(th => {
    const handleSort = () => {
      const column = th.dataset.column;
      
      if (sortState.column === column) {
        sortState.direction = sortState.direction === 'asc' ? 'desc' : 'asc';
      } else {
        sortState.column = column;
        sortState.direction = 'asc';
      }
      
      renderActiveTable(content, state);
    };
    
    th.addEventListener('click', handleSort);
    th.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleSort();
      }
    });
  });
  
  // Row click listeners
  content.querySelectorAll('.tables-row').forEach(tr => {
    const handleSelect = () => {
      const idx = parseInt(tr.dataset.id, 10);
      const row = rows.find((r, i) => (r.id || i) === idx);
      
      if (row && row._raw) {
        selectSymbol(row._raw, state);
      }
      
      // Update selection visual
      content.querySelectorAll('.tables-row').forEach(r => {
        r.classList.remove('selected');
        r.setAttribute('aria-selected', 'false');
      });
      tr.classList.add('selected');
      tr.setAttribute('aria-selected', 'true');
    };
    
    tr.addEventListener('click', handleSelect);
    tr.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleSelect();
      }
    });
  });
}
