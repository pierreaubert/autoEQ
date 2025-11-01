// Visual EQ Configuration Module
// Extracted from audio-player.ts to handle all EQ table and graph functionality

import { invoke } from "@tauri-apps/api/core";
import { StreamingManager } from "../audio-manager-streaming";

export interface FilterParam {
  frequency: number;
  q: number;
  gain: number;
  enabled: boolean;
}

export interface ExtendedFilterParam extends FilterParam {
  filter_type: string; // "Peak", "Lowpass", "Highpass", "Bandpass", "Notch", "Lowshelf", "Highshelf"
}

// Filter type options
export const FILTER_TYPES = {
  Peak: { label: "Peak", shortName: "PK", icon: "○" },
  Lowpass: { label: "Low Pass", shortName: "LP", icon: "╲" },
  Highpass: { label: "High Pass", shortName: "HP", icon: "╱" },
  Bandpass: { label: "Band Pass", shortName: "BP", icon: "∩" },
  Notch: { label: "Notch", shortName: "NO", icon: "V" },
  Lowshelf: { label: "Low Shelf", shortName: "LS", icon: "⎣" },
  Highshelf: { label: "High Shelf", shortName: "HS", icon: "⎤" },
};

export interface VisualEQConfigCallbacks {
  onFilterParamsChange?: (filterParams: ExtendedFilterParam[]) => void;
  onEQToggle?: (enabled: boolean) => void;
}

export class VisualEQConfig {
  private container: HTMLElement;
  private instanceId: string;
  private callbacks: VisualEQConfigCallbacks;
  private streamingManager: StreamingManager;

  // EQ Modal elements
  private eqModal: HTMLElement | null = null;
  private eqBackdrop: HTMLElement | null = null;
  private eqModalCloseBtn: HTMLButtonElement | null = null;
  private playbackOptionsContainer: HTMLElement | null = null;
  private eqTableContainer: HTMLElement | null = null;

  // EQ Graph properties
  private eqGraphCanvas: HTMLCanvasElement | null = null;
  private eqGraphCtx: CanvasRenderingContext2D | null = null;
  private selectedFilterIndex: number = -1;
  private isDraggingHandle: boolean = false;
  private dragMode: 'ring' | 'bar' = 'ring';
  private lastMouseY: number = 0;

  // EQ Response data
  private eqResponseData: any = null; // Cached response from backend
  private eqResponseDebounceTimer: number | null = null;

  // EQ Graph constants
  private readonly EQ_GRAPH_MIN_FREQ = 20;
  private readonly EQ_GRAPH_MAX_FREQ = 20000;
  private readonly EQ_GRAPH_MIN_Q = 0.1;
  private readonly EQ_GRAPH_MAX_Q = 3.0;
  private readonly EQ_GRAPH_FREQ_POINTS = 256; // Number of points for response curve

  // EQ Graph dynamic gain range (computed from response data)
  private eqGraphMinGain = -18; // Default: -6 * max_db (3.0)
  private eqGraphMaxGain = 3; // Default: max_db

  // EQ state
  private currentFilterParams: ExtendedFilterParam[] = [
    { frequency: 100, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
    { frequency: 1000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
    { frequency: 10000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
  ];
  private eqEnabled: boolean = true;

  constructor(
    container: HTMLElement,
    instanceId: string,
    streamingManager: StreamingManager,
    callbacks: VisualEQConfigCallbacks = {}
  ) {
    this.container = container;
    this.instanceId = instanceId;
    this.streamingManager = streamingManager;
    this.callbacks = callbacks;
    
    this.createEQModal();
    this.setupEventListeners();
  }

  // ===== MODAL CREATION AND MANAGEMENT =====

  private createEQModal(): void {
    console.log("[EQ Debug] Creating modal element");

    // Check if modal already exists
    const existingModal = document.getElementById(this.instanceId + "-eq-modal");
    if (existingModal) {
      console.log("[EQ Debug] Modal already exists:", existingModal);
      return;
    }

    // Create backdrop
    const backdrop = document.createElement("div");
    backdrop.id = this.instanceId + "-eq-backdrop";
    backdrop.className = "eq-modal-backdrop";

    // Create modal
    const modal = document.createElement("div");
    modal.id = this.instanceId + "-eq-modal";
    modal.className = "eq-modal";
    console.log("[EQ Debug] Modal element created:", modal);
    console.log("[EQ Debug] Modal ID:", modal.id);

    modal.innerHTML = `
      <div class="eq-modal-content">
        <div class="eq-modal-header">
          <h3>EQ Configuration</h3>
          <button type="button" class="eq-modal-close-btn">&times;</button>
        </div>
        <div class="eq-modal-body">
          <div class="playback-options-container"></div>
          <div class="eq-table-container"></div>
        </div>
      </div>
    `;

    // Add to DOM
    document.body.appendChild(backdrop);
    document.body.appendChild(modal);

    console.log("[EQ Debug] Modal and backdrop inserted into body");
    console.log("[EQ Debug] Modal in DOM:", document.contains(modal));
    const foundModal = document.getElementById(this.instanceId + "-eq-modal");
    console.log("[EQ Debug] Can find modal after insertion:", !!foundModal);
  }

  private setupEventListeners(): void {
    // Cache modal elements
    this.eqModal = document.getElementById(this.instanceId + "-eq-modal");
    this.eqBackdrop = document.getElementById(this.instanceId + "-eq-backdrop");

    console.log(
      "[EQ Debug] Modal element lookup ID:",
      this.instanceId + "-eq-modal"
    );
    console.log("[EQ Debug] Modal element found:", this.eqModal);
    console.log("[EQ Debug] Backdrop element found:", this.eqBackdrop);

    if (this.eqModal) {
      this.eqModalCloseBtn = this.eqModal.querySelector(".eq-modal-close-btn");
      this.playbackOptionsContainer = this.eqModal.querySelector(
        ".playback-options-container"
      );
      this.eqTableContainer = this.eqModal.querySelector(".eq-table-container");

      // Cache EQ graph canvas
      this.eqGraphCanvas = this.eqModal.querySelector(".eq-graph-canvas");
      if (this.eqGraphCanvas) {
        this.eqGraphCtx = this.eqGraphCanvas.getContext("2d");
        this.resizeEQGraphCanvas();
      }
    }

    // Modal close events
    this.eqModalCloseBtn?.addEventListener("click", () => this.closeEQModal());
    this.eqBackdrop?.addEventListener("click", () => this.closeEQModal());

    // Close modal when clicking outside
    document.addEventListener("click", (event: MouseEvent) => {
      if (
        this.eqModal &&
        !this.eqModal.contains(event.target as Node) &&
        !this.eqConfigBtn?.contains(event.target as Node)
      ) {
        this.closeEQModal();
      }
    });
  }

  openEQModal(eqConfigBtn: HTMLElement): void {
    console.log("[EQ Debug] Attempting to show modal");
    console.log("[EQ Debug] Current modal state:", {
      exists: !!this.eqModal,
      backdropExists: !!this.eqBackdrop,
      id: this.eqModal?.id,
      className: this.eqModal?.className,
      parentElement: this.eqModal?.parentElement?.tagName,
    });

    if (this.eqModal && this.eqBackdrop && eqConfigBtn) {
      this.renderEQTable();
      
      // Position modal near the button
      const buttonRect = eqConfigBtn.getBoundingClientRect();
      const modalRect = this.eqModal.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      // Calculate position
      let left = buttonRect.left + buttonRect.width / 2 - modalRect.width / 2;
      let top = buttonRect.bottom + 10;

      // Ensure modal stays within viewport
      const maxWidth = Math.min(800, viewportWidth - 40);
      const maxHeight = Math.min(600, viewportHeight - 40);

      if (left < 20) left = 20;
      if (left + maxWidth > viewportWidth - 20) {
        left = viewportWidth - maxWidth - 20;
      }
      if (top + maxHeight > viewportHeight - 20) {
        top = buttonRect.top - maxHeight - 10;
      }

      this.eqModal.style.left = `${left}px`;
      this.eqModal.style.top = `${top}px`;
      this.eqModal.style.width = `${maxWidth}px`;
      this.eqModal.style.height = `${maxHeight}px`;

      console.log("[EQ Debug] Modal positioned at:", {
        left,
        top,
        width: maxWidth,
        height: maxHeight,
      });

      this.eqBackdrop.classList.add("visible");
      this.eqModal.classList.add("visible");

      console.log("[EQ Debug] Modal classes after show:", {
        modal: this.eqModal.className,
        backdrop: this.eqBackdrop.className,
      });

      // Compute and draw EQ graph
      this.computeEQResponse();
    } else {
      console.error(
        "[EQ Debug] Modal, backdrop, or gear button element is null or undefined"
      );
    }
  }

  closeEQModal(): void {
    if (this.eqModal) {
      this.eqModal.classList.remove("visible");
    }
    if (this.eqBackdrop) {
      this.eqBackdrop.classList.remove("visible");
    }
  }

  // ===== EQ TABLE RENDERING =====

  private renderEQTable(): void {
    console.log("[EQ Debug] Rendering playback configuration");

    console.log(
      "[EQ Debug] Playback options container:",
      this.playbackOptionsContainer
    );
    console.log("[EQ Debug] EQ table container:", this.eqTableContainer);
    console.log("[EQ Debug] Current filter params:", this.currentFilterParams);

    if (!this.playbackOptionsContainer || !this.eqTableContainer) {
      console.error("[EQ Debug] Container not found");
      return;
    }

    // Clear existing content
    this.playbackOptionsContainer.innerHTML = "";
    this.eqTableContainer.innerHTML = "";

    // Render EQ table section
    const eqSection = document.createElement("div");
    eqSection.className = "eq-section";

    // Create header
    const header = document.createElement("div");
    header.className = "eq-section-header";
    header.innerHTML = "<h4>Filter Configuration</h4>";

    // Create graph container
    const graphContainer = document.createElement("div");
    graphContainer.className = "eq-graph-container";

    // Create canvas for EQ graph
    const canvas = document.createElement("canvas");
    canvas.className = "eq-graph-canvas";
    canvas.width = 600;
    canvas.height = 300;

    graphContainer.appendChild(canvas);

    // Create table
    const table = document.createElement("table");
    table.className = "eq-table-vertical";

    // Create table header
    const thead = document.createElement("thead");
    thead.innerHTML = `
      <tr class="eq-row">
        <th class="eq-row-label"></th>
        ${this.currentFilterParams
          .map(
            (_, index) => `
            <th data-filter-index="${index}" class="eq-column-header ${index === this.selectedFilterIndex ? "selected" : ""}">
              Filter ${index + 1}
            </th>
          `
          )
          .join("")}
        <th class="eq-row-label"></th>
      </tr>
    `;
    table.appendChild(thead);

    // Create table body
    const tbody = document.createElement("tbody");

    // Filter type row
    const typeRow = document.createElement("tr");
    typeRow.className = "eq-row";
    typeRow.innerHTML = `
      <td class="eq-row-label">Type</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <select data-index="${index}" class="eq-filter-type">
              ${Object.entries(FILTER_TYPES).map(([type, config]) => 
                `<option value="${type}" ${filter.filter_type === type ? "selected" : ""}>${config.label}</option>`
              ).join('')}
            </select>
          </td>
        `
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(typeRow);

    // Enable row
    const enableRow = document.createElement("tr");
    enableRow.className = "eq-row";
    enableRow.innerHTML = `
      <td class="eq-row-label">Enable</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="checkbox" data-index="${index}" class="eq-enabled" ${filter.enabled ? "checked" : ""}>
          </td>
        `
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(enableRow);

    // Frequency row
    const freqRow = document.createElement("tr");
    freqRow.className = "eq-row";
    freqRow.innerHTML = `
      <td class="eq-row-label">Freq (Hz)</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="number" data-index="${index}" class="eq-frequency" value="${filter.frequency.toFixed(1)}" step="1">
          </td>
        `
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(freqRow);

    // Gain row
    const gainRow = document.createElement("tr");
    gainRow.className = "eq-row";
    gainRow.innerHTML = `
      <td class="eq-row-label">Gain (dB)</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="number" data-index="${index}" class="eq-gain" value="${filter.gain.toFixed(2)}" step="0.1">
          </td>
        `
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(gainRow);

    // Q row
    const qRow = document.createElement("tr");
    qRow.className = "eq-row";
    qRow.innerHTML = `
      <td class="eq-row-label">Q</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="number" data-index="${index}" class="eq-q" value="${filter.q.toFixed(2)}" step="0.1">
          </td>
        `
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(qRow);

    table.appendChild(tbody);

    // Assemble the section
    this.eqTableContainer.innerHTML = "";
    eqSection.appendChild(header);
    eqSection.appendChild(graphContainer);
    eqSection.appendChild(table);
    this.eqTableContainer.appendChild(eqSection);

    // Cache canvas and setup graph
    this.eqGraphCanvas = canvas;
    this.eqGraphCtx = canvas.getContext("2d");
    this.resizeEQGraphCanvas();

    // Setup table interactions
    this.setupTableInteractions(table);

    // Initial graph draw
    this.computeEQResponse();
  }

  private setupTableInteractions(table: HTMLTableElement): void {
    // Handle column selection
    table.addEventListener("click", (e) => {
      const target = e.target as HTMLElement;
      const cell = target.closest("td, th");
      if (cell && cell.dataset.filterIndex) {
        const index = parseInt(cell.dataset.filterIndex, 10);
        this.selectedFilterIndex = index;
        this.drawEQGraph();
        this.renderEQTable(); // Re-render to update selection
      }
    });

    // Handle input changes
    table.addEventListener("input", (e) => this.handleEQTableChange(e));
  }

  private handleEQTableChange(e: Event): void {
    const target = e.target as HTMLInputElement | HTMLSelectElement;
    const index = parseInt(target.dataset.index || "", 10);

    if (isNaN(index) || !this.currentFilterParams[index]) return;

    let type = target.className.replace("eq-", "");

    // Handle filter type select separately
    if (type === "filter-type") {
      type = "filter_type";
    }

    let value: any = target.value;

    // Convert numeric values
    if (type === "frequency" || type === "gain" || type === "q") {
      value = parseFloat(value);
    } else if (type === "enabled") {
      value = (target as HTMLInputElement).checked;
    }

    // Update the filter parameter
    (this.currentFilterParams[index] as unknown as Record<
      string,
      any
    >)[type] = value;

    // Select this filter in the graph
    this.selectedFilterIndex = index;

    // Request graph update
    this.requestEQResponseUpdate();

    // Redraw graph
    this.drawEQGraph();

    // Update filter parameters - this will also update the display
    this.updateFilterParams(this.currentFilterParams);
  }

  // ===== FILTER PARAMETER MANAGEMENT =====

  updateFilterParams(filterParams: Partial<ExtendedFilterParam>[]): void {
    this.currentFilterParams = filterParams.map((p) => ({
      enabled: p.enabled ?? true,
      frequency: p.frequency || 0,
      q: p.q || 1,
      gain: p.gain || 0,
      filter_type: p.filter_type || "Peak",
    })) as ExtendedFilterParam[];

    // Recalculate and apply filters
    this.setupEQFilters();

    // Notify callback
    this.callbacks.onFilterParamsChange?.(this.currentFilterParams);
  }

  clearEQFilters(): void {
    this.currentFilterParams = [];
    this.setupEQFilters();
    this.renderEQTable(); // Update table to show no filters
  }

  private setupEQFilters(): void {
    let activeFilterCount = 0;
    let maxPositiveGain = 0;

    // Create new filters from parameters
    this.currentFilterParams.forEach((param) => {
      if (param.enabled) {
        activeFilterCount++;
        if (param.gain > maxPositiveGain) {
          maxPositiveGain = param.gain;
        }
      }
    });

    console.log(
      `Created ${activeFilterCount} EQ filters with gain compensation`,
      maxPositiveGain
    );

    // Notify streaming manager if playing
    if (this.eqEnabled) {
      const filters = this.currentFilterParams
        .filter((p) => p.enabled)
        .map((p) => ({
          frequency: p.frequency,
          q: p.q,
          gain: p.gain,
        }));

      this.streamingManager.updateFilters(filters).catch((error) => {
        console.error("Failed to update filters in real-time:", error);
      });
    }
  }

  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled;

    // Apply EQ changes in real-time if playing
    const filters = enabled
      ? this.currentFilterParams
          .filter((p) => p.enabled)
          .map((p) => ({
            frequency: p.frequency,
            q: p.q,
            gain: p.gain,
          }))
      : [];

    this.streamingManager.updateFilters(filters).catch((error: unknown) => {
      console.error("Failed to update filters:", error);
    });

    console.log(`EQ ${enabled ? "enabled" : "disabled"}`);
    this.callbacks.onEQToggle?.(enabled);
  }

  isEQEnabled(): boolean {
    return this.eqEnabled;
  }

  getFilterParams(): ExtendedFilterParam[] {
    return [...this.currentFilterParams];
  }

  // ===== EQ GRAPH IMPLEMENTATION =====

  private resizeEQGraphCanvas(): void {
    if (!this.eqGraphCanvas) return;
    const container = this.eqGraphCanvas.parentElement;
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const width = Math.max(400, rect.width - 40);
    const height = 300;

    this.eqGraphCanvas.width = width;
    this.eqGraphCanvas.height = height;
    this.drawEQGraph();
  }

  private async computeEQResponse(): Promise<void> {
    if (!this.currentFilterParams || this.currentFilterParams.length === 0) {
      this.eqResponseData = null;
      this.drawEQGraph();
      return;
    }

    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const frequencies: number[] = [];
    for (let i = 0; i < this.EQ_GRAPH_FREQ_POINTS; i++) {
      const logFreq =
        logMin + (logMax - logMin) * (i / (this.EQ_GRAPH_FREQ_POINTS - 1));
      frequencies.push(Math.pow(10, logFreq));
    }

    const filters = this.currentFilterParams.map((f) => ({
      filter_type: f.filter_type || "Peak",
      frequency: f.frequency,
      q: f.q,
      gain: f.gain,
      enabled: f.enabled,
    }));

    console.log("[EQ Graph] Computing response with filters:", filters);
    
    try {
      const result = await invoke("compute_eq_response", {
        filters,
        sampleRate: 48000,
        frequencies,
      });
      
      console.log("[EQ Graph] Response data received:", result);
      this.eqResponseData = result;
      this.drawEQGraph();
    } catch (error) {
      console.error("[EQ Graph] Failed to compute response:", error);
    }
  }

  private requestEQResponseUpdate(): void {
    if (this.eqResponseDebounceTimer) {
      clearTimeout(this.eqResponseDebounceTimer);
    }

    this.eqResponseDebounceTimer = window.setTimeout(() => {
      this.computeEQResponse();
      this.eqResponseDebounceTimer = null;
    }, 100);
  }

  private drawEQGraph(): void {
    if (!this.eqGraphCanvas || !this.eqGraphCtx) {
      console.log("[EQ Graph] Canvas or context not available");
      return;
    }

    const ctx = this.eqGraphCtx;
    const width = this.eqGraphCanvas.width;
    const height = this.eqGraphCanvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Draw background
    ctx.fillStyle = "#1a1a1a";
    ctx.fillRect(0, 0, width, height);

    console.log(
      "[EQ Graph] Drawing graph - canvas:",
      width,
      "x",
      height,
      "hasData:",
      !!this.eqResponseData,
      "selectedFilter:",
      this.selectedFilterIndex
    );

    if (this.eqResponseData) {
      console.log("[EQ Graph] Drawing response curves");
      this.drawGrid(ctx, width, height);
      this.drawFrequencyLabels(ctx, width, height);
      this.drawGainLabels(ctx, width, height);
      this.drawCombinedResponse(ctx, width, height);
      this.drawIndividualResponses(ctx, width, height);
    } else {
      console.log("[EQ Graph] No response data available");
    }

    this.drawFilterHandles(ctx, width, height, true); // isDarkMode = true
  }

  private computeDynamicYAxisRange(): void {
    if (!this.eqResponseData) return;

    let minGain = Infinity;
    let maxGain = -Infinity;

    if (this.eqResponseData.combined_response && Array.isArray(this.eqResponseData.combined_response)) {
      this.eqResponseData.combined_response.forEach((gain: number) => {
        minGain = Math.min(minGain, gain);
        maxGain = Math.max(maxGain, gain);
      });
    }

    if (this.eqResponseData.individual_responses) {
      const responses = this.eqResponseData.individual_responses;
      if (Array.isArray(responses)) {
        responses.forEach((response: number[]) => {
          if (Array.isArray(response)) {
            response.forEach((gain: number) => {
              minGain = Math.min(minGain, gain);
              maxGain = Math.max(maxGain, gain);
            });
          }
        });
      } else if (typeof responses === 'object') {
        Object.values(responses).forEach((response: any) => {
          if (Array.isArray(response)) {
            response.forEach((gain: number) => {
              minGain = Math.min(minGain, gain);
              maxGain = Math.max(maxGain, gain);
            });
          }
        });
      }
    }

    if (minGain !== Infinity && maxGain !== -Infinity) {
      this.eqGraphMinGain = minGain - 1;
      this.eqGraphMaxGain = maxGain + 1;
      console.log(
        "[EQ Graph] Dynamic Y-axis range:",
        this.eqGraphMinGain,
        "to",
        this.eqGraphMaxGain
      );
    }
  }

  private drawGrid(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    ctx.strokeStyle = "rgba(255, 255, 255, 0.1)";
    ctx.lineWidth = 1;

    // Vertical frequency lines
    const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqMarkers.forEach((freq) => {
      const x = this.freqToX(freq, width);
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height - 30);
      ctx.stroke();
    });

    // Horizontal gain lines
    const gainRange = this.eqGraphMaxGain - this.eqGraphMinGain;
    const gainStep = Math.pow(10, Math.floor(Math.log10(gainRange / 5)));
    
    for (let gain = Math.ceil(this.eqGraphMinGain / gainStep) * gainStep; 
         gain <= this.eqGraphMaxGain; 
         gain += gainStep) {
      const y = this.gainToY(gain, height);
      ctx.beginPath();
      ctx.moveTo(40, y);
      ctx.lineTo(width, y);
      
      if (Math.abs(gain) < 0.01) {
        ctx.lineWidth = 2;
        ctx.strokeStyle = "rgba(255, 255, 255, 0.3)";
      } else {
        ctx.lineWidth = 1;
        ctx.strokeStyle = "rgba(255, 255, 255, 0.1)";
      }
      ctx.stroke();
    }
  }

  private drawFrequencyLabels(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    ctx.fillStyle = "rgba(255, 255, 255, 0.7)";
    ctx.font = "11px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "top";

    const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqMarkers.forEach((freq) => {
      const x = this.freqToX(freq, width);
      const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
      ctx.fillText(label, x, height - 25);
    });
  }

  private drawGainLabels(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    ctx.fillStyle = "rgba(255, 255, 255, 0.7)";
    ctx.font = "11px sans-serif";
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";

    const gainRange = this.eqGraphMaxGain - this.eqGraphMinGain;
    const gainStep = Math.pow(10, Math.floor(Math.log10(gainRange / 5)));
    
    for (let gain = Math.ceil(this.eqGraphMinGain / gainStep) * gainStep; 
         gain <= this.eqGraphMaxGain; 
         gain += gainStep) {
      const y = this.gainToY(gain, height);
      const label = `${gain > 0 ? '+' : ''}${gain.toFixed(0)}dB`;
      ctx.fillText(label, 35, y);
    }
  }

  private drawCombinedResponse(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    if (!this.eqResponseData?.combined_response) return;
    const { frequencies, combined_response } = this.eqResponseData;

    this.computeDynamicYAxisRange();

    ctx.strokeStyle = "rgba(100, 200, 255, 0.8)";
    ctx.lineWidth = 2;
    ctx.beginPath();

    frequencies.forEach((freq: number, i: number) => {
      const x = this.freqToX(freq, width);
      const y = this.gainToY(combined_response[i], height);
      
      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });

    ctx.stroke();
  }

  private drawIndividualResponses(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    if (!this.eqResponseData?.individual_responses) return;
    const { frequencies, individual_responses } = this.eqResponseData;

    const colors = [
      "rgba(255, 100, 100, 0.6)",
      "rgba(100, 255, 100, 0.6)",
      "rgba(255, 255, 100, 0.6)",
      "rgba(100, 100, 255, 0.6)",
      "rgba(255, 100, 255, 0.6)",
      "rgba(100, 255, 255, 0.6)",
    ];

    this.currentFilterParams.forEach((filter, filterIdx) => {
      if (!filter.enabled || Math.abs(filter.gain) < 0.1) return;
      
      const response = individual_responses[filterIdx];
      if (!response || !Array.isArray(response)) return;

      ctx.strokeStyle = colors[filterIdx % colors.length];
      ctx.lineWidth = 1.5;
      ctx.beginPath();

      frequencies.forEach((freq: number, i: number) => {
        const x = this.freqToX(freq, width);
        const y = this.gainToY(response[i], height);
        
        if (i === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      });

      ctx.stroke();
    });
  }

  private drawFilterHandles(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    isDarkMode: boolean
  ): void {
    this.currentFilterParams.forEach((filter, idx) => {
      if (!filter.enabled) return;
      
      const x = this.freqToX(filter.frequency, width);
      const y = this.gainToY(filter.gain, height);
      const isSelected = idx === this.selectedFilterIndex;

      // Draw Q bar (horizontal line showing Q bandwidth)
      ctx.strokeStyle = isSelected 
        ? "rgba(255, 200, 100, 0.8)" 
        : "rgba(255, 255, 255, 0.4)";
      ctx.lineWidth = isSelected ? 3 : 2;
      
      const barWidth = 40 / filter.q;
      ctx.beginPath();
      ctx.moveTo(x - barWidth / 2, y);
      ctx.lineTo(x + barWidth / 2, y);
      ctx.stroke();

      // Draw handle point
      ctx.fillStyle = isSelected 
        ? "rgba(255, 200, 100, 1)" 
        : "rgba(255, 255, 255, 0.8)";
      ctx.beginPath();
      ctx.arc(x, y, isSelected ? 6 : 4, 0, Math.PI * 2);
      ctx.fill();

      // Draw selection ring
      if (isSelected) {
        ctx.strokeStyle = "rgba(255, 200, 100, 0.6)";
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(x, y, 10, 0, Math.PI * 2);
        ctx.stroke();
      }
    });
  }

  // ===== COORDINATE CONVERSION =====

  private freqToX(freq: number, width: number): number {
    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const logFreq = Math.log10(
      Math.max(this.EQ_GRAPH_MIN_FREQ, Math.min(this.EQ_GRAPH_MAX_FREQ, freq)),
    );
    return ((logFreq - logMin) / (logMax - logMin)) * (width - 40) + 40;
  }

  private xToFreq(x: number, width: number): number {
    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const logFreq = logMin + ((x - 40) / (width - 40)) * (logMax - logMin);
    return Math.pow(10, logFreq);
  }

  private gainToY(gain: number, height: number): number {
    const range = this.eqGraphMaxGain - this.eqGraphMinGain;
    const normalized = (gain - this.eqGraphMinGain) / range;
    return height - 30 - normalized * (height - 60);
  }

  private yToGain(y: number, height: number): number {
    const range = this.eqGraphMaxGain - this.eqGraphMinGain;
    const normalized = (height - 30 - y) / (height - 60);
    return this.eqGraphMinGain + normalized * range;
  }

  // ===== GRAPH INTERACTIONS =====

  handleGraphMouseDown(e: MouseEvent): void {
    if (!this.eqGraphCanvas) return;
    
    const rect = this.eqGraphCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const width = this.eqGraphCanvas.width;
    const height = this.eqGraphCanvas.height;
    const clickedFreq = this.xToFreq(x, width);

    let closestIdx = -1;
    let minFreqDist = Infinity;

    // First, check if clicking on Q bar of selected filter
    if (this.selectedFilterIndex >= 0) {
      const filter = this.currentFilterParams[this.selectedFilterIndex];
      if (filter && filter.enabled) {
        const filterX = this.freqToX(filter.frequency, width);
        const filterY = this.gainToY(filter.gain, height);
        const barWidth = 40 / filter.q;
        const dx = x - filterX;
        const dy = y - filterY;

        if (Math.abs(dx) <= barWidth / 2 && Math.abs(dy) <= 5) {
          closestIdx = this.selectedFilterIndex;
          this.dragMode = 'bar';
        }
      }
    }

    // If not on Q bar, find closest filter by frequency
    if (closestIdx === -1) {
      this.currentFilterParams.forEach((filter, idx) => {
        if (!filter.enabled) return;
        const freqDist = Math.abs(filter.frequency - clickedFreq);
        if (freqDist < minFreqDist) {
          minFreqDist = freqDist;
          closestIdx = idx;
        }
      });
      this.dragMode = 'ring';
    }

    if (closestIdx >= 0) {
      this.selectedFilterIndex = closestIdx;
      this.isDraggingHandle = true;
      this.lastMouseY = y;
      this.drawEQGraph();
      this.updateTableSelection();
    }
  }

  handleGraphMouseMove(e: MouseEvent): void {
    if (!this.isDraggingHandle || !this.eqGraphCanvas) return;

    const rect = this.eqGraphCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const width = this.eqGraphCanvas.width;
    const height = this.eqGraphCanvas.height;
    const filter = this.currentFilterParams[this.selectedFilterIndex];
    if (!filter) return;

    if (this.dragMode === 'ring') {
      // Change frequency and gain
      filter.frequency = Math.max(
        this.EQ_GRAPH_MIN_FREQ,
        Math.min(this.EQ_GRAPH_MAX_FREQ, this.xToFreq(x, width)),
      );
      filter.gain = Math.max(
        this.eqGraphMinGain,
        Math.min(this.eqGraphMaxGain, this.yToGain(y, height)),
      );
    } else if (this.dragMode === 'bar') {
      // Change Q only
      const qDelta = (y - this.lastMouseY) / 50;
      filter.q = Math.max(
        this.EQ_GRAPH_MIN_Q,
        Math.min(this.EQ_GRAPH_MAX_Q, filter.q + qDelta),
      );
      this.lastMouseY = y;
    }

    this.requestEQResponseUpdate();
    this.drawEQGraph();
    this.updateTableInputs();
  }

  handleGraphMouseUp(): void {
    if (this.isDraggingHandle) {
      this.isDraggingHandle = false;
      this.updateFilterParams(this.currentFilterParams);
    }
  }

  handleGraphMouseLeave(): void {
    this.isDraggingHandle = false;
  }

  private updateTableSelection(): void {
    if (!this.eqTableContainer) return;
    
    const table = this.eqTableContainer.querySelector("table");
    if (!table) return;

    // Update selected column
    const cells = table.querySelectorAll(`td[data-filter-index="${this.selectedFilterIndex}"], th[data-filter-index="${this.selectedFilterIndex}"]`);
    cells.forEach(cell => cell.classList.add("selected"));

    // Remove selection from other columns
    const allCells = table.querySelectorAll('td[data-filter-index], th[data-filter-index]');
    allCells.forEach(cell => {
      const index = parseInt(cell.getAttribute('data-filter-index') || '-1', 10);
      if (index !== this.selectedFilterIndex) {
        cell.classList.remove('selected');
      }
    });
  }

  private updateTableInputs(): void {
    if (!this.eqTableContainer) return;
    const filter = this.currentFilterParams[this.selectedFilterIndex];
    if (!filter) return;

    const table = this.eqTableContainer.querySelector("table");
    if (!table) return;

    const cells = table.querySelectorAll(
      `td[data-filter-index="${this.selectedFilterIndex}"]`,
    );
    cells.forEach((cell) => {
      const freqInput = cell.querySelector(".eq-frequency") as HTMLInputElement;
      const qInput = cell.querySelector(".eq-q") as HTMLInputElement;
      const gainInput = cell.querySelector(".eq-gain") as HTMLInputElement;

      if (freqInput) freqInput.value = filter.frequency.toFixed(1);
      if (qInput) qInput.value = filter.q.toFixed(2);
      if (gainInput) gainInput.value = filter.gain.toFixed(2);
    });
  }

  // ===== CLEANUP =====

  destroy(): void {
    // Close modal if open
    this.closeEQModal();

    // Remove DOM elements
    if (this.eqModal) {
      this.eqModal.remove();
      this.eqModal = null;
    }
    if (this.eqBackdrop) {
      this.eqBackdrop.remove();
      this.eqBackdrop = null;
    }

    // Clear timers
    if (this.eqResponseDebounceTimer) {
      clearTimeout(this.eqResponseDebounceTimer);
      this.eqResponseDebounceTimer = null;
    }
  }
}
