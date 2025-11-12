// Limiter Plugin
// Brick-wall limiter with true peak detection and lookahead

import { BasePlugin } from './plugin-base';
import { PluginMenubar } from './plugin-menubar';
import type { PluginMetadata } from './plugin-types';

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

  // Parameters
  private ceiling: number = -0.1;         // dB (max output level)
  private release: number = 100.0;        // ms
  private lookahead: number = 5.0;        // ms
  private truePeakMode: boolean = true;   // True peak vs sample peak

  // State
  private currentGainReduction: number = 0.0; // dB (negative)
  private currentPeak: number = -Infinity;    // dB
  private peakHold: number = -Infinity;       // dB
  private peakHoldTimer: number = 0;
  private clipping: boolean = false;
  private animationFrameId: number | null = null;

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="limiter-plugin ${standalone ? 'standalone' : 'embedded'}">
        ${standalone ? '<div class="limiter-menubar-container"></div>' : ''}
        <div class="limiter-content">
          <!-- Grid: Parameters + Meters -->
          <div class="limiter-grid">
            <!-- Left: Parameters & Controls -->
            <div class="limiter-parameters">
              <h4 class="section-title">Limiter Settings</h4>

              <div class="parameter-group">
                <label>
                  Ceiling
                  <span class="param-value">${this.ceiling.toFixed(2)} dB</span>
                </label>
                <input type="range" class="param-slider" data-param="ceiling"
                       min="-12" max="0" step="0.01" value="${this.ceiling}" />
                <div class="param-scale">
                  <span>-12</span>
                  <span>0 dB</span>
                </div>
                <p class="param-hint">Maximum output level (brick wall)</p>
              </div>

              <div class="parameter-group">
                <label>
                  Release
                  <span class="param-value">${this.release.toFixed(1)} ms</span>
                </label>
                <input type="range" class="param-slider" data-param="release"
                       min="10" max="1000" step="1" value="${this.release}" />
                <div class="param-scale">
                  <span>10</span>
                  <span>1000 ms</span>
                </div>
                <p class="param-hint">Time to return to unity gain</p>
              </div>

              <div class="parameter-group">
                <label>
                  Lookahead
                  <span class="param-value">${this.lookahead.toFixed(1)} ms</span>
                </label>
                <input type="range" class="param-slider" data-param="lookahead"
                       min="0" max="10" step="0.1" value="${this.lookahead}" />
                <div class="param-scale">
                  <span>0</span>
                  <span>10 ms</span>
                </div>
                <p class="param-hint">Anticipate peaks for transparent limiting</p>
              </div>

              <div class="parameter-group checkbox-group">
                <label class="checkbox-label">
                  <input type="checkbox" class="param-checkbox" data-param="truePeakMode"
                         ${this.truePeakMode ? 'checked' : ''} />
                  <span>True Peak Detection</span>
                </label>
                <p class="param-hint">Detect inter-sample peaks (ITU-R BS.1770)</p>
              </div>

              <!-- Status Indicators -->
              <div class="limiter-status">
                <div class="status-item">
                  <span class="status-label">Ceiling:</span>
                  <span class="status-value">${this.ceiling.toFixed(2)} dB</span>
                </div>
                <div class="status-item">
                  <span class="status-label">Peak Hold:</span>
                  <span class="status-value peak-hold-value">${this.peakHold === -Infinity ? '-∞' : this.peakHold.toFixed(2)} dB</span>
                </div>
                <div class="status-item ${this.clipping ? 'clipping' : ''}">
                  <span class="status-label">Clipping:</span>
                  <span class="status-value clipping-indicator">${this.clipping ? 'YES' : 'NO'}</span>
                </div>
              </div>
            </div>

            <!-- Right: Meters -->
            <div class="limiter-meters">
              <!-- Gain Reduction Meter -->
              <div class="meter-section">
                <h4 class="section-title">Gain Reduction</h4>
                <div class="meter-container">
                  <canvas class="gr-meter-canvas" width="80" height="250"></canvas>
                  <div class="meter-value">${Math.abs(this.currentGainReduction).toFixed(1)} dB</div>
                </div>
              </div>

              <!-- Peak Meter -->
              <div class="meter-section">
                <h4 class="section-title">Output Peak</h4>
                <div class="meter-container">
                  <canvas class="peak-meter-canvas" width="80" height="250"></canvas>
                  <div class="meter-value">${this.currentPeak === -Infinity ? '-∞' : this.currentPeak.toFixed(1)} dB</div>
                </div>
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

    // Setup canvases
    this.setupMeterCanvas(this.grMeterCanvas);
    this.setupMeterCanvas(this.peakMeterCanvas);

    this.attachEventListeners();
    this.startRendering();
  }

  /**
   * Setup meter canvas
   */
  private setupMeterCanvas(canvas: HTMLCanvasElement | null): void {
    if (!canvas) return;

    const dpr = window.devicePixelRatio || 1;
    const width = 80;
    const height = 250;

    canvas.width = width * dpr;
    canvas.height = height * dpr;

    const ctx = canvas.getContext('2d');
    if (ctx) {
      ctx.scale(dpr, dpr);
    }
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Sliders
    const sliders = this.container?.querySelectorAll('.param-slider') || [];
    sliders.forEach((slider) => {
      slider.addEventListener('input', (e) => {
        const param = (e.target as HTMLElement).dataset.param!;
        const value = parseFloat((e.target as HTMLInputElement).value);

        // Update parameter
        (this as any)[param] = value;

        // Update display
        const label = (e.target as HTMLElement).parentElement?.querySelector('.param-value');
        if (label) {
          if (param === 'release' || param === 'lookahead') {
            label.textContent = `${value.toFixed(1)} ms`;
          } else {
            label.textContent = `${value.toFixed(2)} dB`;
          }
        }

        // Update status
        if (param === 'ceiling') {
          const statusValue = this.container?.querySelector('.status-item .status-value');
          if (statusValue) {
            statusValue.textContent = `${value.toFixed(2)} dB`;
          }
        }

        // Notify parameter change
        this.updateParameter(param, value);
      });
    });

    // Checkbox
    const checkbox = this.container?.querySelector('.param-checkbox') as HTMLInputElement;
    if (checkbox) {
      checkbox.addEventListener('change', (e) => {
        const param = (e.target as HTMLElement).dataset.param!;
        const value = (e.target as HTMLInputElement).checked;

        (this as any)[param] = value;
        this.updateParameter(param, value);
      });
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
    const grValue = this.container?.querySelector('.limiter-meters .meter-value') as HTMLElement;
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
    const peakValue = this.container?.querySelectorAll('.limiter-meters .meter-value')[1] as HTMLElement;
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
      const statusItem = this.container?.querySelector('.status-item.clipping');
      const clippingIndicator = this.container?.querySelector('.clipping-indicator');

      if (this.clipping) {
        statusItem?.classList.add('clipping');
        if (clippingIndicator) clippingIndicator.textContent = 'YES';
      } else {
        statusItem?.classList.remove('clipping');
        if (clippingIndicator) clippingIndicator.textContent = 'NO';
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
      truePeakMode: this.truePeakMode,
    };
  }

  /**
   * Set parameters
   */
  setParameters(params: Partial<{
    ceiling: number;
    release: number;
    lookahead: number;
    truePeakMode: boolean;
  }>): void {
    if (params.ceiling !== undefined) this.ceiling = params.ceiling;
    if (params.release !== undefined) this.release = params.release;
    if (params.lookahead !== undefined) this.lookahead = params.lookahead;
    if (params.truePeakMode !== undefined) this.truePeakMode = params.truePeakMode;

    // Re-render if already initialized
    if (this.container) {
      this.render(this.config.standalone ?? true);
    }
  }

  /**
   * Resize handler
   */
  resize(): void {
    this.setupMeterCanvas(this.grMeterCanvas);
    this.setupMeterCanvas(this.peakMeterCanvas);
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    this.stopRendering();

    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    super.destroy();
  }
}
