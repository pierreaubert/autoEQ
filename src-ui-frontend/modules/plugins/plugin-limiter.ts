// Limiter Plugin
// Brick-wall limiter with true peak detection and lookahead

import { BasePlugin } from './plugin-base';
import { PluginMenubar } from './plugin-menubar';
import type { PluginMetadata } from './plugin-types';
import Plotly from 'plotly.js-basic-dist-min';

/**
 * Limiter Plugin
 * Prevents audio from exceeding a specified ceiling
 */
export class LimiterPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: 'limiter-plugin',
    name: 'SotF: Limiter',
    category: 'dynamics',
    version: '1.0.0',
  };

  // UI components
  private menubar: PluginMenubar | null = null;

  // UI elements
  private grMeterCanvas: HTMLCanvasElement | null = null;
  private grMeterCtx: CanvasRenderingContext2D | null = null;
  private peakMeterCanvas: HTMLCanvasElement | null = null;
  private peakMeterCtx: CanvasRenderingContext2D | null = null;
  private plotContainer: HTMLElement | null = null;

  // Parameters
  private ceiling: number = -0.1;         // dB (max output level)
  private release: number = 100.0;        // ms
  private lookahead: number = 5.0;        // ms

  // Parameter metadata for keyboard control
  private parameterOrder = ['ceiling', 'release', 'lookahead'];
  private parameterRanges = {
    ceiling: { min: -12, max: 0, step: 0.1 },
    release: { min: 10, max: 1000, step: 10 },
    lookahead: { min: 0, max: 10, step: 0.5 },
  };

  // State
  private currentGainReduction: number = 0.0; // dB (negative)
  private currentPeak: number = -Infinity;    // dB
  private peakHold: number = -Infinity;       // dB
  private peakHoldTimer: number = 0;
  private clipping: boolean = false;
  private animationFrameId: number | null = null;
  private selectedParameterIndex: number = -1; // -1 = none selected

  /**
   * Render a single parameter slider with labels
   */
  private renderParameter(paramName: string, index: number, label: string, unit: string): string {
    const value = (this as any)[paramName];
    const range = this.parameterRanges[paramName as keyof typeof this.parameterRanges];

    // Format value display
    let displayValue = value.toFixed(paramName === 'ceiling' ? 2 : 1);
    displayValue = `${displayValue} ${unit}`;

    // Format min/max labels
    const minLabel = `${range.min} ${unit}`;
    const maxLabel = `${range.max} ${unit}`;

    return `
      <div class="field parameter-field plugin-param-field" data-param="${paramName}" data-index="${index}">
        <div class="plugin-param-header">
          <label class="label is-small has-text-light plugin-param-label">${label}</label>
          <span class="tag is-dark param-value plugin-param-value">${displayValue}</span>
        </div>
        <input
          type="range"
          class="slider is-fullwidth param-slider"
          data-param="${paramName}"
          min="${range.min}"
          max="${range.max}"
          step="${range.step}"
          value="${value}"
        />
        <div class="plugin-param-minmax">
          <span class="has-text-grey-light plugin-param-minmax-label">${minLabel}</span>
          <span class="has-text-grey-light plugin-param-minmax-label">${maxLabel}</span>
        </div>
      </div>
    `;
  }

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="is-flex is-flex-direction-column limiter-plugin ${standalone ? 'standalone' : 'embedded'}" style="height: 100%; min-height: 0; background: #1a1a1a;">
        ${standalone ? '<div class="limiter-menubar-container"></div>' : ''}
        <div class="is-flex is-flex-direction-column is-flex-grow-1" style="min-height: 0; overflow: hidden; padding: 0; margin: 0;">
          <!-- Bulma Columns -->
          <div class="columns is-gapless" style="height: 100%; margin: 0;">
            <!-- Column 1: Parameters (30%) -->
            <div class="column is-one-quarter">
              <div class="box" style="background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0; height: 100%; margin: 0;">
                <h4 class="title is-6 has-text-light">Limiter Settings</h4>
                <div>
                  ${this.renderParameter('ceiling', 0, 'Ceiling', 'dB')}
                  ${this.renderParameter('release', 1, 'Release', 'ms')}
                  ${this.renderParameter('lookahead', 2, 'Lookahead', 'ms')}
                </div>

                <!-- Status Indicators -->
                <div class="limiter-status" style="margin-top: 20px; padding: 12px; background: var(--bg-primary, #1a1a1a); border-radius: 4px;">
                  <div class="status-item" style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                    <span class="has-text-grey-light" style="font-size: 11px;">Peak Hold:</span>
                    <span class="has-text-light peak-hold-value" style="font-size: 11px; font-family: monospace;">${this.peakHold === -Infinity ? '-∞' : this.peakHold.toFixed(2)} dB</span>
                  </div>
                  <div class="status-item ${this.clipping ? 'clipping' : ''}" style="display: flex; justify-content: space-between;">
                    <span class="has-text-grey-light" style="font-size: 11px;">Clipping:</span>
                    <span class="clipping-indicator" style="font-size: 11px; font-weight: 600; color: ${this.clipping ? '#ef4444' : '#22c55e'};">${this.clipping ? 'YES' : 'NO'}</span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Column 2: Gain Reduction Meter (15%) -->
            <div class="column is-narrow" style="width: 15%;">
              <div class="box is-flex is-flex-direction-column" style="background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0; height: 100%; margin: 0;">
                <h4 class="title is-6 has-text-light has-text-centered mb-2">Gain Reduction</h4>
                <div class="is-flex-grow-1 is-flex is-flex-direction-column is-align-items-center is-justify-content-center">
                  <canvas class="gr-meter-canvas" width="60" height="240"></canvas>
                  <div class="gr-value has-text-light has-text-centered" style="margin-top: 8px; font-size: 12px; font-weight: 600;">${Math.abs(this.currentGainReduction).toFixed(1)} dB</div>
                </div>
              </div>
            </div>

            <!-- Column 3: Output Peak Meter (15%) -->
            <div class="column is-narrow" style="width: 15%;">
              <div class="box is-flex is-flex-direction-column" style="background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0; height: 100%; margin: 0;">
                <h4 class="title is-6 has-text-light has-text-centered mb-2">Output Peak</h4>
                <div class="is-flex-grow-1 is-flex is-flex-direction-column is-align-items-center is-justify-content-center">
                  <canvas class="peak-meter-canvas" width="60" height="240"></canvas>
                  <div class="peak-value has-text-light has-text-centered" style="margin-top: 8px; font-size: 12px; font-weight: 600;">${this.currentPeak === -Infinity ? '-∞' : this.currentPeak.toFixed(1)} dB</div>
                </div>
              </div>
            </div>

            <!-- Column 4: Transfer Curve (40%) -->
            <div class="column">
              <div class="box" style="background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0; height: 100%; margin: 0; padding: 12px;">
                <h4 class="title is-6 has-text-light has-text-centered mb-2">Transfer Curve</h4>
                <div id="limiter-plot-${this.metadata.id}" class="transfer-curve-container"></div>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector('.limiter-menubar-container') as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name);
      }
    }

    // Cache elements
    this.grMeterCanvas = this.container.querySelector('.gr-meter-canvas') as HTMLCanvasElement;
    this.grMeterCtx = this.grMeterCanvas?.getContext('2d') || null;
    this.peakMeterCanvas = this.container.querySelector('.peak-meter-canvas') as HTMLCanvasElement;
    this.peakMeterCtx = this.peakMeterCanvas?.getContext('2d') || null;
    this.plotContainer = this.container.querySelector(`#limiter-plot-${this.metadata.id}`) as HTMLElement;

    // Setup canvases
    this.setupMeterCanvas(this.grMeterCanvas, 60, 240);
    this.setupMeterCanvas(this.peakMeterCanvas, 60, 240);

    // Setup Plotly graph
    this.updatePlot();

    this.attachEventListeners();
    this.startRendering();
  }

  /**
   * Setup meter canvas
   */
  private setupMeterCanvas(canvas: HTMLCanvasElement | null, width: number, height: number): void {
    if (!canvas) return;

    const dpr = window.devicePixelRatio || 1;

    canvas.width = width * dpr;
    canvas.height = height * dpr;

    const ctx = canvas.getContext('2d');
    if (ctx) {
      ctx.scale(dpr, dpr);
    }
  }

  /**
   * Update Plotly transfer curve
   */
  private updatePlot(): void {
    if (!this.plotContainer) return;

    const dbRange = 12; // -12 to 0 dB
    const numPoints = 200;

    // Generate input dB values
    const inputDb: number[] = [];
    const outputDb: number[] = [];
    const referenceDb: number[] = [];

    for (let i = 0; i <= numPoints; i++) {
      const inputVal = (i / numPoints) * dbRange - dbRange; // -12 to 0 dB
      inputDb.push(inputVal);
      referenceDb.push(inputVal); // 1:1 reference line

      // Limiter: hard clip at ceiling
      let outputVal: number;
      if (inputVal <= this.ceiling) {
        outputVal = inputVal;
      } else {
        outputVal = this.ceiling;
      }

      outputDb.push(outputVal);
    }

    // Create Plotly traces
    const traces: Plotly.Data[] = [
      // Reference 1:1 line
      {
        x: inputDb,
        y: referenceDb,
        type: 'scatter',
        mode: 'lines',
        name: '1:1 Reference',
        line: {
          color: 'rgba(255, 255, 255, 0.3)',
          width: 1,
          dash: 'dot',
        },
        hoverinfo: 'skip',
      } as Plotly.Data,
      // Limiter curve
      {
        x: inputDb,
        y: outputDb,
        type: 'scatter',
        mode: 'lines',
        name: 'Limiter Curve',
        line: {
          color: '#ef4444',
          width: 3,
        },
        hovertemplate: 'Input: %{x:.1f} dB<br>Output: %{y:.1f} dB<extra></extra>',
      } as Plotly.Data,
    ];

    // Add ceiling line
    traces.push(
      // Horizontal ceiling line
      {
        x: [-12, 0],
        y: [this.ceiling, this.ceiling],
        type: 'scatter',
        mode: 'lines',
        name: 'Ceiling',
        line: {
          color: 'rgba(239, 68, 68, 0.6)',
          width: 2,
          dash: 'dash',
        },
        hoverinfo: 'skip',
      } as Plotly.Data
    );

    // Layout configuration
    const layout: Partial<Plotly.Layout> = {
      title: {
        text: '',
      },
      xaxis: {
        title: { text: 'Input (dB)' },
        gridcolor: 'rgba(255, 255, 255, 0.1)',
        zerolinecolor: 'rgba(255, 255, 255, 0.2)',
        range: [-12, 0],
        dtick: 2,
      },
      yaxis: {
        title: { text: 'Output (dB)' },
        gridcolor: 'rgba(255, 255, 255, 0.1)',
        zerolinecolor: 'rgba(255, 255, 255, 0.2)',
        range: [-12, 0],
        dtick: 2,
      },
      plot_bgcolor: '#1a1a1a',
      paper_bgcolor: '#1a1a1a',
      font: {
        color: '#ffffff',
        size: 11,
      },
      margin: {
        l: 50,
        r: 30,
        t: 30,
        b: 50,
      },
      showlegend: true,
      legend: {
        x: 0.02,
        y: 0.98,
        bgcolor: 'rgba(42, 42, 42, 0.8)',
        bordercolor: 'rgba(64, 64, 64, 0.8)',
        borderwidth: 1,
      },
      hovermode: 'closest',
    };

    // Config
    const config: Partial<Plotly.Config> = {
      responsive: true,
      displayModeBar: false,
    };

    // Create or update plot
    Plotly.react(this.plotContainer, traces, layout, config);
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Slider input events
    const sliders = this.container?.querySelectorAll('.param-slider') || [];
    sliders.forEach((slider) => {
      slider.addEventListener('input', (e) => {
        this.handleSliderChange(e as Event);
      });
    });

    // Parameter field click to select
    const fields = this.container?.querySelectorAll('.parameter-field') || [];
    fields.forEach((field) => {
      field.addEventListener('click', (e) => {
        const index = parseInt((field as HTMLElement).dataset.index || '-1', 10);
        this.selectParameter(index);
      });
    });

    // Keyboard controls
    document.addEventListener('keydown', this.handleKeydown);
  }

  /**
   * Handle slider change
   */
  private handleSliderChange(e: Event): void {
    const param = (e.target as HTMLElement).dataset.param!;
    const value = parseFloat((e.target as HTMLInputElement).value);

    // Update parameter
    (this as any)[param] = value;

    // Update display
    this.updateParameterDisplay(param, value);

    // Redraw transfer curve
    this.updatePlot();

    // Notify parameter change
    this.updateParameter(param, value);
  }

  /**
   * Update parameter display
   */
  private updateParameterDisplay(param: string, value: number): void {
    const field = this.container?.querySelector(`.parameter-field[data-param="${param}"]`);
    if (!field) return;

    const label = field.querySelector('.param-value');
    if (label) {
      const precision = param === 'ceiling' ? 2 : 1;
      const unit = param === 'ceiling' ? 'dB' : 'ms';
      label.textContent = `${value.toFixed(precision)} ${unit}`;
    }

    // Update slider value
    const slider = field.querySelector('.param-slider') as HTMLInputElement;
    if (slider) {
      slider.value = value.toString();
    }
  }

  /**
   * Select parameter by index
   */
  private selectParameter(index: number): void {
    this.selectedParameterIndex = index;

    // Update visual highlighting
    const fields = this.container?.querySelectorAll('.parameter-field') || [];
    fields.forEach((field, idx) => {
      const slider = field.querySelector('.param-slider') as HTMLElement;
      if (slider) {
        if (idx === index) {
          slider.style.accentColor = '#22c55e'; // Green
          field.classList.add('is-selected');
        } else {
          slider.style.accentColor = '';
          field.classList.remove('is-selected');
        }
      }
    });
  }

  /**
   * Clear parameter selection
   */
  private clearParameterSelection(): void {
    this.selectedParameterIndex = -1;

    const fields = this.container?.querySelectorAll('.parameter-field') || [];
    fields.forEach((field) => {
      const slider = field.querySelector('.param-slider') as HTMLElement;
      if (slider) {
        slider.style.accentColor = '';
        field.classList.remove('is-selected');
      }
    });
  }

  /**
   * Handle keyboard shortcuts
   */
  private handleKeydown = (e: KeyboardEvent): void => {
    // Ignore if typing in input
    const target = e.target as HTMLElement;
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT') {
      return;
    }

    // ESC - clear selection
    if (e.key === 'Escape') {
      e.preventDefault();
      this.clearParameterSelection();
      return;
    }

    // 1-3 - select parameter
    const numKey = parseInt(e.key, 10);
    if (numKey >= 1 && numKey <= 3) {
      e.preventDefault();
      this.selectParameter(numKey - 1);
      return;
    }

    // Shift+ArrowLeft / Shift+ArrowRight - adjust selected parameter
    if (this.selectedParameterIndex >= 0 && e.shiftKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();

      const paramName = this.parameterOrder[this.selectedParameterIndex];
      const range = this.parameterRanges[paramName as keyof typeof this.parameterRanges];
      const currentValue = (this as any)[paramName];

      const delta = e.key === 'ArrowRight' ? range.step : -range.step;
      const newValue = Math.max(range.min, Math.min(range.max, currentValue + delta));

      // Update parameter
      (this as any)[paramName] = newValue;

      // Update display
      this.updateParameterDisplay(paramName, newValue);

      // Redraw transfer curve
      this.updatePlot();

      // Notify parameter change
      this.updateParameter(paramName, newValue);
    }
  }

  /**
   * Start rendering loop
   */
  private startRendering(): void {
    const render = () => {
      this.renderMeters();
      this.animationFrameId = requestAnimationFrame(render);
    };
    this.animationFrameId = requestAnimationFrame(render);
  }

  /**
   * Stop rendering loop
   */
  private stopRendering(): void {
    if (this.animationFrameId !== null) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
  }

  /**
   * Render both meters
   */
  private renderMeters(): void {
    this.renderGRMeter();
    this.renderPeakMeter();

    // Update peak hold decay
    const now = Date.now();
    if (now - this.peakHoldTimer > 2000) { // 2 second hold
      this.peakHold = Math.max(-Infinity, this.peakHold - 0.5);
    }
  }

  /**
   * Render gain reduction meter
   */
  private renderGRMeter(): void {
    if (!this.grMeterCanvas || !this.grMeterCtx) return;

    const dpr = window.devicePixelRatio || 1;
    const width = this.grMeterCanvas.width / dpr;
    const height = this.grMeterCanvas.height / dpr;

    // Clear
    this.grMeterCtx.fillStyle = '#2a2a2a';
    this.grMeterCtx.fillRect(0, 0, width, height);

    // Draw scale (0 to -20 dB)
    this.drawMeterScale(this.grMeterCtx, width, height, 20);

    // Draw gain reduction bar
    if (this.currentGainReduction < 0) {
      const grHeight = (Math.abs(this.currentGainReduction) / 20) * (height - 20);

      // Color based on amount
      let color: string;
      if (Math.abs(this.currentGainReduction) < 3) {
        color = '#22c55e'; // Green
      } else if (Math.abs(this.currentGainReduction) < 6) {
        color = '#eab308'; // Yellow
      } else if (Math.abs(this.currentGainReduction) < 10) {
        color = '#f59e0b'; // Orange
      } else {
        color = '#ef4444'; // Red
      }

      this.grMeterCtx.fillStyle = color;
      this.grMeterCtx.fillRect((width - 30) / 2, height - 10 - grHeight, 30, grHeight);
    }

    // Update numeric display
    const grValue = this.container?.querySelector('.gr-value') as HTMLElement;
    if (grValue) {
      grValue.textContent = `${Math.abs(this.currentGainReduction).toFixed(1)} dB`;
    }
  }

  /**
   * Render peak meter
   */
  private renderPeakMeter(): void {
    if (!this.peakMeterCanvas || !this.peakMeterCtx) return;

    const dpr = window.devicePixelRatio || 1;
    const width = this.peakMeterCanvas.width / dpr;
    const height = this.peakMeterCanvas.height / dpr;

    // Clear
    this.peakMeterCtx.fillStyle = '#2a2a2a';
    this.peakMeterCtx.fillRect(0, 0, width, height);

    // Draw scale (-12 to 0 dB)
    this.drawMeterScale(this.peakMeterCtx, width, height, 12, -12);

    // Draw peak bar
    if (this.currentPeak > -Infinity) {
      const peakNorm = Math.max(0, Math.min(1, (this.currentPeak - (-12)) / 12));
      const peakHeight = peakNorm * (height - 20);

      // Gradient from green to red
      const gradient = this.peakMeterCtx.createLinearGradient(0, height - 10, 0, 10);
      gradient.addColorStop(0, '#22c55e');
      gradient.addColorStop(0.6, '#eab308');
      gradient.addColorStop(0.85, '#f59e0b');
      gradient.addColorStop(1, '#ef4444');

      this.peakMeterCtx.fillStyle = gradient;
      this.peakMeterCtx.fillRect((width - 30) / 2, height - 10 - peakHeight, 30, peakHeight);
    }

    // Draw peak hold line
    if (this.peakHold > -Infinity) {
      const peakHoldNorm = Math.max(0, Math.min(1, (this.peakHold - (-12)) / 12));
      const peakHoldY = height - 10 - peakHoldNorm * (height - 20);

      this.peakMeterCtx.fillStyle = this.peakHold > this.ceiling ? '#ef4444' : '#4a9eff';
      this.peakMeterCtx.fillRect((width - 40) / 2, peakHoldY - 1, 40, 2);
    }

    // Draw ceiling line
    const ceilingNorm = Math.max(0, Math.min(1, (this.ceiling - (-12)) / 12));
    const ceilingY = height - 10 - ceilingNorm * (height - 20);

    this.peakMeterCtx.strokeStyle = '#ef4444';
    this.peakMeterCtx.lineWidth = 2;
    this.peakMeterCtx.setLineDash([4, 4]);
    this.peakMeterCtx.beginPath();
    this.peakMeterCtx.moveTo(0, ceilingY);
    this.peakMeterCtx.lineTo(width, ceilingY);
    this.peakMeterCtx.stroke();
    this.peakMeterCtx.setLineDash([]);

    // Update numeric display
    const peakValue = this.container?.querySelector('.peak-value') as HTMLElement;
    if (peakValue) {
      peakValue.textContent = this.currentPeak === -Infinity ? '-∞' : `${this.currentPeak.toFixed(1)} dB`;
    }
  }

  /**
   * Draw meter scale
   */
  private drawMeterScale(ctx: CanvasRenderingContext2D, width: number, height: number, range: number, minDb: number = 0): void {
    ctx.strokeStyle = '#404040';
    ctx.lineWidth = 1;
    ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
    ctx.font = '8px sans-serif';
    ctx.textAlign = 'left';
    ctx.textBaseline = 'middle';

    const markers = range === 20
      ? [0, -3, -6, -10, -15, -20]
      : [0, -3, -6, -9, -12];

    markers.forEach((db) => {
      const norm = (db - minDb) / range;
      const y = height - 10 - norm * (height - 20);

      ctx.beginPath();
      ctx.moveTo(5, y);
      ctx.lineTo(15, y);
      ctx.stroke();

      ctx.fillText(`${db}`, 18, y);
    });
  }

  /**
   * Update metering data (called from external source)
   */
  updateMetering(gainReductionDb: number, peakDb: number): void {
    this.currentGainReduction = Math.min(0, gainReductionDb);
    this.currentPeak = peakDb;

    // Update peak hold
    if (peakDb > this.peakHold) {
      this.peakHold = peakDb;
      this.peakHoldTimer = Date.now();
    }

    // Update peak hold display
    const peakHoldValue = this.container?.querySelector('.peak-hold-value') as HTMLElement;
    if (peakHoldValue) {
      peakHoldValue.textContent = this.peakHold === -Infinity ? '-∞' : `${this.peakHold.toFixed(2)} dB`;
    }

    // Check for clipping
    const wasClipping = this.clipping;
    this.clipping = peakDb > this.ceiling;

    if (this.clipping !== wasClipping) {
      const clippingIndicator = this.container?.querySelector('.clipping-indicator');

      if (this.clipping) {
        if (clippingIndicator) {
          clippingIndicator.textContent = 'YES';
          (clippingIndicator as HTMLElement).style.color = '#ef4444';
        }
      } else {
        if (clippingIndicator) {
          clippingIndicator.textContent = 'NO';
          (clippingIndicator as HTMLElement).style.color = '#22c55e';
        }
      }
    }
  }

  /**
   * Reset peak hold
   */
  resetPeakHold(): void {
    this.peakHold = -Infinity;
    this.peakHoldTimer = 0;
  }

  /**
   * Get parameters
   */
  getParameters() {
    return {
      ceiling: this.ceiling,
      release: this.release,
      lookahead: this.lookahead,
    };
  }

  /**
   * Set parameters
   */
  setParameters(params: Partial<{
    ceiling: number;
    release: number;
    lookahead: number;
  }>): void {
    if (params.ceiling !== undefined) this.ceiling = params.ceiling;
    if (params.release !== undefined) this.release = params.release;
    if (params.lookahead !== undefined) this.lookahead = params.lookahead;

    // Re-render if already initialized
    if (this.container) {
      this.render(this.config.standalone ?? true);
    }
  }

  /**
   * Resize handler
   */
  resize(): void {
    this.setupMeterCanvas(this.grMeterCanvas, 60, 240);
    this.setupMeterCanvas(this.peakMeterCanvas, 60, 240);

    // Resize Plotly graph
    if (this.plotContainer) {
      Plotly.Plots.resize(this.plotContainer);
    }
  }

  /**
   * Get keyboard shortcuts for this plugin
   */
  getShortcuts() {
    return [
      { key: '1-3', description: 'Select parameter' },
      { key: 'Esc', description: 'Clear selection' },
      { key: 'Shift+←→', description: 'Adjust value' },
    ];
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    this.stopRendering();

    // Remove keyboard listener
    document.removeEventListener('keydown', this.handleKeydown);

    // Cleanup Plotly
    if (this.plotContainer) {
      Plotly.purge(this.plotContainer);
    }

    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    super.destroy();
  }
}
