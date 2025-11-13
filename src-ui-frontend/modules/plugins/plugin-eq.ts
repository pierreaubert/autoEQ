// EQ Plugin
// Parametric equalizer with visual frequency response

import { BasePlugin } from './plugin-base';
import { PluginMenubar } from './plugin-menubar';
import type { PluginMetadata, PluginConfig } from './plugin-types';
import { invoke } from '@tauri-apps/api/core';

export interface FilterParam {
  filter_type: string;  // "Peak", "Lowshelf", "Highshelf", etc.
  frequency: number;
  q: number;
  gain: number;
  enabled: boolean;
}

// Filter type definitions
const FILTER_TYPES = {
  Peak: { label: 'Peak', shortName: 'PK', icon: '○' },
  Lowpass: { label: 'Low Pass', shortName: 'LP', icon: '╲' },
  Highpass: { label: 'High Pass', shortName: 'HP', icon: '╱' },
  Bandpass: { label: 'Band Pass', shortName: 'BP', icon: '∩' },
  Notch: { label: 'Notch', shortName: 'NO', icon: 'V' },
  Lowshelf: { label: 'Low Shelf', shortName: 'LS', icon: '⎣' },
  Highshelf: { label: 'High Shelf', shortName: 'HS', icon: '⎤' },
};

/**
 * EQ Plugin
 * Visual parametric equalizer
 */
export class EQPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: 'eq-plugin',
    name: 'SotF: EQ',
    category: 'eq',
    version: '1.0.0',
  };

  // UI components
  private menubar: PluginMenubar | null = null;

  // UI elements
  private eqCanvas: HTMLCanvasElement | null = null;
  private eqCtx: CanvasRenderingContext2D | null = null;
  private filterTable: HTMLElement | null = null;
  private filterPopup: HTMLElement | null = null;

  // EQ state
  private filters: FilterParam[] = [
    { filter_type: 'Peak', frequency: 100, q: 1.0, gain: 0, enabled: true },
    { filter_type: 'Peak', frequency: 1000, q: 1.0, gain: 0, enabled: true },
    { filter_type: 'Peak', frequency: 10000, q: 1.0, gain: 0, enabled: true },
  ];
  private selectedFilterIndex: number = -1;

  // Response data
  private eqResponseData: any = null;
  private eqResponseDebounceTimer: number | null = null;

  // Graph constants
  private readonly MIN_FREQ = 20;
  private readonly MAX_FREQ = 20000;
  private readonly FREQ_POINTS = 256;

  // Dynamic Y-axis range
  private minGain = -18;
  private maxGain = 3;

  // Interaction state
  private isDragging = false;
  private dragStartX = 0;
  private dragStartY = 0;

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="eq-plugin ${standalone ? 'standalone' : 'embedded'}">
        ${standalone ? '<div class="eq-menubar-container"></div>' : ''}
        <div class="eq-content">
          <!-- Visual EQ Graph -->
          <div class="eq-graph-container">
            <canvas class="eq-graph-canvas" width="600" height="300"></canvas>
          </div>

          <!-- Filter Table -->
          <div class="eq-table-container">
            <table class="eq-table"></table>
          </div>
        </div>

        <!-- Filter Popup (hidden by default) -->
        <div class="eq-filter-popup" style="display: none;">
          <div class="popup-header">
            <span class="popup-title"></span>
            <button class="popup-close">×</button>
          </div>
          <div class="popup-body"></div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector('.eq-menubar-container') as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name);
      }
    }

    // Cache elements
    this.eqCanvas = this.container.querySelector('.eq-graph-canvas') as HTMLCanvasElement;
    this.eqCtx = this.eqCanvas?.getContext('2d') || null;
    this.filterTable = this.container.querySelector('.eq-table') as HTMLElement;
    this.filterPopup = this.container.querySelector('.eq-filter-popup') as HTMLElement;

    // Setup
    this.setupCanvas();
    this.renderFilterTable();
    this.attachEventListeners();
    this.computeEQResponse();
  }

  /**
   * Setup canvas
   */
  private setupCanvas(): void {
    if (!this.eqCanvas) return;

    // Force a reflow to get accurate dimensions
    const rect = this.eqCanvas.getBoundingClientRect();
    const width = Math.floor(rect.width) || 600;
    const height = Math.floor(rect.height) || 300;

    const dpr = window.devicePixelRatio || 1;

    // Set canvas resolution accounting for device pixel ratio
    this.eqCanvas.width = width * dpr;
    this.eqCanvas.height = height * dpr;

    // Get context and scale
    this.eqCtx = this.eqCanvas.getContext('2d');
    if (this.eqCtx) {
      this.eqCtx.scale(dpr, dpr);

      console.log('[EQPlugin] Canvas setup:', {
        cssWidth: width,
        cssHeight: height,
        canvasWidth: this.eqCanvas.width,
        canvasHeight: this.eqCanvas.height,
        dpr
      });

      // Draw initial graph
      this.drawEQGraph();
    }
  }

  /**
   * Render filter table
   */
  private renderFilterTable(): void {
    if (!this.filterTable) return;

    const headers = `
      <tr>
        <th>Type</th>
        <th>Enabled</th>
        <th>Freq (Hz)</th>
        <th>Gain (dB)</th>
        <th>Q</th>
        <th>Actions</th>
      </tr>
    `;

    const rows = this.filters.map((filter, index) => `
      <tr data-filter-index="${index}" class="${index === this.selectedFilterIndex ? 'selected' : ''}">
        <td>
          <select class="filter-type-select" data-index="${index}">
            ${Object.entries(FILTER_TYPES).map(([type, config]) =>
              `<option value="${type}" ${filter.filter_type === type ? 'selected' : ''}>${config.label}</option>`
            ).join('')}
          </select>
        </td>
        <td>
          <input type="checkbox" class="filter-enabled" data-index="${index}" ${filter.enabled ? 'checked' : ''} />
        </td>
        <td>
          <input type="number" class="filter-frequency" data-index="${index}" value="${filter.frequency.toFixed(1)}" min="20" max="20000" step="1" />
        </td>
        <td>
          <input type="number" class="filter-gain" data-index="${index}" value="${filter.gain.toFixed(2)}" step="0.1" />
        </td>
        <td>
          <input type="number" class="filter-q" data-index="${index}" value="${filter.q.toFixed(2)}" min="0.1" max="3.0" step="0.1" />
        </td>
        <td>
          <button class="filter-edit-btn" data-index="${index}" title="Edit">✎</button>
          <button class="filter-remove-btn" data-index="${index}" title="Remove">×</button>
        </td>
      </tr>
    `).join('');

    this.filterTable.innerHTML = `
      <thead>${headers}</thead>
      <tbody>${rows}</tbody>
      <tfoot>
        <tr>
          <td colspan="6">
            <button class="filter-add-btn">+ Add Filter</button>
          </td>
        </tr>
      </tfoot>
    `;

    this.attachTableEventListeners();
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    if (!this.eqCanvas) return;

    // Canvas interactions
    this.eqCanvas.addEventListener('mousedown', (e) => this.handleCanvasMouseDown(e));
    this.eqCanvas.addEventListener('mousemove', (e) => this.handleCanvasMouseMove(e));
    this.eqCanvas.addEventListener('mouseup', () => this.handleCanvasMouseUp());
    this.eqCanvas.addEventListener('mouseleave', () => this.handleCanvasMouseUp());

    // Popup close
    const closeBtn = this.filterPopup?.querySelector('.popup-close') as HTMLButtonElement;
    if (closeBtn) {
      closeBtn.addEventListener('click', () => this.hideFilterPopup());
    }
  }

  /**
   * Attach table event listeners
   */
  private attachTableEventListeners(): void {
    if (!this.filterTable) return;

    // Type change
    this.filterTable.querySelectorAll('.filter-type-select').forEach((select) => {
      select.addEventListener('change', (e) => {
        const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
        this.filters[index].filter_type = (e.target as HTMLSelectElement).value;
        this.onFilterChange();
      });
    });

    // Enabled toggle
    this.filterTable.querySelectorAll('.filter-enabled').forEach((checkbox) => {
      checkbox.addEventListener('change', (e) => {
        const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
        this.filters[index].enabled = (e.target as HTMLInputElement).checked;
        this.onFilterChange();
      });
    });

    // Parameter inputs
    ['frequency', 'gain', 'q'].forEach((param) => {
      this.filterTable?.querySelectorAll(`.filter-${param}`).forEach((input) => {
        input.addEventListener('input', (e) => {
          const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
          const value = parseFloat((e.target as HTMLInputElement).value);
          (this.filters[index] as any)[param] = value;
          this.selectedFilterIndex = index;
          this.onFilterChange();
        });
      });
    });

    // Edit button
    this.filterTable.querySelectorAll('.filter-edit-btn').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
        this.showFilterPopup(index);
      });
    });

    // Remove button
    this.filterTable.querySelectorAll('.filter-remove-btn').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
        this.removeFilter(index);
      });
    });

    // Add button
    const addBtn = this.filterTable.querySelector('.filter-add-btn') as HTMLButtonElement;
    if (addBtn) {
      addBtn.addEventListener('click', () => this.addFilter());
    }
  }

  /**
   * Add a filter
   */
  private addFilter(): void {
    this.filters.push({
      filter_type: 'Peak',
      frequency: 1000,
      q: 1.0,
      gain: 0,
      enabled: true,
    });
    this.renderFilterTable();
    this.onFilterChange();
  }

  /**
   * Remove a filter
   */
  private removeFilter(index: number): void {
    if (this.filters.length <= 1) {
      console.warn('[EQPlugin] Cannot remove last filter');
      return;
    }
    this.filters.splice(index, 1);
    if (this.selectedFilterIndex >= this.filters.length) {
      this.selectedFilterIndex = this.filters.length - 1;
    }
    this.renderFilterTable();
    this.onFilterChange();
  }

  /**
   * Handle filter change
   */
  private onFilterChange(): void {
    this.requestEQResponseUpdate();
    this.drawEQGraph();
    this.updateParameter('filters', this.filters);
  }

  /**
   * Compute EQ response
   */
  private async computeEQResponse(): Promise<void> {
    if (this.filters.length === 0) {
      this.eqResponseData = null;
      this.drawEQGraph();
      return;
    }

    const logMin = Math.log10(this.MIN_FREQ);
    const logMax = Math.log10(this.MAX_FREQ);
    const frequencies: number[] = [];
    for (let i = 0; i < this.FREQ_POINTS; i++) {
      const logFreq = logMin + (logMax - logMin) * (i / (this.FREQ_POINTS - 1));
      frequencies.push(Math.pow(10, logFreq));
    }

    try {
      const result = await invoke('compute_eq_response', {
        filters: this.filters,
        sampleRate: 48000,
        frequencies,
      });

      this.eqResponseData = result;
      this.computeDynamicYAxisRange();
      this.drawEQGraph();
    } catch (error) {
      console.error('[EQPlugin] Failed to compute response:', error);
    }
  }

  /**
   * Request EQ response update (debounced)
   */
  private requestEQResponseUpdate(): void {
    if (this.eqResponseDebounceTimer) {
      clearTimeout(this.eqResponseDebounceTimer);
    }

    this.eqResponseDebounceTimer = window.setTimeout(() => {
      this.computeEQResponse();
      this.eqResponseDebounceTimer = null;
    }, 100);
  }

  /**
   * Compute dynamic Y-axis range
   */
  private computeDynamicYAxisRange(): void {
    let min = Infinity;
    let max = -Infinity;

    // Include filter gains
    this.filters.forEach((filter) => {
      if (filter.enabled) {
        min = Math.min(min, filter.gain);
        max = Math.max(max, filter.gain);
      }
    });

    // Include response data
    if (this.eqResponseData?.combined_response) {
      this.eqResponseData.combined_response.forEach((gain: number) => {
        min = Math.min(min, gain);
        max = Math.max(max, gain);
      });
    }

    // Default range
    if (min === Infinity || max === -Infinity) {
      min = -18;
      max = 3;
    }

    // Add padding
    const padding = 1;
    const minRange = 6;
    const range = max - min;

    if (range < minRange) {
      const center = (min + max) / 2;
      min = center - minRange / 2;
      max = center + minRange / 2;
    }

    this.minGain = min - padding;
    this.maxGain = max + padding;
  }

  /**
   * Draw EQ graph
   */
  private drawEQGraph(): void {
    if (!this.eqCanvas || !this.eqCtx) {
      console.warn('[EQPlugin] Cannot draw: canvas or context missing');
      return;
    }

    const dpr = window.devicePixelRatio || 1;
    const width = this.eqCanvas.width / dpr;
    const height = this.eqCanvas.height / dpr;

    console.log('[EQPlugin] Drawing EQ graph:', {
      width,
      height,
      hasResponseData: !!this.eqResponseData,
      filterCount: this.filters.length
    });

    // Clear
    this.eqCtx.fillStyle = '#1a1a1a';
    this.eqCtx.fillRect(0, 0, width, height);

    // Draw grid
    this.drawGrid(width, height);

    // Draw response curve
    if (this.eqResponseData) {
      this.drawResponse(width, height);
    }

    // Draw filter handles
    this.drawFilterHandles(width, height);
  }

  /**
   * Draw grid
   */
  private drawGrid(width: number, height: number): void {
    if (!this.eqCtx) return;

    this.eqCtx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    this.eqCtx.lineWidth = 1;

    // Vertical frequency lines
    const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqMarkers.forEach((freq) => {
      const x = this.freqToX(freq, width);
      this.eqCtx!.beginPath();
      this.eqCtx!.moveTo(x, 0);
      this.eqCtx!.lineTo(x, height - 30);
      this.eqCtx!.stroke();
    });

    // Horizontal gain lines
    for (let gain = Math.ceil(this.minGain); gain <= this.maxGain; gain += 3) {
      const y = this.gainToY(gain, height);
      this.eqCtx.strokeStyle = gain === 0 ? 'rgba(255, 255, 255, 0.3)' : 'rgba(255, 255, 255, 0.1)';
      this.eqCtx.beginPath();
      this.eqCtx.moveTo(60, y);
      this.eqCtx.lineTo(width - 20, y);
      this.eqCtx.stroke();
    }
  }

  /**
   * Draw response curve
   */
  private drawResponse(width: number, height: number): void {
    if (!this.eqCtx || !this.eqResponseData) return;

    const { frequencies, combined_response } = this.eqResponseData;

    // Draw combined response
    this.eqCtx.strokeStyle = 'rgba(100, 200, 255, 0.8)';
    this.eqCtx.lineWidth = 2;
    this.eqCtx.beginPath();

    frequencies.forEach((freq: number, i: number) => {
      const x = this.freqToX(freq, width);
      const y = this.gainToY(combined_response[i], height);

      if (i === 0) {
        this.eqCtx!.moveTo(x, y);
      } else {
        this.eqCtx!.lineTo(x, y);
      }
    });

    this.eqCtx.stroke();
  }

  /**
   * Draw filter handles
   */
  private drawFilterHandles(width: number, height: number): void {
    if (!this.eqCtx) return;

    this.filters.forEach((filter, idx) => {
      if (!filter.enabled) return;

      const x = this.freqToX(filter.frequency, width);
      const y = this.gainToY(filter.gain, height);
      const isSelected = idx === this.selectedFilterIndex;

      // Draw Q bar
      this.eqCtx!.strokeStyle = isSelected ? 'rgba(255, 200, 100, 0.8)' : 'rgba(255, 255, 255, 0.4)';
      this.eqCtx!.lineWidth = isSelected ? 3 : 2;

      const barWidth = 40 / filter.q;
      this.eqCtx!.beginPath();
      this.eqCtx!.moveTo(x - barWidth / 2, y);
      this.eqCtx!.lineTo(x + barWidth / 2, y);
      this.eqCtx!.stroke();

      // Draw handle point
      this.eqCtx!.fillStyle = isSelected ? 'rgba(255, 200, 100, 1)' : 'rgba(255, 255, 255, 0.8)';
      this.eqCtx!.beginPath();
      this.eqCtx!.arc(x, y, isSelected ? 6 : 4, 0, Math.PI * 2);
      this.eqCtx!.fill();

      // Selection ring
      if (isSelected) {
        this.eqCtx!.strokeStyle = 'rgba(255, 200, 100, 0.6)';
        this.eqCtx!.lineWidth = 2;
        this.eqCtx!.beginPath();
        this.eqCtx!.arc(x, y, 10, 0, Math.PI * 2);
        this.eqCtx!.stroke();
      }
    });
  }

  /**
   * Handle canvas mouse down
   */
  private handleCanvasMouseDown(e: MouseEvent): void {
    if (!this.eqCanvas) return;

    const rect = this.eqCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const dpr = window.devicePixelRatio || 1;
    const width = this.eqCanvas.width / dpr;
    const height = this.eqCanvas.height / dpr;

    const clickedFreq = this.xToFreq(x, width);
    const clickedGain = this.yToGain(y, height);

    console.log('[EQPlugin] Mouse down:', { x, y, clickedFreq, clickedGain });

    // Find closest filter
    let closestIdx = -1;
    let minDist = Infinity;

    this.filters.forEach((filter, idx) => {
      if (!filter.enabled) return;

      // Calculate distance in both frequency and gain
      const freqDist = Math.abs(Math.log10(filter.frequency) - Math.log10(clickedFreq));
      const gainDist = Math.abs(filter.gain - clickedGain) / 10; // Normalize gain distance
      const dist = Math.sqrt(freqDist * freqDist + gainDist * gainDist);

      if (dist < minDist) {
        minDist = dist;
        closestIdx = idx;
      }
    });

    // Only select if reasonably close (threshold)
    if (closestIdx >= 0 && minDist < 0.5) {
      this.selectedFilterIndex = closestIdx;
      this.isDragging = true;
      this.dragStartX = x;
      this.dragStartY = y;
      console.log('[EQPlugin] Selected filter:', closestIdx, this.filters[closestIdx]);
      this.drawEQGraph();
      this.renderFilterTable();
    } else {
      console.log('[EQPlugin] No filter close enough, minDist:', minDist);
    }
  }

  /**
   * Handle canvas mouse move
   */
  private handleCanvasMouseMove(e: MouseEvent): void {
    if (!this.isDragging || !this.eqCanvas) return;

    const rect = this.eqCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const dpr = window.devicePixelRatio || 1;
    const width = this.eqCanvas.width / dpr;
    const height = this.eqCanvas.height / dpr;

    const filter = this.filters[this.selectedFilterIndex];
    if (!filter) return;

    // Update frequency (left/right)
    const newFreq = Math.max(this.MIN_FREQ, Math.min(this.MAX_FREQ, this.xToFreq(x, width)));

    // Update gain (up/down)
    const newGain = Math.max(this.minGain, Math.min(this.maxGain, this.yToGain(y, height)));

    console.log('[EQPlugin] Dragging filter:', {
      filterIndex: this.selectedFilterIndex,
      oldFreq: filter.frequency,
      newFreq,
      oldGain: filter.gain,
      newGain
    });

    filter.frequency = newFreq;
    filter.gain = newGain;

    this.onFilterChange();
  }

  /**
   * Handle canvas mouse up
   */
  private handleCanvasMouseUp(): void {
    this.isDragging = false;
  }

  /**
   * Show filter popup
   */
  private showFilterPopup(index: number): void {
    if (!this.filterPopup) return;

    const filter = this.filters[index];
    const filterType = FILTER_TYPES[filter.filter_type as keyof typeof FILTER_TYPES];

    const title = this.filterPopup.querySelector('.popup-title') as HTMLElement;
    const body = this.filterPopup.querySelector('.popup-body') as HTMLElement;

    title.textContent = `${filterType.shortName} | ${filterType.icon}`;

    body.innerHTML = `
      <div class="popup-field">
        <label>Frequency:</label>
        <input type="number" id="popup-freq" value="${filter.frequency}" min="20" max="20000" step="1" />
      </div>
      <div class="popup-field">
        <label>Gain:</label>
        <input type="number" id="popup-gain" value="${filter.gain}" step="0.1" />
      </div>
      <div class="popup-field">
        <label>Q:</label>
        <input type="number" id="popup-q" value="${filter.q}" min="0.1" max="3.0" step="0.1" />
      </div>
    `;

    this.filterPopup.style.display = 'block';

    // Attach input listeners
    ['freq', 'gain', 'q'].forEach((param) => {
      const input = body.querySelector(`#popup-${param}`) as HTMLInputElement;
      if (input) {
        input.addEventListener('input', (e) => {
          const value = parseFloat((e.target as HTMLInputElement).value);
          const key = param === 'freq' ? 'frequency' : param;
          (filter as any)[key] = value;
          this.onFilterChange();
          this.renderFilterTable();
        });
      }
    });
  }

  /**
   * Hide filter popup
   */
  private hideFilterPopup(): void {
    if (this.filterPopup) {
      this.filterPopup.style.display = 'none';
    }
  }

  /**
   * Coordinate conversions
   */
  private freqToX(freq: number, width: number): number {
    const logMin = Math.log10(this.MIN_FREQ);
    const logMax = Math.log10(this.MAX_FREQ);
    const logFreq = Math.log10(Math.max(this.MIN_FREQ, Math.min(this.MAX_FREQ, freq)));
    const normalized = (logFreq - logMin) / (logMax - logMin);
    return 60 + normalized * (width - 80);
  }

  private xToFreq(x: number, width: number): number {
    const normalized = (x - 60) / (width - 80);
    const logMin = Math.log10(this.MIN_FREQ);
    const logMax = Math.log10(this.MAX_FREQ);
    const logFreq = logMin + normalized * (logMax - logMin);
    return Math.pow(10, logFreq);
  }

  private gainToY(gain: number, height: number): number {
    const range = this.maxGain - this.minGain;
    const normalized = (gain - this.minGain) / range;
    return height - 30 - normalized * (height - 60);
  }

  private yToGain(y: number, height: number): number {
    const range = this.maxGain - this.minGain;
    const normalized = (height - 30 - y) / (height - 60);
    return this.minGain + normalized * range;
  }

  /**
   * Resize handler
   */
  resize(): void {
    this.setupCanvas();
    this.drawEQGraph();
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    if (this.eqResponseDebounceTimer) {
      clearTimeout(this.eqResponseDebounceTimer);
    }

    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    super.destroy();
  }

  /**
   * Get filters
   */
  getFilters(): FilterParam[] {
    return [...this.filters];
  }

  /**
   * Set filters
   */
  setFilters(filters: FilterParam[]): void {
    this.filters = filters;
    this.renderFilterTable();
    this.onFilterChange();
  }
}
