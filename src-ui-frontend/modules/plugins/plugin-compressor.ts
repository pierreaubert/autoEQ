// Compressor Plugin
// Dynamic range compressor with visual gain reduction metering

import { BasePlugin } from './plugin-base';
import { PluginMenubar } from './plugin-menubar';
import type { PluginMetadata } from './plugin-types';

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

  // Parameters
  private threshold: number = -20.0;      // dB
  private ratio: number = 4.0;            // n:1
  private attack: number = 5.0;           // ms
  private release: number = 50.0;         // ms
  private knee: number = 3.0;             // dB
  private makeupGain: number = 0.0;       // dB

  // State
  private currentGainReduction: number = 0.0; // dB (negative)
  private animationFrameId: number | null = null;

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="compressor-plugin ${standalone ? 'standalone' : 'embedded'}">
        ${standalone ? '<div class="compressor-menubar-container"></div>' : ''}
        <div class="compressor-content">
          <!-- Main Grid: Parameters + Meter -->
          <div class="compressor-grid">
            <!-- Left: Parameters -->
            <div class="compressor-parameters">
              <h4 class="section-title">Dynamics Control</h4>

              <div class="parameter-group">
                <label>
                  Threshold
                  <span class="param-value">${this.threshold.toFixed(1)} dB</span>
                </label>
                <input type="range" class="param-slider" data-param="threshold"
                       min="-60" max="0" step="0.1" value="${this.threshold}" />
                <div class="param-scale">
                  <span>-60</span>
                  <span>0</span>
                </div>
              </div>

              <div class="parameter-group">
                <label>
                  Ratio
                  <span class="param-value">${this.ratio.toFixed(1)}:1</span>
                </label>
                <input type="range" class="param-slider" data-param="ratio"
                       min="1" max="20" step="0.1" value="${this.ratio}" />
                <div class="param-scale">
                  <span>1:1</span>
                  <span>20:1</span>
                </div>
              </div>

              <div class="parameter-group">
                <label>
                  Attack
                  <span class="param-value">${this.attack.toFixed(1)} ms</span>
                </label>
                <input type="range" class="param-slider" data-param="attack"
                       min="0.1" max="100" step="0.1" value="${this.attack}" />
                <div class="param-scale">
                  <span>0.1</span>
                  <span>100 ms</span>
                </div>
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
              </div>

              <div class="parameter-group">
                <label>
                  Knee
                  <span class="param-value">${this.knee.toFixed(1)} dB</span>
                </label>
                <input type="range" class="param-slider" data-param="knee"
                       min="0" max="12" step="0.1" value="${this.knee}" />
                <div class="param-scale">
                  <span>Hard</span>
                  <span>Soft</span>
                </div>
              </div>

              <div class="parameter-group">
                <label>
                  Makeup Gain
                  <span class="param-value">${this.makeupGain.toFixed(1)} dB</span>
                </label>
                <input type="range" class="param-slider" data-param="makeupGain"
                       min="0" max="24" step="0.1" value="${this.makeupGain}" />
                <div class="param-scale">
                  <span>0</span>
                  <span>+24 dB</span>
                </div>
              </div>
            </div>

            <!-- Right: Gain Reduction Meter -->
            <div class="compressor-meter-section">
              <h4 class="section-title">Gain Reduction</h4>
              <div class="gr-meter-container">
                <canvas class="gr-meter-canvas" width="100" height="300"></canvas>
                <div class="gr-value">${Math.abs(this.currentGainReduction).toFixed(1)} dB</div>
              </div>

              <!-- Transfer Curve Visualization -->
              <div class="transfer-curve-container">
                <canvas class="transfer-curve-canvas" width="200" height="200"></canvas>
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

    const transferCanvas = this.container.querySelector('.transfer-curve-canvas') as HTMLCanvasElement;

    // Setup canvases
    this.setupGRMeter();
    this.setupTransferCurve(transferCanvas);

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
   * Setup transfer curve canvas
   */
  private setupTransferCurve(canvas: HTMLCanvasElement | null): void {
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const width = 200;
    const height = 200;

    canvas.width = width * dpr;
    canvas.height = height * dpr;

    const scaledCtx = canvas.getContext('2d');
    if (scaledCtx) {
      scaledCtx.scale(dpr, dpr);
    }

    this.drawTransferCurve(canvas, ctx, width, height);
  }

  /**
   * Draw transfer curve (input vs output)
   */
  private drawTransferCurve(canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D, width: number, height: number): void {
    // Clear
    ctx.fillStyle = '#1a1a1a';
    ctx.fillRect(0, 0, width, height);

    // Draw grid
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    ctx.lineWidth = 1;

    // Vertical lines
    for (let i = 0; i <= 4; i++) {
      const x = (i / 4) * width;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }

    // Horizontal lines
    for (let i = 0; i <= 4; i++) {
      const y = (i / 4) * height;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    }

    // Draw 1:1 reference line (no compression)
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
    ctx.lineWidth = 1;
    ctx.setLineDash([2, 2]);
    ctx.beginPath();
    ctx.moveTo(0, height);
    ctx.lineTo(width, 0);
    ctx.stroke();
    ctx.setLineDash([]);

    // Draw compression curve
    ctx.strokeStyle = '#4a9eff';
    ctx.lineWidth = 2;
    ctx.beginPath();

    const dbRange = 60; // -60 to 0 dB
    const thresholdNorm = (this.threshold + dbRange) / dbRange;
    const kneeWidth = this.knee / dbRange;

    for (let i = 0; i <= 100; i++) {
      const inputNorm = i / 100; // 0 to 1
      const inputDb = inputNorm * dbRange - dbRange; // -60 to 0 dB

      let outputDb: number;

      if (inputDb < this.threshold - this.knee / 2) {
        // Below threshold - no compression
        outputDb = inputDb;
      } else if (inputDb > this.threshold + this.knee / 2) {
        // Above threshold - full compression
        const excess = inputDb - this.threshold;
        outputDb = this.threshold + excess / this.ratio;
      } else {
        // Knee region - smooth transition
        const kneeInput = inputDb - (this.threshold - this.knee / 2);
        const kneeRatio = kneeInput / this.knee;
        const kneeOutput = kneeRatio * kneeRatio / 2;
        outputDb = inputDb + kneeOutput * (1 / this.ratio - 1) * this.knee;
      }

      const outputNorm = (outputDb + dbRange) / dbRange;

      const x = inputNorm * width;
      const y = (1 - outputNorm) * height;

      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }

    ctx.stroke();

    // Draw threshold line
    ctx.strokeStyle = 'rgba(239, 68, 68, 0.6)';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 4]);

    const thresholdX = thresholdNorm * width;
    ctx.beginPath();
    ctx.moveTo(thresholdX, 0);
    ctx.lineTo(thresholdX, height);
    ctx.stroke();

    const thresholdY = (1 - thresholdNorm) * height;
    ctx.beginPath();
    ctx.moveTo(0, thresholdY);
    ctx.lineTo(width, thresholdY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Labels
    ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'left';
    ctx.textBaseline = 'bottom';
    ctx.fillText('Input (dB)', 5, height - 5);

    ctx.save();
    ctx.translate(10, height / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.textAlign = 'center';
    ctx.fillText('Output (dB)', 0, 0);
    ctx.restore();
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
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
          if (param === 'ratio') {
            label.textContent = `${value.toFixed(1)}:1`;
          } else if (param === 'attack' || param === 'release') {
            label.textContent = `${value.toFixed(1)} ms`;
          } else {
            label.textContent = `${value.toFixed(1)} dB`;
          }
        }

        // Redraw transfer curve
        const transferCanvas = this.container?.querySelector('.transfer-curve-canvas') as HTMLCanvasElement;
        if (transferCanvas) {
          const ctx = transferCanvas.getContext('2d');
          if (ctx) {
            const dpr = window.devicePixelRatio || 1;
            this.drawTransferCurve(transferCanvas, ctx, 200, 200);
          }
        }

        // Notify parameter change
        this.updateParameter(param, value);
      });
    });
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
    const transferCanvas = this.container?.querySelector('.transfer-curve-canvas') as HTMLCanvasElement;
    if (transferCanvas) {
      this.setupTransferCurve(transferCanvas);
    }
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
