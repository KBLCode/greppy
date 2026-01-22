/**
 * Dropdown Component
 *
 * Custom dropdown with keyboard navigation support.
 *
 * @module components/dropdown
 */

// =============================================================================
// DROPDOWN CLASS
// =============================================================================

/**
 * Custom dropdown component with keyboard navigation.
 */
export class Dropdown {
  /**
   * Create a dropdown.
   * @param {HTMLElement} element - Container element
   * @param {Object} options - Configuration
   * @param {Array} options.options - Dropdown options [{value, label}]
   * @param {string} options.value - Initial value
   * @param {Function} options.onChange - Change callback
   */
  constructor(element, options = {}) {
    this.element = element;
    this.options = options.options || [];
    this.value = options.value || this.options[0]?.value || '';
    this.onChange = options.onChange || (() => {});
    this.render();
    this.bindEvents();
  }
  
  render() {
    const selected = this.options.find(o => o.value === this.value);
    this.element.innerHTML = `
      <div class="dropdown" data-value="${this.value}">
        <button class="dropdown-trigger" type="button">
          <span class="dropdown-value">${selected?.label || this.value}</span>
          <span class="dropdown-arrow">&#9660;</span>
        </button>
        <div class="dropdown-menu">
          ${this.options.map(opt => `
            <div class="dropdown-item ${opt.value === this.value ? 'selected' : ''}" data-value="${opt.value}" tabindex="0">
              <span class="dropdown-check">${opt.value === this.value ? '●' : '○'}</span>
              <span class="dropdown-label">${opt.label}</span>
            </div>
          `).join('')}
        </div>
      </div>
    `;
    this.dropdown = this.element.querySelector('.dropdown');
    this.trigger = this.element.querySelector('.dropdown-trigger');
    this.menu = this.element.querySelector('.dropdown-menu');
  }
  
  bindEvents() {
    this.trigger.addEventListener('click', (e) => {
      e.stopPropagation();
      this.toggle();
    });
    
    this.menu.addEventListener('click', (e) => {
      const item = e.target.closest('.dropdown-item');
      if (item) this.select(item.dataset.value);
    });
    
    this.trigger.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        this.toggle();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        this.open();
      } else if (e.key === 'Escape') {
        this.close();
      }
    });
    
    this.menu.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        this.focusNext();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        this.focusPrev();
      } else if (e.key === 'Enter') {
        const focused = this.menu.querySelector('.dropdown-item:focus');
        if (focused) this.select(focused.dataset.value);
      } else if (e.key === 'Escape') {
        this.close();
        this.trigger.focus();
      }
    });
    
    document.addEventListener('click', () => this.close());
  }
  
  toggle() {
    this.dropdown.classList.contains('open') ? this.close() : this.open();
  }
  
  open() {
    document.querySelectorAll('.dropdown.open').forEach(d => d.classList.remove('open'));
    this.dropdown.classList.add('open');
    const first = this.menu.querySelector('.dropdown-item');
    first?.focus();
  }
  
  close() {
    this.dropdown.classList.remove('open');
  }
  
  select(value) {
    this.value = value;
    this.render();
    this.bindEvents();
    this.onChange(value);
  }
  
  focusNext() {
    const items = [...this.menu.querySelectorAll('.dropdown-item')];
    const idx = items.indexOf(document.activeElement);
    (items[idx + 1] || items[0])?.focus();
  }
  
  focusPrev() {
    const items = [...this.menu.querySelectorAll('.dropdown-item')];
    const idx = items.indexOf(document.activeElement);
    (items[idx - 1] || items[items.length - 1])?.focus();
  }
}
