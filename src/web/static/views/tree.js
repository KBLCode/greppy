/**
 * Tree View Module
 *
 * Renders the file tree browser with symbol details.
 *
 * @module views/tree
 */

import { fetchTree, fetchFileSymbols } from '../api.js';
import { escapeHtml } from '../utils.js';
import { selectSymbol } from '../components/detail.js';

// =============================================================================
// MAIN RENDER
// =============================================================================

/**
 * Render the tree view.
 * @param {Object} state - App state
 */
export async function renderTreeView(state) {
  const sidebar = document.getElementById('tree-content');
  const mainContent = document.getElementById('tree-main-content');
  const fileCountEl = document.getElementById('tree-file-count');
  
  if (!sidebar) return;
  
  sidebar.innerHTML = '<div class="loading">loading</div>';
  
  try {
    const data = await fetchTree();
    // API returns { root: TreeNode } - extract the root node
    state.tree = data.root || data;
    
    if (!state.tree || !state.tree.children || state.tree.children.length === 0) {
      sidebar.innerHTML = '<div class="tree-empty">No files found</div>';
      return;
    }
    
    // Count total files
    const fileCount = countTreeFiles(state.tree);
    if (fileCountEl) fileCountEl.textContent = fileCount;
    
    // Render the tree
    sidebar.innerHTML = '';
    renderTreeViewBranch(sidebar, state.tree.children, 0, state);
    
    // Setup keyboard navigation
    setupTreeViewKeyboardNav();
    
  } catch (err) {
    sidebar.innerHTML = `<div class="tree-empty error">Failed to load: ${err.message}</div>`;
  }
}

// =============================================================================
// TREE RENDERING
// =============================================================================

/**
 * Render a branch of the tree.
 * @param {HTMLElement} container - Container element
 * @param {Array} children - Child nodes
 * @param {number} depth - Current depth
 * @param {Object} state - App state
 */
function renderTreeViewBranch(container, children, depth, state) {
  if (!children) return;
  
  children.forEach(child => {
    const el = document.createElement('div');
    el.className = 'tree-node';
    el.dataset.path = child.path || child.name;
    el.dataset.type = child.type;
    
    const isDir = child.type === 'dir';
    const hasDead = (child.dead || 0) > 0;
    const hasCycle = child.cycle === true;
    
    // Calculate health percentage for files
    const healthPct = child.symbols > 0 
      ? Math.round(((child.symbols - (child.dead || 0)) / child.symbols) * 100) 
      : 100;
    
    if (isDir) {
      // Directory row
      const childCount = countTreeFiles(child);
      const deadCount = countTreeDead(child);
      const symbolCount = countTreeSymbols(child);
      
      el.innerHTML = `
        <div class="tree-row tree-row-dir ${hasCycle ? 'has-cycle' : ''}" style="padding-left: ${depth * 16 + 8}px" tabindex="0">
          <span class="tree-toggle">&#9654;</span>
          <span class="tree-name">${escapeHtml(child.name)}</span>
          <span class="tree-meta">
            <span class="tree-file-count" title="${childCount} files">${childCount}</span>
            <span class="tree-symbol-count" title="${symbolCount} symbols">${symbolCount}</span>
            ${deadCount > 0 ? `<span class="tree-dead-count" title="${deadCount} dead">${deadCount}</span>` : ''}
            ${hasCycle ? '<span class="tree-cycle-indicator" title="Contains cycle">&#x21bb;</span>' : ''}
          </span>
        </div>
        <div class="tree-children collapsed"></div>
      `;
      
      const row = el.querySelector('.tree-row');
      
      row.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleTreeViewDir(el, child, depth, state);
      });
      
      row.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          toggleTreeViewDir(el, child, depth, state);
        }
      });
      
    } else {
      // File row
      el.innerHTML = `
        <div class="tree-row tree-row-file ${hasDead ? 'has-dead' : ''}" style="padding-left: ${depth * 16 + 24}px" tabindex="0">
          <span class="tree-name">${escapeHtml(child.name)}</span>
          <span class="tree-meta">
            <span class="tree-symbol-count" title="${child.symbols || 0} symbols">${child.symbols || 0}</span>
            ${hasDead ? `<span class="tree-dead-count" title="${child.dead} dead">${child.dead}</span>` : ''}
            <span class="tree-health-bar" title="${healthPct}% healthy">
              <span class="tree-health-fill" style="width: ${healthPct}%"></span>
            </span>
          </span>
        </div>
      `;
      
      const row = el.querySelector('.tree-row');
      
      row.addEventListener('click', (e) => {
        e.stopPropagation();
        selectTreeViewFile(child, state);
        document.querySelectorAll('.tree-row-file').forEach(r => r.classList.remove('selected'));
        row.classList.add('selected');
      });
      
      row.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          selectTreeViewFile(child, state);
          document.querySelectorAll('.tree-row-file').forEach(r => r.classList.remove('selected'));
          row.classList.add('selected');
        }
      });
    }
    
    container.appendChild(el);
  });
}

/**
 * Toggle directory expand/collapse.
 * @param {HTMLElement} el - Directory element
 * @param {Object} child - Child data
 * @param {number} depth - Current depth
 * @param {Object} state - App state
 */
function toggleTreeViewDir(el, child, depth, state) {
  const toggle = el.querySelector('.tree-toggle');
  const childrenContainer = el.querySelector('.tree-children');
  const isCollapsed = childrenContainer.classList.contains('collapsed');
  
  if (isCollapsed) {
    toggle.textContent = '\u25BC';
    toggle.classList.add('expanded');
    childrenContainer.classList.remove('collapsed');
    
    // Lazy load children if not already loaded
    if (!childrenContainer.hasChildNodes() && child.children) {
      renderTreeViewBranch(childrenContainer, child.children, depth + 1, state);
    }
  } else {
    toggle.textContent = '\u25B6';
    toggle.classList.remove('expanded');
    childrenContainer.classList.add('collapsed');
  }
}

/**
 * Select a file and show its symbols.
 * @param {Object} file - File data
 * @param {Object} state - App state
 */
async function selectTreeViewFile(file, state) {
  state.selectedFile = file.path;
  
  const mainContent = document.getElementById('tree-main-content');
  if (!mainContent) return;
  
  mainContent.innerHTML = '<div class="loading">loading symbols</div>';
  
  try {
    const data = await fetchFileSymbols(file.path);
    
    if (!data || !data.symbols || data.symbols.length === 0) {
      mainContent.innerHTML = `
        <div class="file-symbols">
          <div class="file-symbols-header">
            <span class="file-symbols-path">${escapeHtml(file.path)}</span>
            <span class="file-symbols-count">0 symbols</span>
          </div>
          <div class="file-symbols-empty">No symbols in this file</div>
        </div>
      `;
      return;
    }
    
    renderTreeViewFileSymbols(mainContent, file, data.symbols, state);
    
  } catch (err) {
    mainContent.innerHTML = `<div class="tree-main-empty error">Failed to load: ${err.message}</div>`;
  }
}

/**
 * Render file symbols in the main content area.
 * @param {HTMLElement} container - Container element
 * @param {Object} file - File data
 * @param {Array} symbols - Symbols array
 * @param {Object} state - App state
 */
function renderTreeViewFileSymbols(container, file, symbols, state) {
  const deadCount = symbols.filter(s => s.dead).length;
  const deadPct = symbols.length > 0 ? Math.round((deadCount / symbols.length) * 100) : 0;
  
  // Group symbols by type
  const byType = {};
  symbols.forEach(s => {
    if (!byType[s.type]) byType[s.type] = [];
    byType[s.type].push(s);
  });
  
  container.innerHTML = `
    <div class="file-symbols">
      <div class="file-symbols-header">
        <div class="file-symbols-info">
          <span class="file-symbols-path">${escapeHtml(file.path)}</span>
          <span class="file-symbols-stats">
            <span class="file-symbols-count">${symbols.length} symbols</span>
            ${deadCount > 0 ? `<span class="file-symbols-dead">${deadCount} dead (${deadPct}%)</span>` : ''}
          </span>
        </div>
      </div>
      <div class="file-symbols-list">
        ${Object.entries(byType).map(([type, items]) => `
          <div class="file-symbols-group">
            <div class="file-symbols-group-header">${type}s (${items.length})</div>
            ${items.map(s => `
              <div class="file-symbol ${s.dead ? 'dead' : ''}" data-line="${s.line}">
                <span class="file-symbol-line">:${s.line}</span>
                <span class="file-symbol-name">${escapeHtml(s.name)}</span>
                <span class="file-symbol-info">
                  <span class="file-symbol-refs" title="${s.refs || 0} references">${s.refs || 0} refs</span>
                  ${s.dead ? '<span class="file-symbol-badge dead">dead</span>' : ''}
                </span>
              </div>
            `).join('')}
          </div>
        `).join('')}
      </div>
    </div>
  `;
  
  // Click handler for symbols
  container.querySelectorAll('.file-symbol').forEach(el => {
    el.addEventListener('click', () => {
      const line = el.dataset.line;
      const symbol = symbols.find(s => String(s.line) === line);
      if (symbol) {
        selectSymbol({
          ...symbol,
          path: file.path
        }, state);
      }
      container.querySelectorAll('.file-symbol').forEach(s => s.classList.remove('selected'));
      el.classList.add('selected');
    });
  });
}

/**
 * Setup keyboard navigation for tree view.
 */
function setupTreeViewKeyboardNav() {
  const sidebar = document.getElementById('tree-content');
  if (!sidebar) return;
  
  sidebar.addEventListener('keydown', (e) => {
    const focused = document.activeElement;
    if (!focused || !focused.classList.contains('tree-row')) return;
    
    const allRows = [...sidebar.querySelectorAll('.tree-row:not(.collapsed .tree-row)')];
    const idx = allRows.indexOf(focused);
    
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      const next = allRows[idx + 1];
      if (next) next.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const prev = allRows[idx - 1];
      if (prev) prev.focus();
    } else if (e.key === 'ArrowRight') {
      // Expand directory
      const node = focused.closest('.tree-node');
      if (node && node.dataset.type === 'dir') {
        const children = node.querySelector('.tree-children');
        if (children && children.classList.contains('collapsed')) {
          focused.click();
        }
      }
    } else if (e.key === 'ArrowLeft') {
      // Collapse directory or go to parent
      const node = focused.closest('.tree-node');
      if (node && node.dataset.type === 'dir') {
        const children = node.querySelector('.tree-children');
        if (children && !children.classList.contains('collapsed')) {
          focused.click();
        }
      }
    }
  });
}

// =============================================================================
// TREE COUNTING HELPERS
// =============================================================================

/**
 * Count total files in a tree node.
 * @param {Object} node - Tree node
 * @returns {number} File count
 */
export function countTreeFiles(node) {
  if (!node) return 0;
  if (node.type !== 'dir') return 1;
  if (!node.children) return 0;
  return node.children.reduce((sum, child) => sum + countTreeFiles(child), 0);
}

/**
 * Count total symbols in a tree node.
 * @param {Object} node - Tree node
 * @returns {number} Symbol count
 */
export function countTreeSymbols(node) {
  if (!node) return 0;
  if (node.type !== 'dir') return node.symbols || 0;
  if (!node.children) return 0;
  return node.children.reduce((sum, child) => sum + countTreeSymbols(child), 0);
}

/**
 * Count dead symbols in a tree node.
 * @param {Object} node - Tree node
 * @returns {number} Dead count
 */
export function countTreeDead(node) {
  if (!node) return 0;
  if (node.type !== 'dir') return node.dead || 0;
  if (!node.children) return 0;
  return node.children.reduce((sum, child) => sum + countTreeDead(child), 0);
}

// =============================================================================
// LEGACY TREE RENDER (for backwards compatibility)
// =============================================================================

/**
 * Render basic tree (legacy).
 * @param {Object} state - App state
 */
export async function renderTree(state) {
  const container = document.getElementById('tree-container');
  if (!container) return;
  
  container.innerHTML = '<div class="tree-loading">loading...</div>';
  
  try {
    state.tree = await fetchTree();
    renderTreeNode(container, state.tree, 0);
  } catch (err) {
    container.innerHTML = '<div class="tree-empty">No tree data</div>';
  }
}

/**
 * Render tree node (legacy).
 * @param {HTMLElement} container - Container
 * @param {Object} node - Node data
 * @param {number} depth - Depth
 */
function renderTreeNode(container, node, depth) {
  if (!node) return;
  
  container.innerHTML = '';
  
  if (node.children) {
    node.children.forEach(child => {
      const el = document.createElement('div');
      el.className = 'tree-item';
      el.dataset.path = child.path || child.name;
      
      if (child.type === 'dir') {
        el.innerHTML = `
          <div class="tree-row" style="padding-left: ${depth * 12 + 8}px">
            <span class="tree-toggle">&#9654;</span>
            <span class="tree-icon">&#128193;</span>
            <span class="tree-name">${child.name}</span>
            <span class="tree-count">${child.count || ''}</span>
          </div>
          <div class="tree-children collapsed"></div>
        `;
        
        el.querySelector('.tree-row').addEventListener('click', () => {
          const toggle = el.querySelector('.tree-toggle');
          const children = el.querySelector('.tree-children');
          const isCollapsed = children.classList.contains('collapsed');
          
          if (isCollapsed) {
            toggle.textContent = '\u25BC';
            children.classList.remove('collapsed');
            if (!children.hasChildNodes() && child.children) {
              child.children.forEach(c => {
                const childEl = document.createElement('div');
                renderTreeItem(childEl, c, depth + 1);
                children.appendChild(childEl);
              });
            }
          } else {
            toggle.textContent = '\u25B6';
            children.classList.add('collapsed');
          }
        });
      } else {
        el.innerHTML = `
          <div class="tree-row" style="padding-left: ${depth * 12 + 20}px">
            <span class="tree-icon">&#128196;</span>
            <span class="tree-name">${child.name}</span>
            <span class="tree-symbols">${child.symbols || 0}</span>
            ${child.dead > 0 ? `<span class="tree-dead">${child.dead}</span>` : ''}
          </div>
        `;
      }
      
      container.appendChild(el);
    });
  }
}

/**
 * Render tree item (legacy).
 * @param {HTMLElement} el - Element
 * @param {Object} child - Child data
 * @param {number} depth - Depth
 */
function renderTreeItem(el, child, depth) {
  el.className = 'tree-item';
  
  if (child.type === 'dir') {
    el.innerHTML = `
      <div class="tree-row" style="padding-left: ${depth * 12 + 8}px">
        <span class="tree-toggle">&#9654;</span>
        <span class="tree-name">${child.name}</span>
      </div>
      <div class="tree-children collapsed"></div>
    `;
  } else {
    el.innerHTML = `
      <div class="tree-row" style="padding-left: ${depth * 12 + 20}px">
        <span class="tree-name">${child.name}</span>
        <span class="tree-symbols">${child.symbols || 0}</span>
      </div>
    `;
  }
}
