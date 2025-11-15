// Compressor Plugin
// Dynamic range compressor with visual gain reduction metering

import { BasePlugin } from './plugin-base';
import { PluginMenubar } from './plugin-menubar';
import type { PluginMetadata } from './plugin-types';
import Plotly from 'plotly.js-basic-dist-min';

/**
 * Compressor Plugin
 * Dynamic range compression with threshold, ratio, attack, release
 */
export class CompressorPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: 'compressor-plugin',
    name: 'SotF: Compressor',
    category: 'dynamics',
    version: '1.0.0',
  };

  // UI components
  private menubar: PluginMenubar | null = null;

  // UI elements
  private grMeterCanvas: HTMLCanvasElement | null = null;
  private grMeterCtx: CanvasRenderingContext2D | null = null;
  private plotContainer: HTMLElement | null = null;

  // Parameters
  private threshold: number = -20.0;      // dB
  private ratio: number = 4.0;            // n:1
  private attack: number = 5.0;           // ms
  private release: number = 50.0;         // ms
  private knee: number = 3.0;             // dB
  private makeupGain: number = 0.0;       // dB

  // Parameter metadata for keyboard control
  private parameterOrder = ['threshold', 'ratio', 'attack', 'release', 'knee', 'makeupGain'];
  private parameterRanges = {
    threshold: { min: -60, max: 0, step: 0.5 },
    ratio: { min: 1, max: 20, step: 0.5 },
    attack: { min: 0.1, max: 100, step: 1 },
    release: { min: 10, max: 1000, step: 10 },
    knee: { min: 0, max: 12, step: 0.5 },
    makeupGain: { min: 0, max: 24, step: 0.5 },
  };

  // State
  private currentGainReduction: number = 0.0; // dB (negative)
  private animationFrameId: number | null = null;
  private selectedParameterIndex: number = -1; // -1 = none selected

  /**
   * Render a single parameter slider with labels
   */
  private renderParameter(paramName: string, index: number, label: string, unit: string): string {
    const value = (this as any)[paramName];
    const range = this.parameterRanges[paramName as keyof typeof this.parameterRanges];

    // Format value display
    let displayValue = value.toFixed(1);
    if (unit === ':1') {
      displayValue = `${value.toFixed(1)}${unit}`;
    } else {
      displayValue = `${value.toFixed(1)} ${unit}`;
    }

    // Format min/max labels
    let minLabel = range.min.toString();
    let maxLabel = range.max.toString();
    if (unit === ':1') {
      minLabel = `${range.min}:1`;
      maxLabel = `${range.max}:1`;
    } else if (unit === 'dB') {
      minLabel = `${range.min} dB`;
      maxLabel = `${range.max} dB`;
    } else {
      minLabel = `${range.min} ${unit}`;
      maxLabel = `${range.max} ${unit}`;
    }

    return `
      <div class="field parameter-field" data-param="${paramName}" data-index="${index}">
        <label class="label is-small has-text-light has-text-centered">${label}</label>
        <div class="columns is-mobile is-gapless is-vcentered" style="margin-bottom: 0;">
          <div class="column is-narrow" style="min-width: 60px;">
            <div class="is-flex is-flex-direction-column is-justify-content-space-between" style="height: 100%; padding: 4px 0;">
              <p class="has-text-grey-light is-size-7 has-text-right">${maxLabel}</p>
              <p class="has-text-grey-light is-size-7 has-text-right">${minLabel}</p>
            </div>
          </div>
          <div class="column">
            <input
              type="range"
              class="slider is-fullwidth param-slider"
              data-param="${paramName}"
              min="${range.min}"
              max="${range.max}"
              step="${range.step}"
              value="${value}"
            />
          </div>
          <div class="column is-narrow has-text-centered" style="min-width: 80px;">
            <span class="tag is-dark">${displayValue}</span>
          </div>
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
      <div class="is-flex is-flex-direction-column compressor-plugin ${standalone ? 'standalone' : 'embedded'}" style="height: 100%; min-height: 0; background: #1a1a1a;">
        ${standalone ? '<div class="compressor-menubar-container"></div>' : ''}
        <div class="is-flex is-flex-direction-column is-flex-grow-1" style="min-height: 0; overflow: hidden; padding: 0; margin: 0;">
          <!-- Bulma Columns -->
          <div class="columns is-gapless" style="flex: 1; min-height: 0;">
            <!-- Column 1: Parameters (25%) -->
            <div class="column is-8 is-flex is-flex-direction-column">
              <div class="box" style="height: 100%; margin: 0 !important; background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0;">
                <h4 class="title is-6 has-text-light">Dynamics Control</h4>
                <div class="is-flex is-flex-direction-column">
                  <div class="columns is-gapless">
                    <div class="column is-2">
                      ${this.renderParameter('threshold', 0, 'Threshold', 'dB')}
  	            </div>
                    <div class="column is-2">
                      ${this.renderParameter('ratio', 1, 'Ratio', ':1')}
  	            </div>
                    <div class="column is-2">
                      ${this.renderParameter('attack', 2, 'Attack', 'ms')}
  	            </div>
                    <div class="column is-2">
                      ${this.renderParameter('release', 3, 'Release', 'ms')}
  	           </div>
                   <div class="column is-2">
                     ${this.renderParameter('knee', 4, 'Knee', 'dB')}
  	           </div>
                   <div class="column is-2">
                     ${this.renderParameter('makeupGain', 5, 'Makeup Gain', 'dB')}
  	           </div>
  	          </div>
                </div>
              </div>
            </div>

            <!-- Column 2: Gain Reduction Meter (25%) -->
            <div class="column is-1 is-flex is-flex-direction-column">
              <div class="box" style="height: 100%; margin: 0 !important; background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0;">
                <h4 class="title is-6 has-text-light has-text-centered">Gain Reduction</h4>
                <div class="is-flex is-flex-direction-column is-align-items-center" style="gap: 8px; height: calc(100% - 40px);">
                  <canvas class="gr-meter-canvas" width="80" height="280"></canvas>
                  <div class="gr-value has-text-light has-text-centered" style="margin-top: 8px; font-size: 14px; font-weight: 600;">${Math.abs(this.currentGainReduction).toFixed(1)} dB</div>
                </div>
              </div>
            </div>

            <!-- Column 3: Transfer Curve (50%) -->
            <div class="column is-3 is-flex is-flex-direction-column">
              <div class="box" style="height: 100%; margin: 0 !important; padding: 12px; background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0;">
                <h4 class="title is-6 has-text-light has-text-centered plugin-section-header">Transfer Curve</h4>
                <div id="compressor-plot-${this.metadata.id}" class="is-flex is-flex-direction-column is-align-items-center transfer-curve-container" style="gap: 8px;"></div>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector('.compressor-menubar-container') as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name);
      }
    }

    // Cache elements
    this.grMeterCanvas = this.container.querySelector('.gr-meter-canvas') as HTMLCanvasElement;
    this.grMeterCtx = this.grMeterCanvas?.getContext('2d') || null;
    this.plotContainer = this.container.querySelector(`#compressor-plot-${this.metadata.id}`) as HTMLElement;

    // Setup canvases
    this.setupGRMeter();

    // Setup Plotly graph
    this.updatePlot();

    this.attachEventListeners();
    this.startRendering();
  }

  /**
   * Setup gain reduction meter canvas
   */
  private setupGRMeter(): void {
    if (!this.grMeterCanvas || !this.grMeterCtx) return;

    const dpr = window.devicePixelRatio || 1;
    const width = 100;
    const height = 300;

    this.grMeterCanvas.width = width * dpr;
    this.grMeterCanvas.height = height * dpr;

    this.grMeterCtx = this.grMeterCanvas.getContext('2d');
    if (this.grMeterCtx) {
      this.grMeterCtx.scale(dpr, dpr);
    }
  }

  /**
   * Update Plotly transfer curve
   */
  private updatePlot(): void {
    if (!this.plotContainer) return;

    const dbRange = 60; // -60 to 0 dB
    const numPoints = 200;

    // Generate input dB values
    const inputDb: number[] = [];
    const outputDb: number[] = [];
    const referenceDb: number[] = [];

    for (let i = 0; i <= numPoints; i++) {
      const inputVal = (i / numPoints) * dbRange - dbRange; // -60 to 0 dB
      inputDb.push(inputVal);
      referenceDb.push(inputVal); // 1:1 reference line

      let outputVal: number;

      if (inputVal < this.threshold - this.knee / 2) {
        // Below threshold - no compression
        outputVal = inputVal;
      } else if (inputVal > this.threshold + this.knee / 2) {
        // Above threshold - full compression
        const excess = inputVal - this.threshold;
        outputVal = this.threshold + excess / this.ratio;
      } else {
        // Knee region - smooth transition
        const kneeInput = inputVal - (this.threshold - this.knee / 2);
        const kneeRatio = kneeInput / this.knee;
        const kneeOutput = kneeRatio * kneeRatio / 2;
        outputVal = inputVal + kneeOutput * (1 / this.ratio - 1) * this.knee;
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
      // Compression curve
      {
        x: inputDb,
        y: outputDb,
        type: 'scatter',
        mode: 'lines',
        name: 'Transfer Curve',
        line: {
          color: '#4a9eff',
          width: 3,
        },
        hovertemplate: 'Input: %{x:.1f} dB<br>Output: %{y:.1f} dB<extra></extra>',
      } as Plotly.Data,
    ];

    // Add threshold lines
    traces.push(
      // Vertical threshold line
      {
        x: [this.threshold, this.threshold],
        y: [-60, 0],
        type: 'scatter',
        mode: 'lines',
        name: 'Threshold',
        line: {
          color: 'rgba(239, 68, 68, 0.6)',
          width: 2,
          dash: 'dash',
        },
        hoverinfo: 'skip',
      } as Plotly.Data,
      // Horizontal threshold line
      {
        x: [-60, 0],
        y: [this.threshold, this.threshold],
        type: 'scatter',
        mode: 'lines',
        name: 'Threshold (ref)',
        line: {
          color: 'rgba(239, 68, 68, 0.6)',
          width: 2,
          dash: 'dash',
        },
        showlegend: false,
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
        range: [-60, 0],
        dtick: 10,
      },
      yaxis: {
        title: { text: 'Output (dB)' },
        gridcolor: 'rgba(255, 255, 255, 0.1)',
        zerolinecolor: 'rgba(255, 255, 255, 0.2)',
        range: [-60, 0],
        dtick: 10,
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
    this.redrawTransferCurve();

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
      if (param === 'ratio') {
        label.textContent = `${value.toFixed(1)}:1`;
      } else if (param === 'attack' || param === 'release') {
        label.textContent = `${value.toFixed(1)} ms`;
      } else {
        label.textContent = `${value.toFixed(1)} dB`;
      }
    }

    // Update slider value
    const slider = field.querySelector('.param-slider') as HTMLInputElement;
    if (slider) {
      slider.value = value.toString();
    }
  }

  /**
   * Redraw transfer curve
   */
  private redrawTransferCurve(): void {
    this.updatePlot();
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

    // 1-6 - select parameter
    const numKey = parseInt(e.key, 10);
    if (numKey >= 1 && numKey <= 6) {
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
      this.redrawTransferCurve();

      // Notify parameter change
      this.updateParameter(paramName, newValue);
    }
  }

  /**
   * Start rendering loop for gain reduction meter
   */
  private startRendering(): void {
    const render = () => {
      this.renderGRMeter();
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

    // Draw scale markers (0 to -30 dB)
    this.grMeterCtx.strokeStyle = '#404040';
    this.grMeterCtx.lineWidth = 1;
    this.grMeterCtx.fillStyle = 'rgba(255, 255, 255, 0.7)';
    this.grMeterCtx.font = '9px sans-serif';
    this.grMeterCtx.textAlign = 'right';
    this.grMeterCtx.textBaseline = 'middle';

    const markers = [0, -3, -6, -10, -15, -20, -30];
    markers.forEach((db) => {
      const y = ((Math.abs(db) / 30) * (height - 20)) + 10;

      this.grMeterCtx!.beginPath();
      this.grMeterCtx!.moveTo(width - 40, y);
      this.grMeterCtx!.lineTo(width - 10, y);
      this.grMeterCtx!.stroke();

      this.grMeterCtx!.fillText(`${db}`, width - 45, y);
    });

    // Draw gain reduction bar
    if (this.currentGainReduction < 0) {
      const grHeight = (Math.abs(this.currentGainReduction) / 30) * (height - 20);

      // Gradient from green to red
      const gradient = this.grMeterCtx.createLinearGradient(0, height - 10, 0, 10);
      gradient.addColorStop(0, '#22c55e');
      gradient.addColorStop(0.3, '#eab308');
      gradient.addColorStop(0.6, '#f59e0b');
      gradient.addColorStop(1, '#ef4444');

      this.grMeterCtx.fillStyle = gradient;
      this.grMeterCtx.fillRect(width - 35, height - 10 - grHeight, 20, grHeight);
    }

    // Update numeric display
    const grValue = this.container?.querySelector('.gr-value') as HTMLElement;
    if (grValue) {
      grValue.textContent = `${Math.abs(this.currentGainReduction).toFixed(1)} dB`;
    }
  }

  /**
   * Update gain reduction (called from external metering)
   */
  updateGainReduction(gainReductionDb: number): void {
    this.currentGainReduction = Math.min(0, gainReductionDb);
  }

  /**
   * Get parameters
   */
  getParameters() {
    return {
      threshold: this.threshold,
      ratio: this.ratio,
      attack: this.attack,
      release: this.release,
      knee: this.knee,
      makeupGain: this.makeupGain,
    };
  }

  /**
   * Set parameters
   */
  setParameters(params: Partial<{
    threshold: number;
    ratio: number;
    attack: number;
    release: number;
    knee: number;
    makeupGain: number;
  }>): void {
    if (params.threshold !== undefined) this.threshold = params.threshold;
    if (params.ratio !== undefined) this.ratio = params.ratio;
    if (params.attack !== undefined) this.attack = params.attack;
    if (params.release !== undefined) this.release = params.release;
    if (params.knee !== undefined) this.knee = params.knee;
    if (params.makeupGain !== undefined) this.makeupGain = params.makeupGain;

    // Re-render if already initialized
    if (this.container) {
      this.render(this.config.standalone ?? true);
    }
  }

  /**
   * Resize handler
   */
  resize(): void {
    this.setupGRMeter();

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
      { key: '1-6', description: 'Select parameter' },
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
