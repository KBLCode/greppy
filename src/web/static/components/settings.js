/**
 * Settings Component
 *
 * Enhanced settings modal with theme, density, graph, data, and streamer mode controls.
 * Persists settings to localStorage and server.
 *
 * @module components/settings
 */

// =============================================================================
// CONSTANTS
// =============================================================================

const STORAGE_KEY = 'greppy-settings';

const DEFAULT_SETTINGS = {
  // Display
  theme: 'dark',
  density: 'comfortable',
  fontSize: 14,
  
  // Graph
  maxGraphNodes: 500,
  animation: true,
  showLabels: true,
  
  // Data
  pageSize: 100,
  autoRefresh: false,
  
  // Streamer Mode
  streamerMode: false,
  hiddenPatterns: ['.env*', '*secret*', '*credential*'],
  
  // Legacy (kept for backwards compatibility)
  showDeadBadges: true,
  showCycleIndicators: true,
  compactMode: false,
  maxListItems: 500
};

const THEMES = [
  { value: 'dark', label: 'Dark' },
  { value: 'light', label: 'Light' },
  { value: 'high-contrast', label: 'High Contrast' }
];

const DENSITIES = [
  { value: 'compact', label: 'Compact' },
  { value: 'comfortable', label: 'Comfortable' },
  { value: 'spacious', label: 'Spacious' }
];

const FONT_SIZES = [
  { value: 12, label: '12px' },
  { value: 14, label: '14px' },
  { value: 16, label: '16px' },
  { value: 18, label: '18px' }
];

const PAGE_SIZES = [
  { value: 50, label: '50' },
  { value: 100, label: '100' },
  { value: 200, label: '200' },
  { value: 500, label: '500' }
];

const KEYBOARD_SHORTCUTS = [
  { key: 'g', desc: 'Graph view' },
  { key: 'l', desc: 'List view' },
  { key: 's', desc: 'Stats view' },
  { key: 't', desc: 'Tree view' },
  { key: 'd', desc: 'Data tables' },
  { key: '/', desc: 'Focus search' },
  { key: '?', desc: 'Show help' },
  { key: 'Esc', desc: 'Close panel' },
  { key: 'Ctrl+,', desc: 'Settings' }
];

// =============================================================================
// STATE
// =============================================================================

let currentSettings = { ...DEFAULT_SETTINGS };
let onSettingsChange = null;

// =============================================================================
// STORAGE
// =============================================================================

/**
 * Load settings from localStorage.
 * @returns {Object} Merged settings
 */
function loadSettings() {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      currentSettings = { ...DEFAULT_SETTINGS, ...parsed };
    }
  } catch (err) {
    console.error('Failed to load settings:', err);
  }
  return currentSettings;
}

/**
 * Save settings to localStorage.
 * @param {Object} settings - Settings to save
 */
function saveSettings(settings) {
  try {
    currentSettings = { ...currentSettings, ...settings };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(currentSettings));
  } catch (err) {
    console.error('Failed to save settings:', err);
  }
}

// =============================================================================
// THEME APPLICATION
// =============================================================================

/**
 * Apply theme to document.
 * @param {string} theme - Theme name
 */
function applyTheme(theme) {
  document.body.setAttribute('data-theme', theme);
}

/**
 * Apply density to document.
 * @param {string} density - Density name
 */
function applyDensity(density) {
  document.body.setAttribute('data-density', density);
}

/**
 * Apply font size to document.
 * @param {number} size - Font size in pixels
 */
function applyFontSize(size) {
  document.documentElement.style.setProperty('--text-base', `${size}px`);
  document.documentElement.style.setProperty('--text-sm', `${size - 1}px`);
  document.documentElement.style.setProperty('--text-xs', `${size - 2}px`);
  document.documentElement.style.setProperty('--text-lg', `${size + 1}px`);
  document.documentElement.style.setProperty('--text-xl', `${size + 2}px`);
}

/**
 * Apply all settings to document.
 * @param {Object} settings - Settings object
 */
function applyAllSettings(settings) {
  applyTheme(settings.theme);
  applyDensity(settings.density);
  applyFontSize(settings.fontSize);
  
  // Streamer mode
  document.body.classList.toggle('streamer-mode', settings.streamerMode);
  
  // Compact mode (legacy)
  document.body.classList.toggle('compact-mode', settings.compactMode);
}

// =============================================================================
// MODAL RENDERING
// =============================================================================

/**
 * Render a select dropdown.
 * @param {string} id - Element ID
 * @param {Array} options - Options array
 * @param {*} value - Current value
 * @returns {string} HTML string
 */
function renderSelect(id, options, value) {
  return `
    <select id="${id}" class="settings-select">
      ${options.map(opt => `
        <option value="${opt.value}" ${opt.value === value ? 'selected' : ''}>
          ${opt.label}
        </option>
      `).join('')}
    </select>
  `;
}

/**
 * Render a toggle switch.
 * @param {string} id - Element ID
 * @param {boolean} checked - Current state
 * @returns {string} HTML string
 */
function renderToggle(id, checked) {
  return `
    <label class="toggle">
      <input type="checkbox" id="${id}" ${checked ? 'checked' : ''}>
      <span class="toggle-slider"></span>
    </label>
  `;
}

/**
 * Render the enhanced settings modal content.
 * @returns {string} HTML string
 */
function renderSettingsModal() {
  const s = currentSettings;
  
  return `
    <div class="modal-header">
      <span class="modal-title">SETTINGS</span>
      <button id="settings-close" class="btn-icon" title="Close (Esc)">×</button>
    </div>
    <div class="modal-body">
      <!-- DISPLAY -->
      <div class="settings-section">
        <div class="settings-label">DISPLAY</div>
        <div class="settings-row">
          <span class="settings-field-label">Theme</span>
          ${renderSelect('setting-theme', THEMES, s.theme)}
        </div>
        <div class="settings-row">
          <span class="settings-field-label">Density</span>
          ${renderSelect('setting-density', DENSITIES, s.density)}
        </div>
        <div class="settings-row">
          <span class="settings-field-label">Font Size</span>
          ${renderSelect('setting-font-size', FONT_SIZES, s.fontSize)}
        </div>
      </div>

      <!-- GRAPH -->
      <div class="settings-section">
        <div class="settings-label">GRAPH</div>
        <div class="settings-row">
          <span class="settings-field-label">Max Nodes</span>
          <input type="number" id="setting-max-nodes" class="settings-input" 
                 min="50" max="2000" value="${s.maxGraphNodes}">
        </div>
        <div class="settings-row">
          <span class="settings-field-label">Animation</span>
          ${renderToggle('setting-animation', s.animation)}
        </div>
        <div class="settings-row">
          <span class="settings-field-label">Labels</span>
          ${renderToggle('setting-labels', s.showLabels)}
        </div>
      </div>

      <!-- DATA -->
      <div class="settings-section">
        <div class="settings-label">DATA</div>
        <div class="settings-row">
          <span class="settings-field-label">Page Size</span>
          ${renderSelect('setting-page-size', PAGE_SIZES, s.pageSize)}
        </div>
        <div class="settings-row">
          <span class="settings-field-label">Auto-refresh</span>
          ${renderToggle('setting-auto-refresh', s.autoRefresh)}
          <span class="settings-hint-inline">refresh on file changes</span>
        </div>
      </div>

      <!-- STREAMER MODE -->
      <div class="settings-section">
        <div class="settings-label">STREAMER MODE</div>
        <div class="settings-row">
          <span class="settings-field-label">Enabled</span>
          ${renderToggle('setting-streamer-mode', s.streamerMode)}
        </div>
        <div class="settings-row settings-row-full">
          <span class="settings-field-label">Hide Patterns</span>
          <input type="text" id="setting-hidden-patterns" class="settings-input-wide" 
                 value="${s.hiddenPatterns.join(', ')}" 
                 placeholder=".env*, *secret*, *credential*">
        </div>
        <div class="settings-hint">Comma-separated glob patterns for files to blur</div>
      </div>

      <!-- KEYBOARD SHORTCUTS -->
      <div class="settings-section">
        <div class="settings-label">KEYBOARD SHORTCUTS</div>
        <div class="shortcuts-grid">
          ${KEYBOARD_SHORTCUTS.map(sc => `
            <div class="shortcut-item">
              <kbd class="shortcut-key">${sc.key}</kbd>
              <span class="shortcut-desc">${sc.desc}</span>
            </div>
          `).join('')}
        </div>
      </div>
    </div>
    <div class="modal-footer">
      <button id="settings-reset" class="btn">Reset</button>
      <button id="settings-save" class="btn btn-primary">Save</button>
    </div>
  `;
}

// =============================================================================
// EVENT HANDLERS
// =============================================================================

/**
 * Collect settings from form elements.
 * @returns {Object} Settings object
 */
function collectSettingsFromForm() {
  return {
    // Display
    theme: document.getElementById('setting-theme')?.value ?? 'dark',
    density: document.getElementById('setting-density')?.value ?? 'comfortable',
    fontSize: parseInt(document.getElementById('setting-font-size')?.value ?? '14', 10),
    
    // Graph
    maxGraphNodes: parseInt(document.getElementById('setting-max-nodes')?.value ?? '500', 10),
    animation: document.getElementById('setting-animation')?.checked ?? true,
    showLabels: document.getElementById('setting-labels')?.checked ?? true,
    
    // Data
    pageSize: parseInt(document.getElementById('setting-page-size')?.value ?? '100', 10),
    autoRefresh: document.getElementById('setting-auto-refresh')?.checked ?? false,
    
    // Streamer Mode
    streamerMode: document.getElementById('setting-streamer-mode')?.checked ?? false,
    hiddenPatterns: (document.getElementById('setting-hidden-patterns')?.value ?? '')
      .split(',')
      .map(p => p.trim())
      .filter(p => p.length > 0),
    
    // Legacy
    showDeadBadges: currentSettings.showDeadBadges,
    showCycleIndicators: currentSettings.showCycleIndicators,
    compactMode: currentSettings.compactMode,
    maxListItems: currentSettings.maxListItems
  };
}

/**
 * Open settings modal.
 */
function openSettingsModal() {
  const modal = document.getElementById('settings-modal');
  if (!modal) return;
  
  // Render fresh content
  modal.querySelector('.modal-content').innerHTML = renderSettingsModal();
  
  // Show modal
  modal.classList.remove('hidden');
  
  // Setup event handlers
  setupModalHandlers(modal);
}

/**
 * Close settings modal.
 */
function closeSettingsModal() {
  const modal = document.getElementById('settings-modal');
  if (modal) {
    modal.classList.add('hidden');
  }
}

/**
 * Setup modal event handlers.
 * @param {HTMLElement} modal - Modal element
 */
function setupModalHandlers(modal) {
  // Close button
  modal.querySelector('#settings-close')?.addEventListener('click', closeSettingsModal);
  
  // Backdrop click
  modal.querySelector('.modal-backdrop')?.addEventListener('click', closeSettingsModal);
  
  // Save button
  modal.querySelector('#settings-save')?.addEventListener('click', () => {
    const newSettings = collectSettingsFromForm();
    saveSettings(newSettings);
    applyAllSettings(newSettings);
    
    if (onSettingsChange) {
      onSettingsChange(newSettings);
    }
    
    closeSettingsModal();
  });
  
  // Reset button
  modal.querySelector('#settings-reset')?.addEventListener('click', () => {
    if (confirm('Reset all settings to defaults?')) {
      currentSettings = { ...DEFAULT_SETTINGS };
      saveSettings(currentSettings);
      applyAllSettings(currentSettings);
      
      // Re-render modal with default values
      modal.querySelector('.modal-content').innerHTML = renderSettingsModal();
      setupModalHandlers(modal);
    }
  });
  
  // Live preview for theme/density/font
  modal.querySelector('#setting-theme')?.addEventListener('change', (e) => {
    applyTheme(e.target.value);
  });
  
  modal.querySelector('#setting-density')?.addEventListener('change', (e) => {
    applyDensity(e.target.value);
  });
  
  modal.querySelector('#setting-font-size')?.addEventListener('change', (e) => {
    applyFontSize(parseInt(e.target.value, 10));
  });
}

// =============================================================================
// INITIALIZATION
// =============================================================================

/**
 * Initialize settings component.
 * @param {Function} onChange - Callback when settings change
 */
export function initSettings(onChange) {
  onSettingsChange = onChange;
  
  // Load and apply settings
  loadSettings();
  applyAllSettings(currentSettings);
  
  // Setup settings button
  const settingsBtn = document.getElementById('settings-btn');
  if (settingsBtn) {
    settingsBtn.addEventListener('click', openSettingsModal);
  }
  
  // Keyboard shortcut
  document.addEventListener('keydown', (e) => {
    if (e.key === ',' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      openSettingsModal();
    }
    if (e.key === 'Escape') {
      closeSettingsModal();
    }
  });
}

/**
 * Get current settings.
 * @returns {Object} Current settings
 */
export function getSettings() {
  return { ...currentSettings };
}

/**
 * Update settings programmatically.
 * @param {Object} updates - Settings to update
 */
export function updateSettings(updates) {
  currentSettings = { ...currentSettings, ...updates };
  saveSettings(currentSettings);
  applyAllSettings(currentSettings);
  
  if (onSettingsChange) {
    onSettingsChange(currentSettings);
  }
}

export { openSettingsModal, closeSettingsModal };
