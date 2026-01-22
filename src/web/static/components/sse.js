/**
 * SSE Module
 *
 * Server-Sent Events connection and handlers.
 *
 * @module components/sse
 */

// =============================================================================
// SSE STATE
// =============================================================================

let eventSource = null;
let reconnectAttempts = 0;
const MAX_RECONNECT_ATTEMPTS = 10;
const INITIAL_RECONNECT_DELAY = 1000;

// =============================================================================
// CONNECTION
// =============================================================================

/**
 * Connect to SSE endpoint for live updates.
 * @param {Object} state - App state
 * @param {Function} onRefresh - Callback to refresh all data
 */
export function connectSSE(state, onRefresh) {
  if (eventSource) {
    eventSource.close();
  }
  
  eventSource = new EventSource('/api/events');
  
  eventSource.onopen = () => {
    console.log('[SSE] Connected');
    reconnectAttempts = 0;
    updateConnectionStatus(true);
  };
  
  eventSource.onerror = (e) => {
    console.warn('[SSE] Connection error', e);
    updateConnectionStatus(false);
    eventSource.close();
    
    // Reconnect with exponential backoff
    if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
      const delay = INITIAL_RECONNECT_DELAY * Math.pow(2, reconnectAttempts);
      reconnectAttempts++;
      console.log(`[SSE] Reconnecting in ${delay}ms (attempt ${reconnectAttempts})`);
      setTimeout(() => connectSSE(state, onRefresh), delay);
    } else {
      console.error('[SSE] Max reconnect attempts reached');
    }
  };
  
  // Handle connection event
  eventSource.addEventListener('connected', (e) => {
    try {
      const data = JSON.parse(e.data);
      console.log('[SSE] Connected event:', data);
      state.daemonConnected = data.daemon;
      if (data.indexed_at) {
        state.indexedAt = data.indexed_at * 1000; // Convert to milliseconds
      }
      updateConnectionStatus(true, data.daemon);
    } catch (err) {
      console.error('[SSE] Error parsing connected event:', err);
    }
  });
  
  // Handle reindex-start event
  eventSource.addEventListener('reindex-start', (e) => {
    try {
      const data = JSON.parse(e.data);
      console.log('[SSE] Reindex started:', data);
      state.isReindexing = true;
      state.reindexProgress = { processed: 0, total: data.files };
      updateReindexStatus(state);
    } catch (err) {
      console.error('[SSE] Error parsing reindex-start event:', err);
    }
  });
  
  // Handle reindex-progress event
  eventSource.addEventListener('reindex-progress', (e) => {
    try {
      const data = JSON.parse(e.data);
      console.log('[SSE] Reindex progress:', data);
      state.reindexProgress = { processed: data.processed, total: data.total };
      updateReindexStatus(state);
    } catch (err) {
      console.error('[SSE] Error parsing reindex-progress event:', err);
    }
  });
  
  // Handle reindex-complete event
  eventSource.addEventListener('reindex-complete', (e) => {
    try {
      const data = JSON.parse(e.data);
      console.log('[SSE] Reindex complete:', data);
      state.isReindexing = false;
      state.indexedAt = Date.now();
      state.reindexProgress = null;
      updateReindexStatus(state);
      
      // Auto-refresh data after reindex
      if (onRefresh) onRefresh();
    } catch (err) {
      console.error('[SSE] Error parsing reindex-complete event:', err);
    }
  });
  
  // Handle file-changed event
  eventSource.addEventListener('file-changed', (e) => {
    try {
      const data = JSON.parse(e.data);
      console.log('[SSE] File changed:', data);
      // Could show a notification or mark UI as stale
    } catch (err) {
      console.error('[SSE] Error parsing file-changed event:', err);
    }
  });
}

// =============================================================================
// STATUS UPDATES
// =============================================================================

/**
 * Update connection status indicator.
 * @param {boolean} connected - Is connected
 * @param {boolean|null} daemon - Daemon status
 */
export function updateConnectionStatus(connected, daemon = null) {
  const indicator = document.getElementById('status-connection');
  if (!indicator) return;
  
  if (connected) {
    if (daemon === false) {
      indicator.innerHTML = '<span class="status-dot status-warning">●</span> no daemon';
      indicator.title = 'Connected to web server, but daemon not running';
    } else {
      indicator.innerHTML = '<span class="status-dot status-ok">●</span> connected';
      indicator.title = 'Connected to daemon';
    }
  } else {
    indicator.innerHTML = '<span class="status-dot status-error">○</span> disconnected';
    indicator.title = 'Disconnected from server';
  }
}

/**
 * Update reindex status in UI.
 * @param {Object} state - App state
 * @param {Function} updateIndexedTime - Timer update function
 */
export function updateReindexStatus(state, updateIndexedTime = null) {
  const indexEl = document.getElementById('status-index');
  if (!indexEl) return;
  
  if (state.isReindexing) {
    const progress = state.reindexProgress;
    if (progress && progress.total > 0) {
      indexEl.innerHTML = `<span class="reindexing">reindexing... (${progress.processed}/${progress.total})</span>`;
    } else {
      indexEl.innerHTML = '<span class="reindexing">reindexing...</span>';
    }
    indexEl.classList.add('pulsing');
  } else {
    indexEl.classList.remove('pulsing');
    if (updateIndexedTime) updateIndexedTime();
  }
}
