/**
 * Error State Components
 *
 * Provides error displays with retry actions.
 *
 * @module components/error
 */

// =============================================================================
// ERROR STATE RENDERER
// =============================================================================

/**
 * Render an error state.
 * @param {Object} options - Error state options
 * @param {string} options.title - Error title
 * @param {string} options.message - Error message
 * @param {Function} [options.onRetry] - Retry callback name (as string)
 * @returns {string} HTML string
 */
export function renderErrorState({ title = 'Error', message, onRetry }) {
  let retryHtml = '';
  if (onRetry) {
    retryHtml = `
      <button class="error-state-retry" onclick="${onRetry}()">
        Retry
      </button>
    `;
  }

  return `
    <div class="error-state fade-in" role="alert" aria-live="assertive">
      <div class="error-state-icon" aria-hidden="true">!</div>
      <div class="error-state-title">${title}</div>
      <div class="error-state-message">${message}</div>
      ${retryHtml}
    </div>
  `;
}

// =============================================================================
// PRESET ERROR STATES
// =============================================================================

/**
 * Error state for API failures.
 * @param {string} resource - What failed to load
 * @param {string} [onRetry] - Retry callback name
 * @returns {string} HTML string
 */
export function errorLoadFailed(resource, onRetry = 'location.reload') {
  return renderErrorState({
    title: 'Failed to load',
    message: `Could not load ${resource}. Check if greppy daemon is running.`,
    onRetry
  });
}

/**
 * Error state for stats API failure.
 * @param {string} [onRetry] - Retry callback name
 * @returns {string} HTML string
 */
export function errorStatsLoad(onRetry = 'window.refreshStats') {
  return renderErrorState({
    title: 'Failed to load stats',
    message: 'Could not fetch codebase statistics. The server may be unavailable.',
    onRetry
  });
}

/**
 * Error state for list API failure.
 * @param {string} [onRetry] - Retry callback name
 * @returns {string} HTML string
 */
export function errorListLoad(onRetry = 'window.refreshList') {
  return renderErrorState({
    title: 'Failed to load symbols',
    message: 'Could not fetch symbol list. Try refreshing the page.',
    onRetry
  });
}

/**
 * Error state for graph API failure.
 * @param {string} [onRetry] - Retry callback name
 * @returns {string} HTML string
 */
export function errorGraphLoad(onRetry = 'window.refreshGraph') {
  return renderErrorState({
    title: 'Failed to load graph',
    message: 'Could not fetch graph data. Try refreshing the page.',
    onRetry
  });
}

/**
 * Error state for cycles API failure.
 * @param {string} [onRetry] - Retry callback name
 * @returns {string} HTML string
 */
export function errorCyclesLoad(onRetry = 'window.refreshCycles') {
  return renderErrorState({
    title: 'Failed to load cycles',
    message: 'Could not fetch circular dependency data.',
    onRetry
  });
}

/**
 * Error state for detail panel API failure.
 * @param {string} [onRetry] - Retry callback name
 * @returns {string} HTML string
 */
export function errorDetailLoad(onRetry = 'window.refreshDetail') {
  return renderErrorState({
    title: 'Failed to load details',
    message: 'Could not fetch symbol details.',
    onRetry
  });
}

/**
 * Error state for timeline API failure.
 * @param {string} [onRetry] - Retry callback name
 * @returns {string} HTML string
 */
export function errorTimelineLoad(onRetry = 'window.refreshTimeline') {
  return renderErrorState({
    title: 'Failed to load timeline',
    message: 'Could not fetch snapshot data.',
    onRetry
  });
}

/**
 * Generic network error.
 * @returns {string} HTML string
 */
export function errorNetwork() {
  return renderErrorState({
    title: 'Network error',
    message: 'Could not connect to the server. Check your connection.',
    onRetry: 'location.reload'
  });
}

/**
 * Error state for index not found.
 * @returns {string} HTML string
 */
export function errorNoIndex() {
  return renderErrorState({
    title: 'No index found',
    message: 'Run "greppy index" to analyze your codebase first.'
  });
}
