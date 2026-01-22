/**
 * Skeleton Loading Components
 *
 * Provides shimmer loading placeholders for all views.
 * Uses CSS-only animations for performance.
 *
 * @module components/skeleton
 */

// =============================================================================
// BASE SKELETONS
// =============================================================================

/**
 * Create a skeleton text line.
 * @param {'short' | 'medium' | 'full'} width - Width variant
 * @returns {string} HTML string
 */
export function skeletonText(width = 'full') {
  const cls = width === 'full' ? '' : ` skeleton-text-${width}`;
  return `<div class="skeleton skeleton-text${cls}"></div>`;
}

/**
 * Create a skeleton rectangle.
 * @param {number} width - Width in pixels
 * @param {number} height - Height in pixels
 * @returns {string} HTML string
 */
export function skeletonRect(width, height) {
  return `<div class="skeleton skeleton-rect" style="width: ${width}px; height: ${height}px;"></div>`;
}

// =============================================================================
// STATS VIEW SKELETON
// =============================================================================

/**
 * Render skeleton for stats view.
 * @returns {string} HTML string
 */
export function renderStatsSkeleton() {
  const statCard = `
    <div class="skeleton-stat-card">
      <div class="skeleton skeleton-value"></div>
      <div class="skeleton skeleton-label"></div>
    </div>
  `;

  return `
    <div class="stats-skeleton fade-in" role="status" aria-label="Loading statistics">
      <div class="sr-only">Loading statistics...</div>
      
      <!-- Metric cards -->
      <div class="stats-header" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: var(--space-4); margin-bottom: var(--space-6);">
        ${statCard}
        ${statCard}
        ${statCard}
        ${statCard}
        ${statCard}
      </div>
      
      <!-- Charts area -->
      <div style="display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-4);">
        <div class="skeleton-graph">
          <div class="skeleton-graph-placeholder">
            <div class="skeleton skeleton-graph-bar" style="height: 60%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 80%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 45%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 90%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 70%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 55%;"></div>
          </div>
        </div>
        <div class="skeleton-graph">
          <div class="skeleton-graph-placeholder">
            <div class="skeleton skeleton-graph-bar" style="height: 40%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 65%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 85%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 50%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 75%;"></div>
            <div class="skeleton skeleton-graph-bar" style="height: 60%;"></div>
          </div>
        </div>
      </div>
    </div>
  `;
}

// =============================================================================
// LIST VIEW SKELETON
// =============================================================================

/**
 * Render skeleton for list view.
 * @param {number} rows - Number of skeleton rows
 * @returns {string} HTML string
 */
export function renderListSkeleton(rows = 15) {
  const tableRow = `
    <div class="skeleton-table-row">
      <div class="skeleton skeleton-cell"></div>
      <div class="skeleton skeleton-cell" style="width: 60%;"></div>
      <div class="skeleton skeleton-cell" style="width: 80%;"></div>
      <div class="skeleton skeleton-cell" style="width: 40%;"></div>
      <div class="skeleton skeleton-cell" style="width: 50%;"></div>
    </div>
  `;

  return `
    <div class="list-skeleton fade-in" role="status" aria-label="Loading symbols">
      <div class="sr-only">Loading symbol list...</div>
      
      <!-- Header row -->
      <div class="skeleton-table-row" style="background: var(--bg-raised); border-bottom: 1px solid var(--text-muted);">
        <div class="skeleton skeleton-cell" style="width: 50px;"></div>
        <div class="skeleton skeleton-cell" style="width: 40px;"></div>
        <div class="skeleton skeleton-cell" style="width: 30px;"></div>
        <div class="skeleton skeleton-cell" style="width: 40px;"></div>
        <div class="skeleton skeleton-cell" style="width: 35px;"></div>
      </div>
      
      <!-- Data rows -->
      ${Array(rows).fill(tableRow).join('')}
    </div>
  `;
}

// =============================================================================
// GRAPH VIEW SKELETON
// =============================================================================

/**
 * Render skeleton for graph view.
 * @returns {string} HTML string
 */
export function renderGraphSkeleton() {
  return `
    <div class="graph-skeleton fade-in" role="status" aria-label="Loading graph">
      <div class="sr-only">Loading visualization...</div>
      
      <div class="skeleton-graph" style="height: calc(100vh - 250px); min-height: 400px;">
        <div style="text-align: center; color: var(--text-dim);">
          <div style="margin-bottom: var(--space-4);">
            <div class="skeleton" style="width: 200px; height: 200px; margin: 0 auto; border-radius: 4px;"></div>
          </div>
          <div class="skeleton skeleton-text" style="width: 150px; margin: 0 auto;"></div>
        </div>
      </div>
    </div>
  `;
}

// =============================================================================
// TREE VIEW SKELETON
// =============================================================================

/**
 * Render skeleton for tree view.
 * @returns {string} HTML string
 */
export function renderTreeSkeleton() {
  const treeItem = (indent = 0) => `
    <div style="display: flex; align-items: center; gap: var(--space-2); padding: var(--space-2) var(--space-3); padding-left: ${16 + indent * 16}px;">
      <div class="skeleton" style="width: 12px; height: 12px;"></div>
      <div class="skeleton skeleton-text" style="width: ${80 + Math.random() * 60}px;"></div>
    </div>
  `;

  return `
    <div class="tree-skeleton fade-in" role="status" aria-label="Loading file tree">
      <div class="sr-only">Loading file tree...</div>
      
      ${treeItem(0)}
      ${treeItem(1)}
      ${treeItem(1)}
      ${treeItem(2)}
      ${treeItem(2)}
      ${treeItem(2)}
      ${treeItem(1)}
      ${treeItem(2)}
      ${treeItem(0)}
      ${treeItem(1)}
      ${treeItem(1)}
      ${treeItem(2)}
    </div>
  `;
}

// =============================================================================
// CYCLES VIEW SKELETON
// =============================================================================

/**
 * Render skeleton for cycles view.
 * @param {number} cards - Number of skeleton cards
 * @returns {string} HTML string
 */
export function renderCyclesSkeleton(cards = 4) {
  const cycleCard = `
    <div style="background: var(--bg-raised); border: 1px solid var(--text-muted); padding: var(--space-4);">
      <div style="display: flex; justify-content: space-between; margin-bottom: var(--space-3);">
        <div class="skeleton" style="width: 100px; height: 16px;"></div>
        <div class="skeleton" style="width: 60px; height: 16px;"></div>
      </div>
      <div style="display: flex; gap: var(--space-2); flex-wrap: wrap;">
        <div class="skeleton" style="width: 80px; height: 20px;"></div>
        <div class="skeleton" style="width: 60px; height: 20px;"></div>
        <div class="skeleton" style="width: 70px; height: 20px;"></div>
        <div class="skeleton" style="width: 90px; height: 20px;"></div>
      </div>
    </div>
  `;

  return `
    <div class="cycles-skeleton fade-in" role="status" aria-label="Loading cycles">
      <div class="sr-only">Loading circular dependencies...</div>
      
      <div style="display: flex; flex-direction: column; gap: var(--space-4);">
        ${Array(cards).fill(cycleCard).join('')}
      </div>
    </div>
  `;
}

// =============================================================================
// TABLES VIEW SKELETON
// =============================================================================

/**
 * Render skeleton for tables view.
 * @param {number} rows - Number of skeleton rows
 * @returns {string} HTML string
 */
export function renderTablesSkeleton(rows = 10) {
  return renderListSkeleton(rows);
}

// =============================================================================
// DETAIL PANEL SKELETON
// =============================================================================

/**
 * Render skeleton for detail panel.
 * @returns {string} HTML string
 */
export function renderDetailSkeleton() {
  return `
    <div class="skeleton-detail fade-in" role="status" aria-label="Loading symbol details">
      <div class="sr-only">Loading symbol details...</div>
      
      <div class="skeleton-detail-header">
        <div class="skeleton skeleton-detail-title"></div>
        <div class="skeleton skeleton-detail-subtitle"></div>
      </div>
      
      <div class="skeleton-detail-section">
        <div class="skeleton skeleton-detail-section-title"></div>
        ${skeletonText('medium')}
        ${skeletonText('short')}
        ${skeletonText('medium')}
      </div>
      
      <div class="skeleton-detail-section">
        <div class="skeleton skeleton-detail-section-title"></div>
        ${skeletonText('full')}
        ${skeletonText('short')}
      </div>
    </div>
  `;
}

// =============================================================================
// TIMELINE VIEW SKELETON
// =============================================================================

/**
 * Render skeleton for timeline view.
 * @returns {string} HTML string
 */
export function renderTimelineSkeleton() {
  const snapshotRow = `
    <div class="skeleton-table-row">
      <div class="skeleton skeleton-cell" style="width: 120px;"></div>
      <div class="skeleton skeleton-cell" style="width: 50px;"></div>
      <div class="skeleton skeleton-cell" style="width: 50px;"></div>
      <div class="skeleton skeleton-cell" style="width: 50px;"></div>
      <div class="skeleton skeleton-cell" style="width: 80px;"></div>
    </div>
  `;

  return `
    <div class="timeline-skeleton fade-in" role="status" aria-label="Loading timeline">
      <div class="sr-only">Loading timeline...</div>
      
      <!-- Chart area -->
      <div class="skeleton-graph" style="height: 250px; margin-bottom: var(--space-6);">
        <div style="width: 90%; height: 80%; display: flex; align-items: flex-end; gap: 2px;">
          ${Array(20).fill('').map(() => `
            <div class="skeleton" style="flex: 1; height: ${30 + Math.random() * 60}%;"></div>
          `).join('')}
        </div>
      </div>
      
      <!-- Snapshot list -->
      ${Array(5).fill(snapshotRow).join('')}
    </div>
  `;
}
