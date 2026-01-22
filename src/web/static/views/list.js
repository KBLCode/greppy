/**
 * List View Module
 *
 * Renders the symbol list as a proper data table with sorting and filtering.
 *
 * @module views/list
 */

import { fetchList } from '../api.js';
import { escapeHtml, truncatePath } from '../utils.js';
import { selectSymbol } from '../components/detail.js';
import { updateNestedState } from '../lib/persistence.js';
import { renderListSkeleton } from '../components/skeleton.js';
import { emptyNoResults, emptyNoSymbols } from '../components/empty.js';
import { errorListLoad } from '../components/error.js';

// =============================================================================
// STATE
// =============================================================================

/** @type {{ column: string, direction: 'asc' | 'desc' }} */
let sortState = { column: 'name', direction: 'asc' };

// =============================================================================
// SORT STATE ACCESSORS
// =============================================================================

/**
 * Set sort state (used to restore from persistence).
 * @param {{ column: string, direction: 'asc' | 'desc' }} newState
 */
export function setSortState(newState) {
  if (newState && newState.column) {
    sortState.column = newState.column;
    sortState.direction = newState.direction || 'asc';
  }
}

/**
 * Get current sort state.
 * @returns {{ column: string, direction: 'asc' | 'desc' }}
 */
export function getSortState() {
  return { ...sortState };
}

// =============================================================================
// CONSTANTS
// =============================================================================

const COLUMNS = [
  { key: 'name', label: 'NAME', sortable: true },
  { key: 'type', label: 'KIND', sortable: true },
  { key: 'path', label: 'FILE', sortable: true },
  { key: 'line', label: 'LINE', sortable: true },
  { key: 'refs', label: 'REFS', sortable: true },
  { key: 'state', label: 'STATUS', sortable: true }
];

// =============================================================================
// LOADING STATE
// =============================================================================

/**
 * Show loading skeleton for list view.
 */
export function showListLoading() {
  const container = document.getElementById('list-container');
  if (container) {
    container.innerHTML = renderListSkeleton(20);
  }
}

/**
 * Show error state for list view.
 */
export function showListError() {
  const container = document.getElementById('list-container');
  if (container) {
    container.innerHTML = errorListLoad('window.refreshList');
  }
}

// =============================================================================
// MAIN RENDER
// =============================================================================

/**
 * Render the list view as a proper data table.
 * @param {Object} state - App state with filters
 */
export async function renderList(state) {
  const container = document.getElementById('list-container');
  if (!container) return;
  
  // Show loading skeleton
  container.innerHTML = renderListSkeleton(15);
  
  try {
    state.list = await fetchList(state.filters);
    
    // Check for no data
    if (!state.list?.items?.length) {
      // Distinguish between no symbols at all vs no results from filter
      const hasFilters = state.filters?.search || state.filters?.type !== 'all' || state.filters?.state !== 'all';
      container.innerHTML = hasFilters ? emptyNoResults('window.clearFilters') : emptyNoSymbols();
      return;
    }
    
    let items = [...state.list.items];
    
    // Sort items
    items = sortItems(items, sortState.column, sortState.direction);
    
    const maxRefs = Math.max(...items.map(i => i.refs || 0), 1);
    
    container.innerHTML = `
      <div class="list-table-wrapper">
        <table class="list-table" role="grid" aria-label="Symbol list">
          <thead>
            <tr role="row">
              ${COLUMNS.map(col => renderColumnHeader(col)).join('')}
            </tr>
          </thead>
          <tbody>
            ${items.map((item, idx) => renderRow(item, idx, maxRefs)).join('')}
          </tbody>
        </table>
      </div>
    `;
    
    // Attach event listeners
    attachHeaderListeners(container, state, items);
    attachRowListeners(container, state, items);
    
  } catch (err) {
    console.error('Failed to load list:', err);
    container.innerHTML = errorListLoad('window.refreshList');
  }
}

// =============================================================================
// COLUMN HEADER
// =============================================================================

/**
 * Render a column header with sort indicator.
 * @param {Object} col - Column definition
 * @returns {string} HTML for th element
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
    return `<th scope="col" class="list-th list-th-${col.key}">${col.label}</th>`;
  }
  
  return `
    <th scope="col" 
        class="list-th list-th-${col.key} sortable ${isSorted ? 'sorted' : ''}"
        data-column="${col.key}"
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

// =============================================================================
// ROW RENDERING
// =============================================================================

/**
 * Render a table row for a symbol.
 * @param {Object} item - Symbol data
 * @param {number} idx - Row index for zebra striping
 * @param {number} maxRefs - Maximum refs for bar scaling
 * @returns {string} HTML for tr element
 */
function renderRow(item, idx, maxRefs) {
  const isDead = item.state === 'dead';
  const isCycle = item.state === 'cycle';
  const rowClass = [
    'list-row',
    isDead ? 'dead' : '',
    isCycle ? 'cycle' : '',
    idx % 2 === 1 ? 'zebra' : ''
  ].filter(Boolean).join(' ');
  
  const statusBadge = getStatusBadge(item.state);
  const refBarWidth = maxRefs > 0 ? ((item.refs || 0) / maxRefs) * 100 : 0;
  const refBarBlocks = getRefBarBlocks(item.refs || 0, maxRefs);
  
  return `
    <tr class="${rowClass}" 
        data-id="${item.id}" 
        role="row"
        tabindex="0"
        aria-label="${escapeHtml(item.name)}, ${item.type}, ${item.refs || 0} references, ${item.state || 'used'}">
      <td class="list-td list-td-name">
        <span class="symbol-name">${escapeHtml(item.name)}</span>
      </td>
      <td class="list-td list-td-type">
        <span class="symbol-type-badge">${escapeHtml(item.type)}</span>
      </td>
      <td class="list-td list-td-path" title="${escapeHtml(item.path)}">
        <span class="symbol-path">${escapeHtml(truncatePath(item.path))}</span>
      </td>
      <td class="list-td list-td-line">
        <span class="symbol-line">${item.line}</span>
      </td>
      <td class="list-td list-td-refs">
        <div class="refs-cell">
          <span class="ref-bar-visual" aria-hidden="true">${refBarBlocks}</span>
          <span class="ref-count">${item.refs || 0}</span>
        </div>
      </td>
      <td class="list-td list-td-status">
        ${statusBadge}
      </td>
    </tr>
  `;
}

/**
 * Get status badge HTML.
 * @param {string} state - Symbol state
 * @returns {string} HTML for status badge
 */
function getStatusBadge(state) {
  if (state === 'dead') {
    return '<span class="status-badge status-dead" aria-label="dead code">○ dead</span>';
  }
  if (state === 'cycle') {
    return '<span class="status-badge status-cycle" aria-label="in cycle">◐ cycle</span>';
  }
  return '<span class="status-badge status-used" aria-label="used">● used</span>';
}

/**
 * Generate visual ref bar using block characters.
 * @param {number} refs - Reference count
 * @param {number} maxRefs - Maximum refs
 * @returns {string} Block characters representing refs
 */
function getRefBarBlocks(refs, maxRefs) {
  if (maxRefs === 0 || refs === 0) return '';
  const ratio = refs / maxRefs;
  const blocks = Math.ceil(ratio * 5);
  return '█'.repeat(blocks);
}

// =============================================================================
// SORTING
// =============================================================================

/**
 * Sort items by column and direction.
 * @param {Array} items - Symbol items
 * @param {string} column - Column key
 * @param {'asc' | 'desc'} direction - Sort direction
 * @returns {Array} Sorted items
 */
function sortItems(items, column, direction) {
  const sorted = [...items].sort((a, b) => {
    let aVal = a[column];
    let bVal = b[column];
    
    // Handle nullish values
    if (aVal == null) aVal = '';
    if (bVal == null) bVal = '';
    
    // Numeric comparison for refs and line
    if (column === 'refs' || column === 'line') {
      aVal = Number(aVal) || 0;
      bVal = Number(bVal) || 0;
      return aVal - bVal;
    }
    
    // String comparison
    return String(aVal).localeCompare(String(bVal));
  });
  
  return direction === 'desc' ? sorted.reverse() : sorted;
}

// =============================================================================
// EVENT LISTENERS
// =============================================================================

/**
 * Attach click listeners to column headers for sorting.
 * @param {HTMLElement} container - Container element
 * @param {Object} state - App state
 * @param {Array} items - Current items (for re-render reference)
 */
function attachHeaderListeners(container, state, items) {
  container.querySelectorAll('.list-th.sortable').forEach(th => {
    const handleSort = () => {
      const column = th.dataset.column;
      
      // Toggle direction if same column, otherwise default to asc
      if (sortState.column === column) {
        sortState.direction = sortState.direction === 'asc' ? 'desc' : 'asc';
      } else {
        sortState.column = column;
        sortState.direction = 'asc';
      }
      
      // Persist sort state
      updateNestedState('sortState.list', { column: sortState.column, direction: sortState.direction });
      
      renderList(state);
    };
    
    th.addEventListener('click', handleSort);
    th.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleSort();
      }
    });
  });
}

/**
 * Attach click listeners to rows for selection.
 * @param {HTMLElement} container - Container element
 * @param {Object} state - App state
 * @param {Array} items - Symbol items
 */
function attachRowListeners(container, state, items) {
  container.querySelectorAll('.list-row').forEach(tr => {
    const handleSelect = () => {
      const item = items.find(i => String(i.id) === tr.dataset.id);
      if (item) {
        selectSymbol(item, state);
      }
      
      // Update selection visual
      container.querySelectorAll('.list-row').forEach(row => {
        row.classList.remove('selected');
        row.setAttribute('aria-selected', 'false');
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
