/**
 * Empty State Components
 *
 * Provides empty state displays when no data matches filters.
 *
 * @module components/empty
 */

// =============================================================================
// ASCII ICONS
// =============================================================================

const ICONS = {
  noData: `
    ╭───────╮
    │       │
    │  ?  ? │
    │   ─   │
    ╰───────╯
  `,
  noResults: `
    ╭───╮
    │ ⌕ │
    ╰───╯
     ╱ ╲
  `,
  noCycles: `
    ╭─────╮
    │  ✓  │
    ╰─────╯
  `,
  error: `
    ╭───────╮
    │  ! !  │
    │   ▲   │
    ╰───────╯
  `,
  empty: `
    ┌─────────┐
    │         │
    │    ∅    │
    │         │
    └─────────┘
  `
};

// =============================================================================
// EMPTY STATE RENDERER
// =============================================================================

/**
 * Render an empty state.
 * @param {Object} options - Empty state options
 * @param {string} options.icon - Icon key from ICONS
 * @param {string} options.title - Title text
 * @param {string} options.description - Description text
 * @param {Object} [options.action] - Optional action button
 * @param {string} options.action.label - Button label
 * @param {string} options.action.onClick - Inline onclick handler
 * @returns {string} HTML string
 */
export function renderEmptyState({ icon = 'empty', title, description, action }) {
  const iconArt = ICONS[icon] || ICONS.empty;
  
  let actionHtml = '';
  if (action) {
    actionHtml = `
      <div class="empty-state-action">
        <button class="btn btn-secondary" onclick="${action.onClick}">${action.label}</button>
      </div>
    `;
  }

  return `
    <div class="empty-state fade-in" role="status">
      <pre class="empty-state-icon" aria-hidden="true">${iconArt}</pre>
      <div class="empty-state-title">${title}</div>
      <div class="empty-state-description">${description}</div>
      ${actionHtml}
    </div>
  `;
}

// =============================================================================
// PRESET EMPTY STATES
// =============================================================================

/**
 * Empty state for no symbols.
 * @returns {string} HTML string
 */
export function emptyNoSymbols() {
  return renderEmptyState({
    icon: 'noData',
    title: 'No symbols found',
    description: 'Run greppy index to analyze your codebase.',
    action: {
      label: 'Refresh',
      onClick: 'location.reload()'
    }
  });
}

/**
 * Empty state for no search results.
 * @param {Function} [onClear] - Clear filters callback name
 * @returns {string} HTML string
 */
export function emptyNoResults(onClear = 'window.clearFilters') {
  return renderEmptyState({
    icon: 'noResults',
    title: 'No results',
    description: 'No symbols match your current filters. Try adjusting your search.',
    action: {
      label: 'Clear filters',
      onClick: `${onClear}()`
    }
  });
}

/**
 * Empty state for no cycles.
 * @returns {string} HTML string
 */
export function emptyNoCycles() {
  return renderEmptyState({
    icon: 'noCycles',
    title: 'No circular dependencies',
    description: 'Your codebase is cycle-free. Nice work!'
  });
}

/**
 * Empty state for no dead code.
 * @returns {string} HTML string
 */
export function emptyNoDeadCode() {
  return renderEmptyState({
    icon: 'noCycles',
    title: 'No dead code',
    description: 'All symbols in your codebase are being used.'
  });
}

/**
 * Empty state for no snapshots.
 * @returns {string} HTML string
 */
export function emptyNoSnapshots() {
  return renderEmptyState({
    icon: 'empty',
    title: 'No snapshots yet',
    description: 'Create a snapshot to start tracking your codebase over time.',
    action: {
      label: 'Create snapshot',
      onClick: 'window.createSnapshot()'
    }
  });
}

/**
 * Empty state for no files.
 * @returns {string} HTML string
 */
export function emptyNoFiles() {
  return renderEmptyState({
    icon: 'noData',
    title: 'No files indexed',
    description: 'Run greppy index to analyze your codebase.'
  });
}

/**
 * Empty state for no entry points.
 * @returns {string} HTML string
 */
export function emptyNoEntryPoints() {
  return renderEmptyState({
    icon: 'noData',
    title: 'No entry points detected',
    description: 'Entry points include main functions, tests, and exports.'
  });
}

/**
 * Empty state for no connections.
 * @returns {string} HTML string
 */
export function emptyNoConnections() {
  return renderEmptyState({
    icon: 'empty',
    title: 'No connections',
    description: 'This symbol has no callers or callees.'
  });
}
