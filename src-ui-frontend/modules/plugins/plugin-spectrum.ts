// Spectrum Analyzer Plugin
// Wraps the existing spectrum analyzer as a plugin

import { BasePlugin } from './plugin-base';
import { PluginMenubar } from './plugin-menubar';
import { SpectrumAnalyzerComponent, type SpectrumDisplayConfig } from '../audio-player/spectrum-analyzer';
import type { PluginMetadata } from './plugin-types';

/**
 * Spectrum Analyzer Plugin
 * FFT-based frequency spectrum visualization
 */
export class SpectrumPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: 'spectrum-plugin',
    name: 'SotF: Spectrum',
    category: 'analyzer',
    version: '1.0.0',
  };

  // UI components
  private menubar: PluginMenubar | null = null;
  private spectrumAnalyzer: SpectrumAnalyzerComponent | null = null;

  // UI elements
  private spectrumCanvas: HTMLCanvasElement | null = null;

  // Configuration
  private minFreq: number = 20;
  private maxFreq: number = 20000;
  private dbRange: number = 60;
  private pollInterval: number = 100;
  private colorScheme: 'light' | 'dark' = 'dark';

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="spectrum-plugin ${standalone ? 'standalone' : 'embedded'}">
        ${standalone ? '<div class="spectrum-menubar-container"></div>' : ''}
        <div class="spectrum-content">
          <!-- Spectrum Display -->
          <div class="spectrum-display-container">
            <canvas class="spectrum-canvas" width="800" height="300"></canvas>
          </div>

          <!-- Controls -->
          <div class="spectrum-controls">
            <div class="control-row">
              <div class="control-group">
                <label>
                  Min Frequency
                  <span class="param-value">${this.minFreq} Hz</span>
                </label>
                <input type="range" class="param-slider" data-param="minFreq"
                       min="10" max="200" step="10" value="${this.minFreq}" />
              </div>

              <div class="control-group">
                <label>
                  Max Frequency
                  <span class="param-value">${this.maxFreq} Hz</span>
                </label>
                <input type="range" class="param-slider" data-param="maxFreq"
                       min="5000" max="24000" step="1000" value="${this.maxFreq}" />
              </div>

              <div class="control-group">
                <label>
                  dB Range
                  <span class="param-value">${this.dbRange} dB</span>
                </label>
                <input type="range" class="param-slider" data-param="dbRange"
                       min="30" max="120" step="10" value="${this.dbRange}" />
              </div>

              <div class="control-group">
                <label>
                  Update Rate
                  <span class="param-value">${this.pollInterval} ms</span>
                </label>
                <input type="range" class="param-slider" data-param="pollInterval"
                       min="50" max="500" step="50" value="${this.pollInterval}" />
              </div>
            </div>

            <div class="control-row">
              <button class="control-btn start-btn">Start</button>
              <button class="control-btn stop-btn" disabled>Stop</button>
              <button class="control-btn reset-btn">Reset</button>
            </div>
          </div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector('.spectrum-menubar-container') as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name, {
          showPresets: false,
          showMatrix: false,
          showMuteSolo: false,
        });
      }
    }

    // Cache elements
    this.spectrumCanvas = this.container.querySelector('.spectrum-canvas') as HTMLCanvasElement;

    // Initialize spectrum analyzer
    if (this.spectrumCanvas) {
      const config: SpectrumDisplayConfig = {
        canvas: this.spectrumCanvas,
        pollInterval: this.pollInterval,
        minFreq: this.minFreq,
        maxFreq: this.maxFreq,
        dbRange: this.dbRange,
        colorScheme: this.colorScheme,
        showLabels: true,
        showGrid: true,
      };

      this.spectrumAnalyzer = new SpectrumAnalyzerComponent(config);
    }

    this.attachEventListeners();
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Parameter sliders
    const sliders = this.container?.querySelectorAll('.param-slider') || [];
    sliders.forEach((slider) => {
      slider.addEventListener('input', (e) => {
        const param = (e.target as HTMLElement).dataset.param!;
        const value = parseInt((e.target as HTMLInputElement).value, 10);

        // Update parameter
        (this as any)[param] = value;

        // Update display
        const label = (e.target as HTMLElement).parentElement?.querySelector('.param-value');
        if (label) {
          if (param === 'pollInterval') {
            label.textContent = `${value} ms`;
          } else if (param === 'dbRange') {
            label.textContent = `${value} dB`;
          } else {
            label.textContent = `${value} Hz`;
          }
        }

        // Recreate spectrum analyzer with new settings
        if (param !== 'pollInterval') {
          this.recreateAnalyzer();
        }

        // Notify parameter change
        this.updateParameter(param, value);
      });
    });

    // Control buttons
    const startBtn = this.container?.querySelector('.start-btn') as HTMLButtonElement;
    const stopBtn = this.container?.querySelector('.stop-btn') as HTMLButtonElement;
    const resetBtn = this.container?.querySelector('.reset-btn') as HTMLButtonElement;

    if (startBtn) {
      startBtn.addEventListener('click', async () => {
        await this.start();
        startBtn.disabled = true;
        if (stopBtn) stopBtn.disabled = false;
      });
    }

    if (stopBtn) {
      stopBtn.addEventListener('click', async () => {
        await this.stop();
        if (startBtn) startBtn.disabled = false;
        stopBtn.disabled = true;
      });
    }

    if (resetBtn) {
      resetBtn.addEventListener('click', () => {
        this.recreateAnalyzer();
      });
    }
  }

  /**
   * Recreate analyzer with new settings
   */
  private recreateAnalyzer(): void {
    // Stop existing analyzer
    if (this.spectrumAnalyzer) {
      this.spectrumAnalyzer.destroy();
      this.spectrumAnalyzer = null;
    }

    // Create new analyzer
    if (this.spectrumCanvas) {
      const config: SpectrumDisplayConfig = {
        canvas: this.spectrumCanvas,
        pollInterval: this.pollInterval,
        minFreq: this.minFreq,
        maxFreq: this.maxFreq,
        dbRange: this.dbRange,
        colorScheme: this.colorScheme,
        showLabels: true,
        showGrid: true,
      };

      this.spectrumAnalyzer = new SpectrumAnalyzerComponent(config);

      // Restart if was active
      const stopBtn = this.container?.querySelector('.stop-btn') as HTMLButtonElement;
      if (stopBtn && !stopBtn.disabled) {
        this.start();
      }
    }
  }

  /**
   * Start spectrum monitoring
   */
  async start(): Promise<void> {
    if (this.spectrumAnalyzer) {
      try {
        await this.spectrumAnalyzer.start();
        this.emit('started', {});
      } catch (error) {
        console.error('[SpectrumPlugin] Failed to start:', error);
        throw error;
      }
    }
  }

  /**
   * Stop spectrum monitoring
   */
  async stop(): Promise<void> {
    if (this.spectrumAnalyzer) {
      try {
        await this.spectrumAnalyzer.stop();
        this.emit('stopped', {});
      } catch (error) {
        console.error('[SpectrumPlugin] Failed to stop:', error);
      }
    }
  }

  /**
   * Check if analyzer is active
   */
  isActive(): boolean {
    return this.spectrumAnalyzer?.isActive() ?? false;
  }

  /**
   * Get current spectrum data
   */
  getSpectrum() {
    return this.spectrumAnalyzer?.getSpectrum() ?? null;
  }

  /**
   * Get parameters
   */
  getParameters() {
    return {
      minFreq: this.minFreq,
      maxFreq: this.maxFreq,
      dbRange: this.dbRange,
      pollInterval: this.pollInterval,
      colorScheme: this.colorScheme,
    };
  }

  /**
   * Set parameters
   */
  setParameters(params: Partial<{
    minFreq: number;
    maxFreq: number;
    dbRange: number;
    pollInterval: number;
    colorScheme: 'light' | 'dark';
  }>): void {
    if (params.minFreq !== undefined) this.minFreq = params.minFreq;
    if (params.maxFreq !== undefined) this.maxFreq = params.maxFreq;
    if (params.dbRange !== undefined) this.dbRange = params.dbRange;
    if (params.pollInterval !== undefined) this.pollInterval = params.pollInterval;
    if (params.colorScheme !== undefined) this.colorScheme = params.colorScheme;

    // Recreate analyzer with new settings
    if (this.spectrumAnalyzer) {
      this.recreateAnalyzer();
    }
  }

  /**
   * Resize handler
   */
  resize(): void {
    if (this.spectrumAnalyzer) {
      this.spectrumAnalyzer.resize();
    }
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    if (this.spectrumAnalyzer) {
      this.spectrumAnalyzer.destroy();
      this.spectrumAnalyzer = null;
    }

    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    super.destroy();
  }
}
