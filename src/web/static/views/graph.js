/**
 * Graph View Module
 *
 * Renders multiple graph visualization modes:
 * - Treemap (hierarchical boxes)
 * - Force Graph (force-directed layout)
 * - Hierarchy Tree (collapsible tree)
 * - Sunburst (radial hierarchy)
 * - Matrix (connection density)
 * - Sankey Flow (call flow)
 *
 * @module views/graph
 */

/* global d3 */

import { fetchGraph, fetchGraphHierarchical, fetchTree } from '../api.js';
import { escapeHtml, truncLabel } from '../utils.js';
import { selectTreemapNode, selectGraphNode } from '../components/detail.js';
import { updateState, debouncedSave, loadState } from '../lib/persistence.js';

// =============================================================================
// MODULE STATE
// =============================================================================

let simulation = null;
let tooltipEl = null;
let currentVisualization = null;

// =============================================================================
// VISUALIZATION MODES
// =============================================================================

const GRAPH_MODES = [
  { id: 'treemap', label: 'Treemap' },
  { id: 'force', label: 'Force Graph' },
  { id: 'hierarchy', label: 'Hierarchy Tree' },
  { id: 'sunburst', label: 'Sunburst' },
  { id: 'matrix', label: 'Matrix' },
  { id: 'sankey', label: 'Sankey Flow' }
];

const GRAPH_LEVELS = [
  { id: 'files', label: 'Files' },
  { id: 'directories', label: 'Directories' },
  { id: 'symbols', label: 'Symbols' }
];

// =============================================================================
// MAIN RENDER
// =============================================================================

/**
 * Render the graph view header with controls.
 * @param {Object} svg - D3 SVG selection
 * @param {number} width - Container width
 * @param {Object} state - App state
 */
function renderGraphHeader(svg, width, state) {
  // Initialize state defaults
  if (!state.graphMode) state.graphMode = 'treemap';
  if (!state.graphLevel) state.graphLevel = 'files';
  
  const header = svg.append('g').attr('class', 'graph-header');
  
  // Background bar
  header.append('rect')
    .attr('x', 0)
    .attr('y', 0)
    .attr('width', width)
    .attr('height', 32)
    .attr('fill', '#111111');
  
  // View dropdown
  let x = 8;
  header.append('text')
    .attr('x', x)
    .attr('y', 20)
    .attr('fill', '#666')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text('View:');
  x += 36;
  
  // Mode selector
  const modeGroup = header.append('g')
    .attr('class', 'graph-mode-selector')
    .attr('transform', `translate(${x}, 6)`);
  
  const currentMode = GRAPH_MODES.find(m => m.id === state.graphMode) || GRAPH_MODES[0];
  
  modeGroup.append('rect')
    .attr('width', 110)
    .attr('height', 20)
    .attr('fill', '#0a0a0a')
    .attr('stroke', '#333')
    .attr('cursor', 'pointer');
  
  modeGroup.append('text')
    .attr('x', 8)
    .attr('y', 14)
    .attr('fill', '#e0e0e0')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .attr('pointer-events', 'none')
    .text(currentMode.label);
  
  modeGroup.append('text')
    .attr('x', 98)
    .attr('y', 14)
    .attr('fill', '#666')
    .attr('font-size', '8px')
    .attr('pointer-events', 'none')
    .text('▼');
  
  modeGroup.on('click', (e) => showModeDropdown(e, state, svg, width));
  
  x += 120;
  
  // Level selector
  header.append('text')
    .attr('x', x)
    .attr('y', 20)
    .attr('fill', '#666')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text('Level:');
  x += 42;
  
  const levelGroup = header.append('g')
    .attr('class', 'graph-level-selector')
    .attr('transform', `translate(${x}, 6)`);
  
  const currentLevel = GRAPH_LEVELS.find(l => l.id === state.graphLevel) || GRAPH_LEVELS[0];
  
  levelGroup.append('rect')
    .attr('width', 90)
    .attr('height', 20)
    .attr('fill', '#0a0a0a')
    .attr('stroke', '#333')
    .attr('cursor', 'pointer');
  
  levelGroup.append('text')
    .attr('x', 8)
    .attr('y', 14)
    .attr('fill', '#e0e0e0')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .attr('pointer-events', 'none')
    .text(currentLevel.label);
  
  levelGroup.append('text')
    .attr('x', 78)
    .attr('y', 14)
    .attr('fill', '#666')
    .attr('font-size', '8px')
    .attr('pointer-events', 'none')
    .text('▼');
  
  levelGroup.on('click', (e) => showLevelDropdown(e, state, svg, width));
}

/**
 * Show mode dropdown menu.
 */
function showModeDropdown(event, state, svg, width) {
  event.stopPropagation();
  hideAllDropdowns(svg);
  
  const dropdown = svg.append('g')
    .attr('class', 'graph-dropdown mode-dropdown')
    .attr('transform', 'translate(44, 26)');
  
  dropdown.append('rect')
    .attr('width', 130)
    .attr('height', GRAPH_MODES.length * 24 + 4)
    .attr('fill', '#0a0a0a')
    .attr('stroke', '#00d4d4');
  
  GRAPH_MODES.forEach((mode, i) => {
    const item = dropdown.append('g')
      .attr('class', 'dropdown-item')
      .attr('transform', `translate(0, ${i * 24 + 2})`)
      .attr('cursor', 'pointer');
    
    item.append('rect')
      .attr('width', 128)
      .attr('height', 24)
      .attr('x', 1)
      .attr('fill', 'transparent');
    
    item.append('text')
      .attr('x', mode.id === state.graphMode ? 20 : 12)
      .attr('y', 16)
      .attr('fill', mode.id === state.graphMode ? '#00d4d4' : '#ccc')
      .attr('font-size', '10px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(mode.label);
    
    if (mode.id === state.graphMode) {
      item.append('text')
        .attr('x', 8)
        .attr('y', 16)
        .attr('fill', '#00d4d4')
        .attr('font-size', '10px')
        .text('✓');
    }
    
    item.on('mouseover', function() {
      d3.select(this).select('rect').attr('fill', '#1a1a1a');
    }).on('mouseout', function() {
      d3.select(this).select('rect').attr('fill', 'transparent');
    }).on('click', () => {
      state.graphMode = mode.id;
      state.treemapPath = '';
      
      // Persist graph mode
      updateState('graphMode', mode.id);
      updateState('treemapPath', '');
      
      renderGraph(state);
    });
  });
  
  // Close on outside click
  setTimeout(() => {
    document.addEventListener('click', () => hideAllDropdowns(svg), { once: true });
  }, 0);
}

/**
 * Show level dropdown menu.
 */
function showLevelDropdown(event, state, svg, width) {
  event.stopPropagation();
  hideAllDropdowns(svg);
  
  const dropdown = svg.append('g')
    .attr('class', 'graph-dropdown level-dropdown')
    .attr('transform', 'translate(164, 26)');
  
  dropdown.append('rect')
    .attr('width', 110)
    .attr('height', GRAPH_LEVELS.length * 24 + 4)
    .attr('fill', '#0a0a0a')
    .attr('stroke', '#00d4d4');
  
  GRAPH_LEVELS.forEach((level, i) => {
    const item = dropdown.append('g')
      .attr('class', 'dropdown-item')
      .attr('transform', `translate(0, ${i * 24 + 2})`)
      .attr('cursor', 'pointer');
    
    item.append('rect')
      .attr('width', 108)
      .attr('height', 24)
      .attr('x', 1)
      .attr('fill', 'transparent');
    
    item.append('text')
      .attr('x', level.id === state.graphLevel ? 20 : 12)
      .attr('y', 16)
      .attr('fill', level.id === state.graphLevel ? '#00d4d4' : '#ccc')
      .attr('font-size', '10px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(level.label);
    
    if (level.id === state.graphLevel) {
      item.append('text')
        .attr('x', 8)
        .attr('y', 16)
        .attr('fill', '#00d4d4')
        .attr('font-size', '10px')
        .text('✓');
    }
    
    item.on('mouseover', function() {
      d3.select(this).select('rect').attr('fill', '#1a1a1a');
    }).on('mouseout', function() {
      d3.select(this).select('rect').attr('fill', 'transparent');
    }).on('click', () => {
      state.graphLevel = level.id;
      renderGraph(state);
    });
  });
  
  setTimeout(() => {
    document.addEventListener('click', () => hideAllDropdowns(svg), { once: true });
  }, 0);
}

/**
 * Hide all dropdown menus.
 */
function hideAllDropdowns(svg) {
  svg.selectAll('.graph-dropdown').remove();
}

/**
 * Render the graph view (multiple visualization modes).
 * @param {Object} state - App state
 */
export async function renderGraph(state) {
  const container = document.getElementById('view-graph');
  const svg = d3.select('#graph-svg');
  const width = container.clientWidth;
  const height = container.clientHeight;
  
  // Stop existing simulation
  if (simulation) {
    simulation.stop();
    simulation = null;
  }
  
  svg.attr('viewBox', [0, 0, width, height]);
  svg.selectAll('*').remove();
  
  // Render header controls
  renderGraphHeader(svg, width, state);
  
  // Content area starts below header
  const contentY = 36;
  const contentHeight = height - contentY;
  
  // Loading state
  svg.append('text')
    .attr('class', 'graph-loading')
    .attr('x', width / 2)
    .attr('y', contentY + contentHeight / 2)
    .attr('text-anchor', 'middle')
    .attr('fill', '#666')
    .text('loading...');
  
  try {
    const mode = state.graphMode || 'treemap';
    
    // Fetch appropriate data based on mode
    let data;
    if (mode === 'treemap' || mode === 'hierarchy' || mode === 'sunburst') {
      data = await fetchGraphHierarchical(state.treemapPath || '', state.filters);
    } else if (mode === 'matrix' || mode === 'sankey' || mode === 'force') {
      data = await fetchGraph(state.filters);
    }
    
    // Clear loading
    svg.selectAll('.graph-loading').remove();
    
    // Create content group
    const content = svg.append('g')
      .attr('class', 'graph-content')
      .attr('transform', `translate(0, ${contentY})`);
    
    // Render based on mode
    switch (mode) {
      case 'treemap':
        if (!data?.root?.children?.length && !state.treemapPath) {
          showEmptyState(content, width, contentHeight, 'no files found');
          return;
        }
        state.treemap = data;
        renderTreemap(content, data, width, contentHeight, state);
        break;
        
      case 'force':
        if (!data?.nodes?.length) {
          showEmptyState(content, width, contentHeight, 'no graph data');
          return;
        }
        state.graph = data;
        renderForceGraph(content, data, width, contentHeight, state);
        break;
        
      case 'hierarchy':
        if (!data?.root?.children?.length) {
          showEmptyState(content, width, contentHeight, 'no hierarchy data');
          return;
        }
        renderHierarchyTree(content, data, width, contentHeight, state);
        break;
        
      case 'sunburst':
        if (!data?.root?.children?.length) {
          showEmptyState(content, width, contentHeight, 'no hierarchy data');
          return;
        }
        renderSunburst(content, data, width, contentHeight, state);
        break;
        
      case 'matrix':
        if (!data?.nodes?.length) {
          showEmptyState(content, width, contentHeight, 'no connection data');
          return;
        }
        renderMatrix(content, data, width, contentHeight, state);
        break;
        
      case 'sankey':
        if (!data?.nodes?.length || !data?.edges?.length) {
          showEmptyState(content, width, contentHeight, 'no flow data');
          return;
        }
        renderSankey(content, data, width, contentHeight, state);
        break;
        
      default:
        showEmptyState(content, width, contentHeight, 'unknown visualization mode');
    }
  } catch (err) {
    svg.selectAll('.graph-loading').remove();
    const content = svg.append('g')
      .attr('class', 'graph-content')
      .attr('transform', `translate(0, ${contentY})`);
    showEmptyState(content, width, height - contentY, `error: ${err.message}`, true);
  }
}

/**
 * Show empty/error state.
 */
function showEmptyState(container, width, height, message, isError = false) {
  container.append('text')
    .attr('x', width / 2)
    .attr('y', height / 2)
    .attr('text-anchor', 'middle')
    .attr('fill', isError ? '#ff3333' : '#666')
    .attr('font-size', '12px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(message);
}

// =============================================================================
// TREEMAP VISUALIZATION
// =============================================================================

/**
 * Render treemap visualization.
 * @param {Object} container - D3 group selection
 * @param {Object} data - Treemap data
 * @param {number} width - Container width
 * @param {number} height - Container height
 * @param {Object} state - App state
 */
function renderTreemap(container, data, width, height, state) {
  const margin = { top: 28, right: 0, bottom: 0, left: 0 };
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  
  // Header
  const header = container.append('g').attr('class', 'treemap-header');
  
  // Breadcrumb
  renderBreadcrumb(header, data.current_path, width, state);
  
  // Totals
  const t = data.totals;
  header.append('text')
    .attr('x', 8)
    .attr('y', 24)
    .attr('fill', '#666')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(`${t.files} files | ${t.symbols} symbols | ${t.dead} dead`);
  
  // D3 hierarchy
  const hierarchy = d3.hierarchy(data.root)
    .sum(d => d.type === 'file' ? Math.max(d.value, 1) : 0)
    .sort((a, b) => b.value - a.value);
  
  // Treemap layout
  d3.treemap()
    .size([innerWidth, innerHeight])
    .paddingOuter(2)
    .paddingTop(18)
    .paddingInner(1)
    .round(true)(hierarchy);
  
  const g = container.append('g').attr('transform', `translate(${margin.left},${margin.top})`);
  
  // Cells
  const cells = g.selectAll('g')
    .data(hierarchy.descendants().slice(1))
    .join('g')
    .attr('transform', d => `translate(${d.x0},${d.y0})`);
  
  // Rectangles
  cells.append('rect')
    .attr('width', d => Math.max(0, d.x1 - d.x0))
    .attr('height', d => Math.max(0, d.y1 - d.y0))
    .attr('fill', d => getTreemapColor(d.data))
    .attr('stroke', '#0a0a0a')
    .attr('stroke-width', 0.5)
    .attr('cursor', d => d.data.type === 'dir' ? 'pointer' : 'default')
    .on('click', (e, d) => handleTreemapClick(e, d, state))
    .on('mouseover', (e, d) => handleTreemapHover(e, d, true))
    .on('mouseout', (e, d) => handleTreemapHover(e, d, false));
  
  // Labels
  cells.filter(d => (d.x1 - d.x0) > 35 && (d.y1 - d.y0) > 14)
    .append('text')
    .attr('x', 3)
    .attr('y', 12)
    .attr('fill', d => d.data.health < 40 ? '#ff9999' : '#ccc')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .attr('pointer-events', 'none')
    .text(d => truncLabel(d.data.name, d.x1 - d.x0 - 6));
  
  // Folder indicators
  cells.filter(d => d.data.type === 'dir' && (d.x1 - d.x0) > 24 && (d.y1 - d.y0) > 14)
    .append('text')
    .attr('x', d => (d.x1 - d.x0) - 14)
    .attr('y', 12)
    .attr('fill', '#666')
    .attr('font-size', '9px')
    .attr('pointer-events', 'none')
    .text('+');
}

/**
 * Get color for treemap cell based on health.
 * @param {Object} data - Cell data
 * @returns {string} Color hex
 */
function getTreemapColor(data) {
  const health = data.health || 100;
  if (data.cycle) return '#332200';
  if (health >= 90) return '#0a1a1a';
  if (health >= 70) return '#0f1f1f';
  if (health >= 50) return '#1a1a0f';
  if (health >= 30) return '#1f1a0a';
  return '#1f0f0f';
}

/**
 * Render breadcrumb navigation.
 * @param {Object} header - D3 selection
 * @param {string} path - Current path
 * @param {number} width - Container width
 * @param {Object} state - App state
 */
function renderBreadcrumb(header, path, width, state) {
  const parts = path ? path.split('/').filter(p => p) : [];
  let x = 8;
  
  // Root
  const root = header.append('text')
    .attr('x', x).attr('y', 12)
    .attr('fill', parts.length ? '#00d4d4' : '#e0e0e0')
    .attr('font-size', '11px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .attr('cursor', parts.length ? 'pointer' : 'default')
    .text('root');
  
  if (parts.length) root.on('click', () => navigateTreemap('', state));
  x += 36;
  
  for (let i = 0; i < parts.length; i++) {
    header.append('text')
      .attr('x', x).attr('y', 12)
      .attr('fill', '#666').attr('font-size', '11px')
      .text('/');
    x += 10;
    
    const pth = parts.slice(0, i + 1).join('/');
    const isLast = i === parts.length - 1;
    
    const link = header.append('text')
      .attr('x', x).attr('y', 12)
      .attr('fill', isLast ? '#e0e0e0' : '#00d4d4')
      .attr('font-size', '11px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .attr('cursor', isLast ? 'default' : 'pointer')
      .text(parts[i]);
    
    if (!isLast) link.on('click', () => navigateTreemap(pth, state));
    x += parts[i].length * 7 + 4;
  }
}

/**
 * Handle treemap cell click.
 * @param {Event} event - Click event
 * @param {Object} d - D3 datum
 * @param {Object} state - App state
 */
function handleTreemapClick(event, d, state) {
  event.stopPropagation();
  if (d.data.type === 'dir') navigateTreemap(d.data.path, state);
  else selectTreemapNode(d.data, state);
}

/**
 * Handle treemap cell hover.
 * @param {Event} event - Mouse event
 * @param {Object} d - D3 datum
 * @param {boolean} hover - Is hovering
 */
function handleTreemapHover(event, d, hover) {
  const rect = d3.select(event.target);
  if (hover) {
    rect.attr('stroke', '#00d4d4').attr('stroke-width', 2);
    showTreemapTooltip(event, d.data);
  } else {
    rect.attr('stroke', '#0a0a0a').attr('stroke-width', 0.5);
    hideTreemapTooltip();
  }
}

/**
 * Navigate to a treemap path.
 * @param {string} path - Path to navigate to
 * @param {Object} state - App state
 */
function navigateTreemap(path, state) {
  state.treemapPath = path;
  
  // Persist treemap path
  updateState('treemapPath', path);
  
  renderGraph(state);
}

/**
 * Show treemap tooltip.
 * @param {Event} event - Mouse event
 * @param {Object} data - Cell data
 */
function showTreemapTooltip(event, data) {
  if (!tooltipEl) {
    tooltipEl = document.createElement('div');
    tooltipEl.className = 'treemap-tooltip';
    document.body.appendChild(tooltipEl);
  }
  const hColor = data.health >= 70 ? '#00d4d4' : data.health >= 40 ? '#ffcc00' : '#ff3333';
  tooltipEl.innerHTML = `
    <div class="tooltip-name">${escapeHtml(data.name)}</div>
    <div class="tooltip-stats">${data.value} symbols | ${data.dead || 0} dead | <span style="color:${hColor}">${data.health}%</span></div>
    ${data.type === 'dir' ? '<div class="tooltip-hint">click to drill down</div>' : ''}
  `;
  tooltipEl.style.display = 'block';
  tooltipEl.style.left = event.pageX + 10 + 'px';
  tooltipEl.style.top = event.pageY + 10 + 'px';
}

/**
 * Hide treemap tooltip.
 */
function hideTreemapTooltip() {
  if (tooltipEl) tooltipEl.style.display = 'none';
}

// =============================================================================
// FORCE-DIRECTED GRAPH
// =============================================================================

/**
 * Render force-directed graph.
 * @param {Object} container - D3 group selection
 * @param {Object} data - Graph data
 * @param {number} width - Container width
 * @param {number} height - Container height
 * @param {Object} state - App state
 */
function renderForceGraph(container, data, width, height, state) {
  const nodes = data.nodes;
  const links = data.edges.map(e => ({ source: e.source, target: e.target, weight: e.weight || 1 }));
  
  // Stats header
  container.append('text')
    .attr('x', 8)
    .attr('y', 16)
    .attr('fill', '#666')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(`${nodes.length} nodes | ${links.length} edges`);
  
  const g = container.append('g').attr('class', 'force-content').attr('transform', 'translate(0, 24)');
  
  // Get parent SVG for zoom
  const svg = d3.select('#graph-svg');
  const zoom = d3.zoom()
    .scaleExtent([0.1, 4])
    .on('zoom', (event) => g.attr('transform', `translate(0, 24) ${event.transform}`));
  
  svg.call(zoom);
  // Initialize with identity transform to prevent snap on first click
  svg.call(zoom.transform, d3.zoomIdentity);
  
  simulation = d3.forceSimulation(nodes)
    .force('link', d3.forceLink(links).id(d => d.id).distance(80))
    .force('charge', d3.forceManyBody().strength(-200))
    .force('center', d3.forceCenter(width / 2, (height - 24) / 2))
    .force('collision', d3.forceCollide().radius(35));
  
  const link = g.append('g').selectAll('line')
    .data(links).join('line')
    .attr('stroke', '#333')
    .attr('stroke-width', d => Math.sqrt(d.weight));
  
  const node = g.append('g').selectAll('g')
    .data(nodes).join('g')
    .attr('class', d => `node ${d.dead > 0 ? 'has-dead' : ''} ${d.cycle ? 'cycle' : ''}`)
    .call(d3.drag()
      .on('start', (e) => { if (!e.active) simulation.alphaTarget(0.3).restart(); e.subject.fx = e.subject.x; e.subject.fy = e.subject.y; })
      .on('drag', (e) => { e.subject.fx = e.x; e.subject.fy = e.y; })
      .on('end', (e) => { if (!e.active) simulation.alphaTarget(0); e.subject.fx = null; e.subject.fy = null; }));
  
  node.append('circle')
    .attr('r', d => Math.sqrt(d.symbols || 1) * 2.5 + 5)
    .attr('fill', '#0a0a0a')
    .attr('stroke', d => d.cycle ? '#ffcc00' : d.dead > 0 ? '#ff3333' : '#00d4d4')
    .attr('stroke-width', 2);
  
  node.append('text')
    .attr('dy', 4)
    .attr('x', d => Math.sqrt(d.symbols || 1) * 2.5 + 10)
    .attr('font-size', '9px')
    .attr('fill', '#666')
    .text(d => d.name);
  
  node.on('mouseover', function(e, d) {
    d3.select(this).select('circle').attr('stroke-width', 3);
    d3.select(this).select('text').attr('fill', '#e0e0e0');
    link.attr('stroke', l => (l.source.id === d.id || l.target.id === d.id) ? '#00d4d4' : '#333');
  }).on('mouseout', function() {
    d3.select(this).select('circle').attr('stroke-width', 2);
    d3.select(this).select('text').attr('fill', '#666');
    link.attr('stroke', '#333');
  }).on('click', (e, d) => selectGraphNode(d, state, d3));
  
  simulation.on('tick', () => {
    link.attr('x1', d => d.source.x).attr('y1', d => d.source.y)
        .attr('x2', d => d.target.x).attr('y2', d => d.target.y);
    node.attr('transform', d => `translate(${d.x},${d.y})`);
  });
}

// =============================================================================
// HIERARCHY TREE VISUALIZATION
// =============================================================================

/**
 * Render collapsible hierarchy tree.
 * @param {Object} container - D3 group selection
 * @param {Object} data - Hierarchical data
 * @param {number} width - Container width
 * @param {number} height - Container height
 * @param {Object} state - App state
 */
function renderHierarchyTree(container, data, width, height, state) {
  const margin = { top: 20, right: 90, bottom: 20, left: 90 };
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  
  // Create the hierarchy
  const root = d3.hierarchy(data.root);
  
  // Calculate total nodes to adjust spacing
  const nodeCount = root.descendants().length;
  const dynamicHeight = Math.max(innerHeight, nodeCount * 20);
  
  // Tree layout
  const treeLayout = d3.tree()
    .size([dynamicHeight, innerWidth - 160]);
  
  // Assign positions
  treeLayout(root);
  
  // Create zoom group
  const svg = d3.select('#graph-svg');
  const g = container.append('g')
    .attr('class', 'hierarchy-tree')
    .attr('transform', `translate(${margin.left}, ${margin.top})`);
  
  const zoom = d3.zoom()
    .scaleExtent([0.3, 3])
    .on('zoom', (event) => g.attr('transform', `translate(${margin.left}, ${margin.top}) ${event.transform}`));
  
  svg.call(zoom);
  // Initialize with identity transform to prevent snap on first click
  svg.call(zoom.transform, d3.zoomIdentity);
  
  // Links
  g.append('g')
    .attr('class', 'tree-links')
    .selectAll('path')
    .data(root.links())
    .join('path')
    .attr('fill', 'none')
    .attr('stroke', '#333')
    .attr('stroke-width', 1)
    .attr('d', d3.linkHorizontal()
      .x(d => d.y)
      .y(d => d.x));
  
  // Nodes
  const nodes = g.append('g')
    .attr('class', 'tree-nodes')
    .selectAll('g')
    .data(root.descendants())
    .join('g')
    .attr('class', d => `tree-node ${d.data.type === 'dir' ? 'dir' : 'file'}`)
    .attr('transform', d => `translate(${d.y},${d.x})`)
    .attr('cursor', 'pointer');
  
  // Node circles
  nodes.append('circle')
    .attr('r', d => d.data.type === 'dir' ? 6 : 4)
    .attr('fill', d => d.data.type === 'dir' ? '#111' : getTreemapColor(d.data))
    .attr('stroke', d => {
      if (d.data.cycle) return '#ffcc00';
      if (d.data.dead > 0) return '#ff3333';
      return d.data.type === 'dir' ? '#00d4d4' : '#666';
    })
    .attr('stroke-width', d => d.data.type === 'dir' ? 2 : 1.5);
  
  // Node labels
  nodes.append('text')
    .attr('dy', '0.31em')
    .attr('x', d => d.children ? -10 : 10)
    .attr('text-anchor', d => d.children ? 'end' : 'start')
    .attr('fill', d => d.data.type === 'dir' ? '#00d4d4' : '#ccc')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => d.data.name);
  
  // Symbol count badges
  nodes.filter(d => d.data.value > 0)
    .append('text')
    .attr('dy', '0.31em')
    .attr('x', d => d.children ? -10 - d.data.name.length * 6 - 30 : 10 + d.data.name.length * 6 + 10)
    .attr('text-anchor', 'start')
    .attr('fill', '#666')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => `(${d.data.value})`);
  
  // Hover and click interactions
  nodes.on('mouseover', function(e, d) {
    d3.select(this).select('circle')
      .attr('stroke-width', 3)
      .attr('stroke', '#00d4d4');
    d3.select(this).select('text').attr('fill', '#fff');
    showTreemapTooltip(e, d.data);
  }).on('mouseout', function(e, d) {
    d3.select(this).select('circle')
      .attr('stroke-width', d.data.type === 'dir' ? 2 : 1.5)
      .attr('stroke', d.data.cycle ? '#ffcc00' : d.data.dead > 0 ? '#ff3333' : d.data.type === 'dir' ? '#00d4d4' : '#666');
    d3.select(this).select('text').attr('fill', d.data.type === 'dir' ? '#00d4d4' : '#ccc');
    hideTreemapTooltip();
  }).on('click', (e, d) => {
    if (d.data.type === 'dir') {
      navigateTreemap(d.data.path, state);
    } else {
      selectTreemapNode(d.data, state);
    }
  });
}

// =============================================================================
// SUNBURST VISUALIZATION
// =============================================================================

/**
 * Render sunburst (radial hierarchy) visualization.
 * @param {Object} container - D3 group selection
 * @param {Object} data - Hierarchical data
 * @param {number} width - Container width
 * @param {number} height - Container height
 * @param {Object} state - App state
 */
function renderSunburst(container, data, width, height, state) {
  const radius = Math.min(width, height) / 2 - 20;
  
  // Create hierarchy with value based on symbol count
  const root = d3.hierarchy(data.root)
    .sum(d => d.type === 'file' ? Math.max(d.value, 1) : 0)
    .sort((a, b) => b.value - a.value);
  
  // Partition layout
  const partition = d3.partition()
    .size([2 * Math.PI, radius]);
  
  partition(root);
  
  // Arc generator
  const arc = d3.arc()
    .startAngle(d => d.x0)
    .endAngle(d => d.x1)
    .padAngle(d => Math.min((d.x1 - d.x0) / 2, 0.005))
    .padRadius(radius / 2)
    .innerRadius(d => d.y0)
    .outerRadius(d => d.y1 - 1);
  
  // Create group centered in container
  const g = container.append('g')
    .attr('class', 'sunburst')
    .attr('transform', `translate(${width / 2}, ${height / 2})`);
  
  // Draw arcs
  const paths = g.selectAll('path')
    .data(root.descendants().filter(d => d.depth > 0))
    .join('path')
    .attr('fill', d => getSunburstColor(d))
    .attr('d', arc)
    .attr('cursor', 'pointer')
    .attr('stroke', '#0a0a0a')
    .attr('stroke-width', 0.5);
  
  // Hover and click interactions
  paths.on('mouseover', function(e, d) {
    d3.select(this)
      .attr('stroke', '#00d4d4')
      .attr('stroke-width', 2);
    showSunburstTooltip(e, d, width, height);
  }).on('mouseout', function() {
    d3.select(this)
      .attr('stroke', '#0a0a0a')
      .attr('stroke-width', 0.5);
    hideTreemapTooltip();
  }).on('click', (e, d) => {
    if (d.data.type === 'dir') {
      navigateTreemap(d.data.path, state);
    } else {
      selectTreemapNode(d.data, state);
    }
  });
  
  // Labels for larger arcs
  const labelArcs = root.descendants().filter(d => {
    const angle = d.x1 - d.x0;
    const innerRadius = d.y0;
    const arcLength = angle * innerRadius;
    return d.depth > 0 && arcLength > 30;
  });
  
  g.selectAll('text.sunburst-label')
    .data(labelArcs)
    .join('text')
    .attr('class', 'sunburst-label')
    .attr('transform', d => {
      const angle = (d.x0 + d.x1) / 2;
      const r = (d.y0 + d.y1) / 2;
      const x = Math.sin(angle) * r;
      const y = -Math.cos(angle) * r;
      const rotation = (angle * 180 / Math.PI) - 90;
      const flip = rotation > 90 ? rotation + 180 : rotation;
      return `translate(${x},${y}) rotate(${flip})`;
    })
    .attr('text-anchor', 'middle')
    .attr('dy', '0.35em')
    .attr('fill', '#ccc')
    .attr('font-size', '8px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .attr('pointer-events', 'none')
    .text(d => truncLabel(d.data.name, 12));
  
  // Center breadcrumb
  renderSunburstBreadcrumb(g, data.current_path, state);
}

/**
 * Get color for sunburst segment.
 */
function getSunburstColor(d) {
  const data = d.data;
  if (data.cycle) return '#332200';
  if (data.type === 'dir') {
    const depthColors = ['#0a1a1a', '#0c1c1c', '#0e1e1e', '#101f1f'];
    return depthColors[Math.min(d.depth - 1, depthColors.length - 1)];
  }
  return getTreemapColor(data);
}

/**
 * Show sunburst tooltip.
 */
function showSunburstTooltip(event, d, width, height) {
  showTreemapTooltip(event, d.data);
}

/**
 * Render breadcrumb in sunburst center.
 */
function renderSunburstBreadcrumb(g, path, state) {
  const parts = path ? path.split('/').filter(p => p) : [];
  
  g.append('text')
    .attr('class', 'sunburst-center')
    .attr('text-anchor', 'middle')
    .attr('dy', '-0.5em')
    .attr('fill', '#666')
    .attr('font-size', '10px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(parts.length > 0 ? parts[parts.length - 1] : 'root');
  
  if (parts.length > 0) {
    g.append('text')
      .attr('class', 'sunburst-back')
      .attr('text-anchor', 'middle')
      .attr('dy', '1em')
      .attr('fill', '#00d4d4')
      .attr('font-size', '9px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .attr('cursor', 'pointer')
      .text('[back]')
      .on('click', () => {
        const parentPath = parts.slice(0, -1).join('/');
        navigateTreemap(parentPath, state);
      });
  }
}

// =============================================================================
// MATRIX VISUALIZATION
// =============================================================================

/**
 * Render connection density matrix with zoom/pan and interactive features.
 * 
 * Features:
 * - Row labels on left, column labels rotated -45deg at top
 * - D3 zoom/pan behavior with scale extent [0.5, 4]
 * - Click interaction dispatches 'matrix-cell-click' custom event
 * - Color legend at bottom showing connection density scale
 * 
 * @param {Object} container - D3 group selection
 * @param {Object} data - Graph data with nodes and edges
 * @param {number} width - Container width
 * @param {number} height - Container height
 * @param {Object} state - App state
 */
function renderMatrix(container, data, width, height, state) {
  const margin = { top: 80, right: 20, bottom: 60, left: 120 };
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  
  // Limit nodes for performance
  const maxNodes = 40;
  const nodes = data.nodes.slice(0, maxNodes);
  const nodeIds = new Set(nodes.map(n => n.id));
  
  // Build adjacency matrix
  const matrix = [];
  const nodeIndex = new Map(nodes.map((n, i) => [n.id, i]));
  
  // Initialize matrix
  nodes.forEach((source, i) => {
    matrix[i] = nodes.map((target, j) => ({
      x: j,
      y: i,
      z: 0,
      source: source,
      target: target
    }));
  });
  
  // Fill in edge weights
  data.edges.forEach(edge => {
    const si = nodeIndex.get(edge.source);
    const ti = nodeIndex.get(edge.target);
    if (si !== undefined && ti !== undefined) {
      matrix[si][ti].z += edge.weight || 1;
      matrix[ti][si].z += edge.weight || 1; // Symmetric
    }
  });
  
  // Cell size
  const cellSize = Math.min(
    innerWidth / nodes.length,
    innerHeight / nodes.length,
    20
  );
  const matrixSize = cellSize * nodes.length;
  
  // Color scale
  const maxZ = d3.max(matrix.flat(), d => d.z) || 1;
  const colorScale = d3.scaleSequential()
    .domain([0, maxZ])
    .interpolator(d3.interpolate('#0a0a0a', '#00d4d4'));
  
  // Create zoomable group - this is the main transform target
  const zoomGroup = container.append('g')
    .attr('class', 'matrix-zoom-group');
  
  // Create matrix group inside zoom group
  const g = zoomGroup.append('g')
    .attr('class', 'matrix')
    .attr('transform', `translate(${margin.left}, ${margin.top})`);
  
  // Setup zoom behavior
  const zoom = d3.zoom()
    .scaleExtent([0.5, 4])
    .on('zoom', (event) => {
      zoomGroup.attr('transform', event.transform);
    });
  
  // Apply zoom to container's parent SVG
  const svg = container.node().ownerSVGElement 
    ? d3.select(container.node().ownerSVGElement) 
    : container;
  svg.call(zoom);
  // Initialize with identity transform to prevent snap on first click
  svg.call(zoom.transform, d3.zoomIdentity);
  
  // Add zoom reset on double-click
  svg.on('dblclick.zoom', () => {
    svg.transition()
      .duration(300)
      .call(zoom.transform, d3.zoomIdentity);
  });
  
  // Draw cells
  const rows = g.selectAll('.matrix-row')
    .data(matrix)
    .join('g')
    .attr('class', 'matrix-row')
    .attr('transform', (d, i) => `translate(0, ${i * cellSize})`);
  
  const cells = rows.selectAll('.matrix-cell')
    .data(d => d)
    .join('rect')
    .attr('class', 'matrix-cell')
    .attr('x', d => d.x * cellSize)
    .attr('width', cellSize - 1)
    .attr('height', cellSize - 1)
    .attr('fill', d => d.z > 0 ? colorScale(d.z) : '#111')
    .attr('stroke', '#0a0a0a')
    .attr('stroke-width', 0.5)
    .attr('cursor', d => d.z > 0 ? 'pointer' : 'default');
  
  // Interactions
  cells.on('mouseover', function(e, d) {
    if (d.z > 0) {
      d3.select(this).attr('stroke', '#00d4d4').attr('stroke-width', 2);
      showMatrixTooltip(e, d);
    }
  }).on('mouseout', function() {
    d3.select(this).attr('stroke', '#0a0a0a').attr('stroke-width', 0.5);
    hideTreemapTooltip();
  }).on('click', (e, d) => {
    if (d.z > 0 && d.source.id !== d.target.id) {
      // Dispatch custom event for detail panel listeners
      const customEvent = new CustomEvent('matrix-cell-click', {
        bubbles: true,
        detail: {
          from: d.source.id,
          to: d.target.id,
          weight: d.z,
          fromNode: d.source,
          toNode: d.target
        }
      });
      e.target.dispatchEvent(customEvent);
      
      // Also select the source node in the graph
      selectGraphNode(d.source, state, d3);
    }
  });
  
  // Row labels (left)
  g.selectAll('.matrix-row-label')
    .data(nodes)
    .join('text')
    .attr('class', 'matrix-row-label')
    .attr('x', -6)
    .attr('y', (d, i) => i * cellSize + cellSize / 2)
    .attr('dy', '0.32em')
    .attr('text-anchor', 'end')
    .attr('fill', '#666')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => truncLabel(d.name, 15));
  
  // Column labels (top, rotated -45 degrees)
  g.selectAll('.matrix-col-label')
    .data(nodes)
    .join('text')
    .attr('class', 'matrix-col-label')
    .attr('transform', (d, i) => `translate(${i * cellSize + cellSize / 2}, -6) rotate(-45)`)
    .attr('text-anchor', 'start')
    .attr('fill', '#666')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => truncLabel(d.name, 12));
  
  // Matrix info text (top right)
  if (nodes.length < maxNodes) {
    container.append('text')
      .attr('x', width - 10)
      .attr('y', 16)
      .attr('text-anchor', 'end')
      .attr('fill', '#666')
      .attr('font-size', '10px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(`${nodes.length} × ${nodes.length} matrix`);
  } else {
    container.append('text')
      .attr('x', width - 10)
      .attr('y', 16)
      .attr('text-anchor', 'end')
      .attr('fill', '#ffcc00')
      .attr('font-size', '10px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(`showing top ${maxNodes} of ${data.nodes.length} nodes`);
  }
  
  // Color legend at bottom
  renderMatrixLegend(container, colorScale, maxZ, width, height, margin);
}

/**
 * Render color legend for the matrix visualization.
 * @param {Object} container - D3 group selection
 * @param {Function} colorScale - D3 color scale
 * @param {number} maxZ - Maximum connection weight
 * @param {number} width - Container width
 * @param {number} height - Container height
 * @param {Object} margin - Margin object
 */
function renderMatrixLegend(container, colorScale, maxZ, width, height, margin) {
  const legendWidth = 200;
  const legendHeight = 12;
  const legendX = margin.left;
  const legendY = height - 30;
  
  // Create legend group
  const legend = container.append('g')
    .attr('class', 'matrix-legend')
    .attr('transform', `translate(${legendX}, ${legendY})`);
  
  // Create gradient definition
  const gradientId = 'matrix-legend-gradient-' + Math.random().toString(36).substr(2, 9);
  const defs = container.append('defs');
  const gradient = defs.append('linearGradient')
    .attr('id', gradientId)
    .attr('x1', '0%')
    .attr('x2', '100%')
    .attr('y1', '0%')
    .attr('y2', '0%');
  
  // Add gradient stops
  const numStops = 10;
  for (let i = 0; i <= numStops; i++) {
    const t = i / numStops;
    gradient.append('stop')
      .attr('offset', `${t * 100}%`)
      .attr('stop-color', colorScale(t * maxZ));
  }
  
  // Draw legend rectangle with gradient
  legend.append('rect')
    .attr('width', legendWidth)
    .attr('height', legendHeight)
    .attr('fill', `url(#${gradientId})`)
    .attr('stroke', '#333')
    .attr('stroke-width', 1)
    .attr('rx', 2);
  
  // Legend title
  legend.append('text')
    .attr('x', 0)
    .attr('y', -4)
    .attr('fill', '#888')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text('Connection Density');
  
  // Min label
  legend.append('text')
    .attr('x', 0)
    .attr('y', legendHeight + 12)
    .attr('fill', '#666')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .attr('text-anchor', 'start')
    .text('0');
  
  // Max label
  legend.append('text')
    .attr('x', legendWidth)
    .attr('y', legendHeight + 12)
    .attr('fill', '#666')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .attr('text-anchor', 'end')
    .text(maxZ.toString());
  
  // Mid label (if range is large enough)
  if (maxZ > 2) {
    legend.append('text')
      .attr('x', legendWidth / 2)
      .attr('y', legendHeight + 12)
      .attr('fill', '#666')
      .attr('font-size', '9px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .attr('text-anchor', 'middle')
      .text(Math.round(maxZ / 2).toString());
  }
  
  // Zoom hint text
  container.append('text')
    .attr('x', width - 10)
    .attr('y', height - 10)
    .attr('text-anchor', 'end')
    .attr('fill', '#555')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text('scroll to zoom, drag to pan, dbl-click to reset');
}

/**
 * Show matrix cell tooltip.
 */
function showMatrixTooltip(event, d) {
  if (!tooltipEl) {
    tooltipEl = document.createElement('div');
    tooltipEl.className = 'treemap-tooltip';
    document.body.appendChild(tooltipEl);
  }
  
  tooltipEl.innerHTML = `
    <div class="tooltip-name">${escapeHtml(d.source.name)} → ${escapeHtml(d.target.name)}</div>
    <div class="tooltip-stats">${d.z} connections</div>
  `;
  tooltipEl.style.display = 'block';
  tooltipEl.style.left = event.pageX + 10 + 'px';
  tooltipEl.style.top = event.pageY + 10 + 'px';
}

// =============================================================================
// SANKEY FLOW VISUALIZATION
// =============================================================================

/**
 * Render Sankey flow diagram with zoom/pan and click interactions.
 * @param {Object} container - D3 group selection
 * @param {Object} data - Graph data with nodes and edges
 * @param {number} width - Container width
 * @param {number} height - Container height
 * @param {Object} state - App state
 */
function renderSankey(container, data, width, height, state) {
  const margin = { top: 50, right: 150, bottom: 30, left: 150 };
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  const nodeWidth = 100;
  const nodeHeight = 24;
  const nodePadding = 6;
  
  // Helper to truncate labels
  const truncateName = (name, maxLen) => {
    if (!name) return '';
    return name.length > maxLen ? name.slice(0, maxLen - 1) + '...' : name;
  };
  
  // Classify nodes by their role
  const nodeMap = new Map(data.nodes.map(n => [n.id, { ...n, inDegree: 0, outDegree: 0 }]));
  
  data.edges.forEach(e => {
    const source = nodeMap.get(e.source);
    const target = nodeMap.get(e.target);
    if (source) source.outDegree++;
    if (target) target.inDegree++;
  });
  
  const nodes = Array.from(nodeMap.values());
  
  // Categorize: entry (no incoming), internal, leaf (no outgoing)
  const entries = nodes.filter(n => n.inDegree === 0 && n.outDegree > 0);
  const leaves = nodes.filter(n => n.outDegree === 0 && n.inDegree > 0);
  const internals = nodes.filter(n => n.inDegree > 0 && n.outDegree > 0);
  
  // Limit for performance
  const maxPerColumn = 20;
  const limitedEntries = entries.slice(0, maxPerColumn);
  const limitedLeaves = leaves.slice(0, maxPerColumn);
  const limitedInternals = internals.slice(0, maxPerColumn);
  
  // Create columns with totals for display
  const columns = [
    { label: 'Entry Points', nodes: limitedEntries, x: 0, total: entries.length },
    { label: 'Internal', nodes: limitedInternals, x: 1, total: internals.length },
    { label: 'Leaf Functions', nodes: limitedLeaves, x: 2, total: leaves.length }
  ];
  
  const columnGap = 80;
  const columnWidth = (innerWidth - columnGap * 2) / 3;
  
  // Position nodes
  const allNodes = [];
  columns.forEach((col, ci) => {
    const totalHeight = col.nodes.length * (nodeHeight + nodePadding);
    const startY = Math.max(margin.top + 30, (innerHeight - totalHeight) / 2 + margin.top);
    
    col.nodes.forEach((n, ni) => {
      n.sankeyX = margin.left + ci * (columnWidth + columnGap) + (columnWidth - nodeWidth) / 2;
      n.sankeyY = startY + ni * (nodeHeight + nodePadding);
      n.sankeyColumn = ci;
      n.columnLabel = col.label;
      allNodes.push(n);
    });
  });
  
  // Build links between adjacent columns
  const sankeyLinks = [];
  const nodeIdSet = new Set(allNodes.map(n => n.id));
  
  data.edges.forEach(e => {
    const source = nodeMap.get(e.source);
    const target = nodeMap.get(e.target);
    if (source && target && nodeIdSet.has(source.id) && nodeIdSet.has(target.id)) {
      if (source.sankeyColumn !== undefined && target.sankeyColumn !== undefined) {
        if (Math.abs(source.sankeyColumn - target.sankeyColumn) === 1) {
          sankeyLinks.push({
            source: source,
            target: target,
            weight: e.weight || 1,
            edgeData: e
          });
        }
      }
    }
  });
  
  // Get the SVG element for zoom behavior
  const svg = container.node().ownerSVGElement 
    ? d3.select(container.node().ownerSVGElement) 
    : container;
  
  // Create main content group that will be transformed by zoom
  const g = container.append('g')
    .attr('class', 'sankey-content');
  
  // Setup zoom behavior
  const zoom = d3.zoom()
    .scaleExtent([0.3, 3])
    .on('zoom', (event) => {
      g.attr('transform', event.transform);
    });
  
  svg.call(zoom);
  // Initialize with identity transform to prevent snap on first click
  svg.call(zoom.transform, d3.zoomIdentity);
  
  // Double-click to reset zoom
  svg.on('dblclick.zoom', () => {
    svg.transition().duration(300).call(zoom.transform, d3.zoomIdentity);
  });
  
  // Add zoom controls hint
  container.append('text')
    .attr('x', 10)
    .attr('y', height - 10)
    .attr('fill', '#555')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text('scroll to zoom, drag to pan, dbl-click to reset');
  
  // Column headers with background
  columns.forEach((col, ci) => {
    const headerX = margin.left + ci * (columnWidth + columnGap) + columnWidth / 2;
    
    // Header background
    g.append('rect')
      .attr('x', headerX - 70)
      .attr('y', 8)
      .attr('width', 140)
      .attr('height', 22)
      .attr('fill', '#111')
      .attr('stroke', '#333')
      .attr('rx', 4);
    
    // Header text
    g.append('text')
      .attr('x', headerX)
      .attr('y', 23)
      .attr('text-anchor', 'middle')
      .attr('fill', '#00d4d4')
      .attr('font-size', '11px')
      .attr('font-weight', 'bold')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(col.label);
    
    // Count badge
    g.append('text')
      .attr('x', headerX)
      .attr('y', 42)
      .attr('text-anchor', 'middle')
      .attr('fill', col.nodes.length < col.total ? '#ffcc00' : '#666')
      .attr('font-size', '9px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(`${col.nodes.length}${col.nodes.length < col.total ? ' of ' + col.total : ''}`);
  });
  
  // Links
  const maxWeight = d3.max(sankeyLinks, l => l.weight) || 1;
  
  const links = g.selectAll('.sankey-link')
    .data(sankeyLinks)
    .join('path')
    .attr('class', 'sankey-link')
    .attr('d', d => {
      const x0 = d.source.sankeyX + nodeWidth;
      const y0 = d.source.sankeyY + nodeHeight / 2;
      const x1 = d.target.sankeyX;
      const y1 = d.target.sankeyY + nodeHeight / 2;
      const midX = (x0 + x1) / 2;
      return `M${x0},${y0} C${midX},${y0} ${midX},${y1} ${x1},${y1}`;
    })
    .attr('fill', 'none')
    .attr('stroke', '#00d4d4')
    .attr('stroke-opacity', 0.25)
    .attr('stroke-width', d => Math.max(1.5, (d.weight / maxWeight) * 5))
    .attr('cursor', 'pointer');
  
  // Link interactions
  links.on('mouseover', function(e, d) {
    d3.select(this)
      .attr('stroke-opacity', 0.9)
      .attr('stroke', '#00ffff');
    showSankeyLinkTooltip(e, d);
  }).on('mouseout', function() {
    d3.select(this)
      .attr('stroke-opacity', 0.25)
      .attr('stroke', '#00d4d4');
    hideTreemapTooltip();
  }).on('click', function(e, d) {
    e.stopPropagation();
    // Dispatch custom event for link click
    const event = new CustomEvent('sankey-link-click', {
      detail: {
        source: { id: d.source.id, name: d.source.name, file: d.source.file },
        target: { id: d.target.id, name: d.target.name, file: d.target.file },
        weight: d.weight,
        relationship: `${d.source.name} -> ${d.target.name}`
      },
      bubbles: true
    });
    container.node().dispatchEvent(event);
  });
  
  // Nodes
  const nodeGroups = g.selectAll('.sankey-node')
    .data(allNodes)
    .join('g')
    .attr('class', 'sankey-node')
    .attr('transform', d => `translate(${d.sankeyX}, ${d.sankeyY})`)
    .attr('cursor', 'pointer');
  
  // Node rectangles with better styling
  nodeGroups.append('rect')
    .attr('class', 'sankey-node-rect')
    .attr('width', nodeWidth)
    .attr('height', nodeHeight)
    .attr('fill', d => d.dead > 0 ? '#1f0f0f' : '#0a1a1a')
    .attr('stroke', d => d.cycle ? '#ffcc00' : d.dead > 0 ? '#ff3333' : '#333')
    .attr('stroke-width', 1)
    .attr('rx', 3);
  
  // Internal labels (on the node)
  nodeGroups.append('text')
    .attr('class', 'sankey-node-label-internal')
    .attr('x', nodeWidth / 2)
    .attr('y', nodeHeight / 2)
    .attr('dy', '0.35em')
    .attr('text-anchor', 'middle')
    .attr('fill', d => d.dead > 0 ? '#ff9999' : '#ccc')
    .attr('font-size', '9px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => truncateName(d.name, 14));
  
  // External labels based on column position
  nodeGroups.append('text')
    .attr('class', 'sankey-node-label-external')
    .attr('x', d => {
      // Left column: label on left side
      if (d.sankeyColumn === 0) return -8;
      // Right column: label on right side
      if (d.sankeyColumn === 2) return nodeWidth + 8;
      // Middle column: label below
      return nodeWidth / 2;
    })
    .attr('y', d => {
      // Middle column: label below node
      if (d.sankeyColumn === 1) return nodeHeight + 12;
      return nodeHeight / 2;
    })
    .attr('dy', d => d.sankeyColumn === 1 ? '0' : '0.35em')
    .attr('text-anchor', d => {
      if (d.sankeyColumn === 0) return 'end';
      if (d.sankeyColumn === 2) return 'start';
      return 'middle';
    })
    .attr('fill', '#888')
    .attr('font-size', '8px')
    .attr('font-family', 'JetBrains Mono, monospace')
    .text(d => {
      const fullName = d.name || d.id;
      // Show longer names in external label for left/right columns
      if (d.sankeyColumn === 0 || d.sankeyColumn === 2) {
        return truncateName(fullName, 20);
      }
      // Middle column: show only if name is longer than internal label
      return fullName.length > 14 ? truncateName(fullName, 25) : '';
    });
  
  // Node interactions
  nodeGroups.on('mouseover', function(e, d) {
    d3.select(this).select('.sankey-node-rect')
      .attr('stroke', '#00d4d4')
      .attr('stroke-width', 2)
      .attr('fill', d.dead > 0 ? '#2a1515' : '#0f2a2a');
    
    d3.select(this).select('.sankey-node-label-external')
      .attr('fill', '#00d4d4');
    
    // Highlight connected links
    links.attr('stroke-opacity', l => 
      (l.source.id === d.id || l.target.id === d.id) ? 0.8 : 0.08);
    
    // Dim unconnected nodes
    nodeGroups.filter(n => n.id !== d.id)
      .select('.sankey-node-rect')
      .attr('opacity', n => {
        const isConnected = sankeyLinks.some(l => 
          (l.source.id === d.id && l.target.id === n.id) ||
          (l.target.id === d.id && l.source.id === n.id)
        );
        return isConnected ? 1 : 0.4;
      });
    
    showSankeyTooltip(e, d);
  }).on('mouseout', function(e, d) {
    d3.select(this).select('.sankey-node-rect')
      .attr('stroke', d.cycle ? '#ffcc00' : d.dead > 0 ? '#ff3333' : '#333')
      .attr('stroke-width', 1)
      .attr('fill', d.dead > 0 ? '#1f0f0f' : '#0a1a1a');
    
    d3.select(this).select('.sankey-node-label-external')
      .attr('fill', '#888');
    
    links.attr('stroke-opacity', 0.25);
    nodeGroups.select('.sankey-node-rect').attr('opacity', 1);
    hideTreemapTooltip();
  }).on('click', function(e, d) {
    e.stopPropagation();
    
    // Dispatch custom event for node click
    const event = new CustomEvent('sankey-node-click', {
      detail: {
        id: d.id,
        name: d.name,
        file: d.file,
        line: d.line,
        column: d.columnLabel,
        inDegree: d.inDegree,
        outDegree: d.outDegree,
        dead: d.dead || 0,
        cycle: d.cycle || false,
        type: d.type
      },
      bubbles: true
    });
    container.node().dispatchEvent(event);
    
    // Also call existing selection handler
    selectGraphNode(d, state, d3);
  });
  
  // Info text
  const totalShown = limitedEntries.length + limitedInternals.length + limitedLeaves.length;
  const totalNodes = entries.length + internals.length + leaves.length;
  
  if (totalShown < totalNodes) {
    container.append('text')
      .attr('x', width - 10)
      .attr('y', height - 10)
      .attr('text-anchor', 'end')
      .attr('fill', '#ffcc00')
      .attr('font-size', '10px')
      .attr('font-family', 'JetBrains Mono, monospace')
      .text(`showing ${totalShown} of ${totalNodes} nodes`);
  }
}

/**
 * Show sankey link tooltip.
 */
function showSankeyLinkTooltip(event, d) {
  if (!tooltipEl) {
    tooltipEl = document.createElement('div');
    tooltipEl.className = 'treemap-tooltip';
    document.body.appendChild(tooltipEl);
  }
  
  tooltipEl.innerHTML = `
    <div class="tooltip-name">${escapeHtml(d.source.name)} -> ${escapeHtml(d.target.name)}</div>
    <div class="tooltip-stats">
      Weight: ${d.weight}
      <br>Click for details
    </div>
  `;
  tooltipEl.style.display = 'block';
  tooltipEl.style.left = event.pageX + 10 + 'px';
  tooltipEl.style.top = event.pageY + 10 + 'px';
}

/**
 * Show sankey node tooltip.
 */
function showSankeyTooltip(event, d) {
  if (!tooltipEl) {
    tooltipEl = document.createElement('div');
    tooltipEl.className = 'treemap-tooltip';
    document.body.appendChild(tooltipEl);
  }
  
  tooltipEl.innerHTML = `
    <div class="tooltip-name">${escapeHtml(d.name)}</div>
    <div class="tooltip-stats">
      ${d.inDegree} incoming | ${d.outDegree} outgoing
      ${d.dead > 0 ? ` | <span style="color:#ff3333">${d.dead} dead</span>` : ''}
    </div>
  `;
  tooltipEl.style.display = 'block';
  tooltipEl.style.left = event.pageX + 10 + 'px';
  tooltipEl.style.top = event.pageY + 10 + 'px';
}
