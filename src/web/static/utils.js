/**
 * Utils Module
 *
 * Shared utility functions for the Greppy web UI.
 *
 * @module utils
 */

// =============================================================================
// HTML ESCAPING
// =============================================================================

/**
 * Escape HTML special characters to prevent XSS.
 * @param {string} str - Raw string
 * @returns {string} HTML-safe string
 */
export function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/[&<>"']/g, c => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;'
  })[c]);
}

// =============================================================================
// PATH UTILITIES
// =============================================================================

/**
 * Truncate a file path for display, keeping the last 2 segments.
 * @param {string} path - Full file path
 * @returns {string} Truncated path
 */
export function truncatePath(path) {
  if (!path) return '';
  const parts = path.split('/');
  if (parts.length <= 3) return path;
  return '.../' + parts.slice(-2).join('/');
}

// =============================================================================
// NUMBER FORMATTING
// =============================================================================

/**
 * Format a number with locale-aware separators.
 * @param {number} num - Number to format
 * @returns {string} Formatted number
 */
export function formatNumber(num) {
  if (num === null || num === undefined) return '-';
  return num.toLocaleString();
}

// =============================================================================
// LABEL TRUNCATION
// =============================================================================

/**
 * Truncate a label to fit within a given pixel width.
 * @param {string} name - Label text
 * @param {number} maxW - Maximum width in pixels
 * @returns {string} Truncated label
 */
export function truncLabel(name, maxW) {
  const chars = Math.floor(maxW / 6);
  if (name.length <= chars) return name;
  if (chars <= 3) return '';
  return name.slice(0, chars - 2) + '..';
}
