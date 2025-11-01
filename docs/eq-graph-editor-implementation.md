# Graphical EQ Editor Implementation Plan

## Overview

This document provides a complete implementation guide for adding an interactive graphical EQ editor to the audio player. The backend support is already complete; this focuses on the frontend implementation.

## Current Status

### ✅ Completed
- Backend EQ response computation (`compute_eq_response`)
- Position tracking and progress cursor
- 30-bin spectrum analyzer with loudness display
- Basic EQ table editor with text inputs

### 🚧 To Implement
- Interactive canvas-based EQ graph
- Visual filter editing (drag handles)
- Filter type selection UI
- Graph ↔ controls synchronization

---

## Architecture

### Data Flow
```
User Interaction → Update Filter Params → Call Backend → Render Graph → Update Controls
                                                ↓
                                        Update Audio Filters
```

### Key Components
1. **EQ Graph Canvas** - Visual representation and interaction
2. **Filter Controls** - Per-filter input controls below graph
3. **Backend Integration** - Real-time response computation
4. **Synchronization** - Bidirectional updates between graph and controls

---

## Implementation Steps

## Part 1: Add Filter Type Support

### 1.1 Extend FilterParam Interface

**File**: `src-ui-frontend/modules/audio-player/audio-player.ts`

Add after the existing `FilterParam` interface (around line 33):

```typescript
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
```

### 1.2 Update currentFilterParams

Change the type and add default filter_type:

```typescript
private currentFilterParams: ExtendedFilterParam[] = [
  { frequency: 100, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
  { frequency: 1000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
  { frequency: 10000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
];
```

---

## Part 2: Add EQ Graph Canvas

### 2.1 Add Canvas Element to Modal

**File**: `src-ui-frontend/modules/audio-player/audio-player.ts`

Modify `_createEQModal()` method to include canvas:

```typescript
modal.innerHTML = `
  <div class="eq-modal-content">
    <div class="eq-modal-header">
      <h3>Equalizer Configuration</h3>
      <button type="button" class="eq-modal-close-btn">&times;</button>
    </div>
    <div class="eq-modal-body">
      <div class="eq-graph-container">
        <canvas class="eq-graph-canvas"></canvas>
      </div>
      <div class="playback-options-container"></div>
      <div class="eq-table-container"></div>
    </div>
  </div>
`;
```

### 2.2 Add Canvas Properties to Class

Add to the class properties section (around line 100):

```typescript
// EQ Graph properties
private eqGraphCanvas: HTMLCanvasElement | null = null;
private eqGraphCtx: CanvasRenderingContext2D | null = null;
private selectedFilterIndex: number = -1;
private isDraggingHandle: boolean = false;
private dragMode: 'ring' | 'bar' | null = null;
private dragStartX: number = 0;
private dragStartY: number = 0;
private eqResponseData: any = null; // Cached response from backend

// EQ Graph constants
private readonly EQ_GRAPH_MIN_FREQ = 20;
private readonly EQ_GRAPH_MAX_FREQ = 20000;
private readonly EQ_GRAPH_MIN_GAIN = -18; // -6 * max_db (3.0)
private readonly EQ_GRAPH_MAX_GAIN = 3;   // max_db
private readonly EQ_GRAPH_MIN_Q = 0.1;
private readonly EQ_GRAPH_MAX_Q = 3.0;
private readonly EQ_GRAPH_FREQ_POINTS = 256; // Number of points for response curve
```

### 2.3 Cache Canvas in cacheUIElements()

Add after existing canvas caching (around line 500):

```typescript
// Cache EQ graph canvas
if (this.eqModal) {
  this.eqGraphCanvas = this.eqModal.querySelector(".eq-graph-canvas");
  if (this.eqGraphCanvas) {
    this.eqGraphCtx = this.eqGraphCanvas.getContext("2d");
    // Set canvas dimensions
    this.resizeEQGraphCanvas();
  }
}
```

---

## Part 3: Backend Integration

### 3.1 Add Method to Compute EQ Response

Add this method after `updateLoudnessDisplay()`:

```typescript
/**
 * Compute EQ response from backend
 */
private async computeEQResponse(): Promise<void> {
  if (!this.currentFilterParams || this.currentFilterParams.length === 0) {
    this.eqResponseData = null;
    return;
  }

  try {
    // Generate logarithmically-spaced frequency grid
    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const frequencies: number[] = [];
    
    for (let i = 0; i < this.EQ_GRAPH_FREQ_POINTS; i++) {
      const logFreq = logMin + (logMax - logMin) * (i / (this.EQ_GRAPH_FREQ_POINTS - 1));
      frequencies.push(Math.pow(10, logFreq));
    }

    // Prepare filter params for backend
    const filters = this.currentFilterParams.map(f => ({
      filter_type: f.filter_type || "Peak",
      frequency: f.frequency,
      q: f.q,
      gain: f.gain,
      enabled: f.enabled,
    }));

    // Call backend
    const result = await invoke("compute_eq_response", {
      filters,
      sampleRate: 48000, // Use config or default
      frequencies,
    });

    this.eqResponseData = result;
    
    // Redraw graph with new data
    this.drawEQGraph();
  } catch (error) {
    console.error("[EQ Graph] Failed to compute response:", error);
  }
}
```

### 3.2 Add Debounced Update

Add debounce helper and method:

```typescript
// Add to class properties
private eqResponseDebounceTimer: number | null = null;

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
  }, 60); // 60ms debounce
}
```

---

## Part 4: Graph Rendering

### 4.1 Resize Canvas Method

```typescript
private resizeEQGraphCanvas(): void {
  if (!this.eqGraphCanvas) return;
  
  const container = this.eqGraphCanvas.parentElement;
  if (!container) return;
  
  const rect = container.getBoundingClientRect();
  const width = Math.max(rect.width || 600, 400);
  const height = 300; // Fixed height
  
  this.eqGraphCanvas.width = width;
  this.eqGraphCanvas.height = height;
  
  // Redraw after resize
  this.drawEQGraph();
}
```

### 4.2 Main Drawing Method

```typescript
private drawEQGraph(): void {
  if (!this.eqGraphCanvas || !this.eqGraphCtx || !this.eqResponseData) {
    return;
  }
  
  const ctx = this.eqGraphCtx;
  const width = this.eqGraphCanvas.width;
  const height = this.eqGraphCanvas.height;
  
  const isDarkMode = window.matchMedia?.("(prefers-color-scheme: dark)").matches;
  
  // Clear canvas
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = isDarkMode ? "rgb(26, 26, 26)" : "rgb(255, 255, 255)";
  ctx.fillRect(0, 0, width, height);
  
  // Draw grid
  this.drawGrid(ctx, width, height, isDarkMode);
  
  // Draw individual filter responses (if not all Peak)
  this.drawIndividualResponses(ctx, width, height, isDarkMode);
  
  // Draw combined response
  this.drawCombinedResponse(ctx, width, height, isDarkMode);
  
  // Draw filter handles
  this.drawFilterHandles(ctx, width, height, isDarkMode);
}
```

### 4.3 Grid Drawing

```typescript
private drawGrid(ctx: CanvasRenderingContext2D, width: number, height: number, isDarkMode: boolean): void {
  ctx.strokeStyle = isDarkMode ? "rgba(255, 255, 255, 0.1)" : "rgba(0, 0, 0, 0.1)";
  ctx.lineWidth = 1;
  
  // Frequency grid lines (log scale: 20, 50, 100, 200, 500, 1k, 2k, 5k, 10k, 20k)
  const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
  freqMarkers.forEach(freq => {
    const x = this.freqToX(freq, width);
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
  });
  
  // Gain grid lines (-18, -12, -6, 0, +3 dB)
  const gainMarkers = [-18, -12, -6, 0, 3];
  gainMarkers.forEach(gain => {
    const y = this.gainToY(gain, height);
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
    
    // Emphasize 0 dB line
    if (gain === 0) {
      ctx.lineWidth = 2;
      ctx.strokeStyle = isDarkMode ? "rgba(255, 255, 255, 0.3)" : "rgba(0, 0, 0, 0.3)";
      ctx.stroke();
      ctx.lineWidth = 1;
      ctx.strokeStyle = isDarkMode ? "rgba(255, 255, 255, 0.1)" : "rgba(0, 0, 0, 0.1)";
    }
  });
  
  // Draw axis labels
  ctx.fillStyle = isDarkMode ? "rgba(255, 255, 255, 0.5)" : "rgba(0, 0, 0, 0.5)";
  ctx.font = "10px sans-serif";
  
  freqMarkers.forEach(freq => {
    const x = this.freqToX(freq, width);
    const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
    ctx.fillText(label, x + 2, height - 4);
  });
  
  gainMarkers.forEach(gain => {
    const y = this.gainToY(gain, height);
    ctx.fillText(`${gain > 0 ? '+' : ''}${gain}dB`, 4, y - 2);
  });
}
```

### 4.4 Coordinate Conversion Helpers

```typescript
/**
 * Convert frequency to X coordinate (logarithmic scale)
 */
private freqToX(freq: number, width: number): number {
  const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
  const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
  const logFreq = Math.log10(Math.max(this.EQ_GRAPH_MIN_FREQ, Math.min(this.EQ_GRAPH_MAX_FREQ, freq)));
  return ((logFreq - logMin) / (logMax - logMin)) * width;
}

/**
 * Convert X coordinate to frequency (logarithmic scale)
 */
private xToFreq(x: number, width: number): number {
  const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
  const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
  const logFreq = logMin + ((x / width) * (logMax - logMin));
  return Math.pow(10, logFreq);
}

/**
 * Convert gain (dB) to Y coordinate
 */
private gainToY(gain: number, height: number): number {
  const range = this.EQ_GRAPH_MAX_GAIN - this.EQ_GRAPH_MIN_GAIN;
  const normalized = (gain - this.EQ_GRAPH_MIN_GAIN) / range;
  return height - (normalized * height);
}

/**
 * Convert Y coordinate to gain (dB)
 */
private yToGain(y: number, height: number): number {
  const range = this.EQ_GRAPH_MAX_GAIN - this.EQ_GRAPH_MIN_GAIN;
  const normalized = (height - y) / height;
  return this.EQ_GRAPH_MIN_GAIN + (normalized * range);
}
```

### 4.5 Draw Response Curves

```typescript
private drawCombinedResponse(ctx: CanvasRenderingContext2D, width: number, height: number, isDarkMode: boolean): void {
  if (!this.eqResponseData || !this.eqResponseData.combined_response) return;
  
  const { frequencies, combined_response } = this.eqResponseData;
  
  ctx.strokeStyle = isDarkMode ? "#4dabf7" : "#007bff";
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

private drawIndividualResponses(ctx: CanvasRenderingContext2D, width: number, height: number, isDarkMode: boolean): void {
  if (!this.eqResponseData || !this.eqResponseData.individual_responses) return;
  
  const { frequencies, individual_responses } = this.eqResponseData;
  
  // Color palette for different filters
  const colors = [
    isDarkMode ? "#fa5252" : "#dc3545",
    isDarkMode ? "#fab005" : "#ffc107",
    isDarkMode ? "#40c057" : "#28a745",
    isDarkMode ? "#4dabf7" : "#007bff",
    isDarkMode ? "#cc5de8" : "#6f42c1",
  ];
  
  this.currentFilterParams.forEach((filter, filterIdx) => {
    if (!filter.enabled || Math.abs(filter.gain) < 0.1) return;
    
    const response = individual_responses[filterIdx];
    if (!response) return;
    
    ctx.strokeStyle = colors[filterIdx % colors.length];
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.5;
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    
    frequencies.forEach((freq: number, i: number) => {
      const x = this.freqToX(freq, width);
      const y = this.gainToY(response.magnitudes_db[i], height);
      
      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });
    
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.globalAlpha = 1;
  });
}
```

### 4.6 Draw Filter Handles

```typescript
private drawFilterHandles(ctx: CanvasRenderingContext2D, width: number, height: number, isDarkMode: boolean): void {
  this.currentFilterParams.forEach((filter, idx) => {
    if (!filter.enabled) return;
    
    const x = this.freqToX(filter.frequency, width);
    const y = this.gainToY(filter.gain, height);
    const isSelected = idx === this.selectedFilterIndex;
    
    // Draw ring (for frequency/gain adjustment)
    ctx.strokeStyle = isSelected 
      ? (isDarkMode ? "#fa5252" : "#dc3545")
      : (isDarkMode ? "#4dabf7" : "#007bff");
    ctx.lineWidth = isSelected ? 3 : 2;
    ctx.fillStyle = isDarkMode ? "rgba(77, 171, 247, 0.3)" : "rgba(0, 123, 255, 0.3)";
    
    ctx.beginPath();
    ctx.arc(x, y, isSelected ? 8 : 6, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
    
    // Draw Q bar (horizontal bar for Q adjustment)
    if (isSelected) {
      // Bar width inversely proportional to Q (higher Q = narrower bar)
      const barWidth = 40 / filter.q;
      ctx.strokeStyle = isDarkMode ? "#fab005" : "#ffc107";
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(x - barWidth, y);
      ctx.lineTo(x + barWidth, y);
      ctx.stroke();
    }
  });
}
```

---

## Part 5: Mouse Interactions

### 5.1 Setup Event Listeners

Add to `setupEventListeners()` method:

```typescript
// EQ Graph interactions
if (this.eqGraphCanvas) {
  this.eqGraphCanvas.addEventListener("mousedown", (e) => this.handleGraphMouseDown(e));
  this.eqGraphCanvas.addEventListener("mousemove", (e) => this.handleGraphMouseMove(e));
  this.eqGraphCanvas.addEventListener("mouseup", (e) => this.handleGraphMouseUp(e));
  this.eqGraphCanvas.addEventListener("mouseleave", (e) => this.handleGraphMouseUp(e));
  
  // Change cursor on hover
  this.eqGraphCanvas.style.cursor = "crosshair";
}
```

### 5.2 Mouse Event Handlers

```typescript
private handleGraphMouseDown(e: MouseEvent): void {
  if (!this.eqGraphCanvas) return;
  
  const rect = this.eqGraphCanvas.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;
  const width = this.eqGraphCanvas.width;
  const height = this.eqGraphCanvas.height;
  
  const clickFreq = this.xToFreq(x, width);
  const clickGain = this.yToGain(y, height);
  
  // Find closest filter by frequency (log distance)
  let closestIdx = -1;
  let minLogDist = Infinity;
  
  this.currentFilterParams.forEach((filter, idx) => {
    if (!filter.enabled) return;
    
    const filterX = this.freqToX(filter.frequency, width);
    const filterY = this.gainToY(filter.gain, height);
    const dx = x - filterX;
    const dy = y - filterY;
    const dist = Math.sqrt(dx * dx + dy * dy);
    
    // Check if clicking on ring (within 10px)
    if (dist < 10 && dist < minLogDist) {
      closestIdx = idx;
      minLogDist = dist;
      this.dragMode = 'ring';
    }
    
    // Check if clicking on Q bar (if selected)
    if (idx === this.selectedFilterIndex) {
      const barWidth = 40 / filter.q;
      if (Math.abs(dy) < 5 && Math.abs(dx) < barWidth) {
        closestIdx = idx;
        this.dragMode = 'bar';
      }
    }
  });
  
  if (closestIdx >= 0) {
    this.selectedFilterIndex = closestIdx;
    this.isDraggingHandle = true;
    this.dragStartX = x;
    this.dragStartY = y;
    this.drawEQGraph();
  }
}

private handleGraphMouseMove(e: MouseEvent): void {
  if (!this.isDraggingHandle || !this.eqGraphCanvas) return;
  
  const rect = this.eqGraphCanvas.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;
  const width = this.eqGraphCanvas.width;
  const height = this.eqGraphCanvas.height;
  
  const filter = this.currentFilterParams[this.selectedFilterIndex];
  if (!filter) return;
  
  if (this.dragMode === 'ring') {
    // Update frequency and gain
    filter.frequency = Math.max(this.EQ_GRAPH_MIN_FREQ, Math.min(this.EQ_GRAPH_MAX_FREQ, this.xToFreq(x, width)));
    filter.gain = Math.max(this.EQ_GRAPH_MIN_GAIN, Math.min(this.EQ_GRAPH_MAX_GAIN, this.yToGain(y, height)));
  } else if (this.dragMode === 'bar') {
    // Update Q based on horizontal movement
    const deltaX = x - this.dragStartX;
    const qDelta = deltaX / 20; // Sensitivity factor
    filter.q = Math.max(this.EQ_GRAPH_MIN_Q, Math.min(this.EQ_GRAPH_MAX_Q, filter.q + qDelta));
    this.dragStartX = x;
  }
  
  // Update graph and controls
  this.requestEQResponseUpdate();
  this.renderEQTable(); // Re-render controls to sync
}

private handleGraphMouseUp(e: MouseEvent): void {
  if (this.isDraggingHandle) {
    this.isDraggingHandle = false;
    this.dragMode = null;
    
    // Apply filter changes to audio
    this.updateFilterParams(this.currentFilterParams);
  }
}
```

---

## Part 6: Enhanced Controls UI

### 6.1 Update renderEQTable()

Modify to include filter type selection:

```typescript
private renderEQTable(): void {
  if (!this.eqTableContainer) return;
  
  const eqSection = document.createElement("div");
  eqSection.className = "eq-section";
  
  const header = document.createElement("h4");
  header.textContent = "Equalizer Filters";
  
  const table = document.createElement("table");
  table.className = "eq-table";
  table.innerHTML = `
    <thead>
      <tr>
        <th>Type</th>
        <th>Enabled</th>
        <th>Frequency (Hz)</th>
        <th>Q</th>
        <th>Gain (dB)</th>
      </tr>
    </thead>
    <tbody>
      ${this.currentFilterParams.map((filter, index) => `
        <tr data-filter-index="${index}" class="${index === this.selectedFilterIndex ? 'selected' : ''}">
          <td>
            <select data-index="${index}" class="eq-filter-type">
              ${Object.entries(FILTER_TYPES).map(([type, info]) => `
                <option value="${type}" ${filter.filter_type === type ? 'selected' : ''}>
                  ${info.icon} ${info.shortName}
                </option>
              `).join('')}
            </select>
          </td>
          <td>
            <input type="checkbox" data-index="${index}" class="eq-enabled" ${filter.enabled ? 'checked' : ''}>
          </td>
          <td>
            <input type="number" data-index="${index}" class="eq-frequency" 
                   value="${filter.frequency.toFixed(1)}" min="20" max="20000" step="1">
          </td>
          <td>
            <input type="number" data-index="${index}" class="eq-q" 
                   value="${filter.q.toFixed(2)}" min="0.1" max="3.0" step="0.05">
          </td>
          <td>
            <input type="number" data-index="${index}" class="eq-gain" 
                   value="${filter.gain.toFixed(2)}" min="-18" max="3" step="0.1">
          </td>
        </tr>
      `).join('')}
    </tbody>
  `;
  
  this.eqTableContainer.innerHTML = "";
  eqSection.appendChild(header);
  eqSection.appendChild(table);
  this.eqTableContainer.appendChild(eqSection);
  
  // Attach event listeners
  table.addEventListener("input", (e) => this.handleEQTableChange(e));
  table.addEventListener("change", (e) => this.handleEQTableChange(e));
}
```

---

## Part 7: CSS Styling

**File**: `src-ui-frontend/modules/audio-player/audio-player.css`

Add these styles:

```css
/* EQ Graph Container */
.eq-graph-container {
  width: 100%;
  height: 300px;
  margin-bottom: var(--spacing-md);
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--bg-accent);
}

.eq-graph-canvas {
  display: block;
  width: 100%;
  height: 100%;
  cursor: crosshair;
}

/* EQ Table Styling */
.eq-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.eq-table th,
.eq-table td {
  padding: 8px;
  border: 1px solid var(--border-color);
  text-align: center;
}

.eq-table th {
  background: var(--bg-accent);
  font-weight: 600;
}

.eq-table tr.selected {
  background: var(--button-primary);
  color: white;
}

.eq-table tr.selected input,
.eq-table tr.selected select {
  background: rgba(255, 255, 255, 0.9);
}

.eq-filter-type {
  width: 80px;
  padding: 4px;
  font-family: monospace;
  font-size: 11px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  background: var(--bg-primary);
  cursor: pointer;
}

.eq-table input[type="number"] {
  width: 70px;
  padding: 4px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  background: var(--bg-primary);
  text-align: center;
}

.eq-table input[type="checkbox"] {
  width: 16px;
  height: 16px;
  cursor: pointer;
}
```

---

## Part 8: Integration & Wiring

### 8.1 Call computeEQResponse() when modal opens

In `openEQModal()` method:

```typescript
private openEQModal(): void {
  // ... existing positioning code ...
  
  // Show modal
  this.eqBackdrop.classList.add("visible");
  this.eqModal.classList.add("visible");
  
  // Compute and draw EQ graph
  this.computeEQResponse();
  
  // ... rest of existing code ...
}
```

### 8.2 Update handleEQTableChange()

Modify to handle filter type changes and trigger graph update:

```typescript
private handleEQTableChange(e: Event): void {
  const target = e.target as HTMLInputElement | HTMLSelectElement;
  const index = parseInt(target.dataset.index || "0", 10);
  
  if (isNaN(index) || !this.currentFilterParams[index]) return;
  
  // Get the field being changed
  const className = target.className;
  
  if (className.includes("eq-filter-type")) {
    this.currentFilterParams[index].filter_type = (target as HTMLSelectElement).value;
  } else if (className.includes("eq-enabled")) {
    this.currentFilterParams[index].enabled = (target as HTMLInputElement).checked;
  } else if (className.includes("eq-frequency")) {
    this.currentFilterParams[index].frequency = parseFloat((target as HTMLInputElement).value) || 0;
  } else if (className.includes("eq-q")) {
    this.currentFilterParams[index].q = parseFloat((target as HTMLInputElement).value) || 1;
  } else if (className.includes("eq-gain")) {
    this.currentFilterParams[index].gain = parseFloat((target as HTMLInputElement).value) || 0;
  }
  
  // Request graph update
  this.requestEQResponseUpdate();
  
  // Update audio filters
  this.updateFilterParams(this.currentFilterParams);
}
```

---

## Testing Checklist

### Functionality Tests
- [ ] Modal opens with EQ graph displayed
- [ ] Graph shows grid, labels, and 0dB line
- [ ] Combined response curve renders correctly
- [ ] Individual filter curves show when enabled
- [ ] Clicking near a filter handle selects it
- [ ] Dragging ring updates frequency and gain
- [ ] Dragging bar updates Q value
- [ ] Controls update when dragging handles
- [ ] Graph updates when controls change
- [ ] Filter type dropdown works
- [ ] Disabled filters don't show in graph
- [ ] Multiple filters can be edited

### Visual Tests
- [ ] Graph scales properly on resize
- [ ] Dark/light theme colors correct
- [ ] Selected filter highlighted
- [ ] Axis labels readable
- [ ] Handles don't overlap

### Performance Tests
- [ ] No lag when dragging
- [ ] Debouncing works (max 1 backend call per 60ms)
- [ ] Graph renders smoothly
- [ ] No memory leaks on open/close

---

## Keyboard Shortcuts (Optional Enhancement)

Add keyboard navigation when a filter is selected:

```typescript
private setupKeyboardShortcuts(): void {
  this.eqModal?.addEventListener("keydown", (e: KeyboardEvent) => {
    if (this.selectedFilterIndex < 0) return;
    
    const filter = this.currentFilterParams[this.selectedFilterIndex];
    if (!filter) return;
    
    let changed = false;
    
    switch (e.key) {
      case "ArrowLeft":
        if (e.shiftKey) {
          // Decrease Q
          filter.q = Math.max(this.EQ_GRAPH_MIN_Q, filter.q - 0.05);
          changed = true;
        } else {
          // Decrease frequency (logarithmically)
          filter.frequency = Math.max(this.EQ_GRAPH_MIN_FREQ, filter.frequency * 0.95);
          changed = true;
        }
        e.preventDefault();
        break;
        
      case "ArrowRight":
        if (e.shiftKey) {
          // Increase Q
          filter.q = Math.min(this.EQ_GRAPH_MAX_Q, filter.q + 0.05);
          changed = true;
        } else {
          // Increase frequency (logarithmically)
          filter.frequency = Math.min(this.EQ_GRAPH_MAX_FREQ, filter.frequency * 1.05);
          changed = true;
        }
        e.preventDefault();
        break;
        
      case "ArrowUp":
        // Increase gain
        filter.gain = Math.min(this.EQ_GRAPH_MAX_GAIN, filter.gain + 0.1);
        changed = true;
        e.preventDefault();
        break;
        
      case "ArrowDown":
        // Decrease gain
        filter.gain = Math.max(this.EQ_GRAPH_MIN_GAIN, filter.gain - 0.1);
        changed = true;
        e.preventDefault();
        break;
    }
    
    if (changed) {
      this.requestEQResponseUpdate();
      this.renderEQTable();
      this.updateFilterParams(this.currentFilterParams);
    }
  });
}
```

---

## Common Issues & Solutions

### Issue: Graph doesn't update when dragging
**Solution**: Ensure `requestEQResponseUpdate()` is called in `handleGraphMouseMove()`

### Issue: Backend call fails
**Solution**: Check that filter_type strings match backend enum exactly (case-sensitive)

### Issue: Canvas is blank
**Solution**: Verify canvas dimensions are set correctly in `resizeEQGraphCanvas()`

### Issue: Clicks not registering
**Solution**: Check event listener attachment and coordinate conversion math

### Issue: Dragging is laggy
**Solution**: Increase debounce timeout or reduce graph redraw frequency

---

## Future Enhancements

1. **Undo/Redo** - Add command pattern for filter changes
2. **Presets** - Save/load filter configurations
3. **A/B Comparison** - Compare two EQ settings
4. **Auto-EQ Integration** - Apply optimization results to graph
5. **Touch Support** - Add touch event handlers for tablets
6. **Zoom/Pan** - Allow zooming into frequency ranges
7. **Snap to Grid** - Optional snap for precise adjustments
8. **Export Graph** - Save graph as image

---

## Summary

This implementation provides a fully interactive EQ editor with:
- ✅ Visual representation of all filters
- ✅ Drag-and-drop editing
- ✅ Real-time backend computation
- ✅ Filter type selection
- ✅ Bidirectional synchronization
- ✅ Professional UI/UX

The modular approach allows incremental implementation and testing of each component.
