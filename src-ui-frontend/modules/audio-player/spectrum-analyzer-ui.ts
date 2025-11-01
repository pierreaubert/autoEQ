import {
  SpectrumAnalyzerComponent,
  type SpectrumDisplayConfig,
} from "./spectrum-analyzer";

/**
 * UI wrapper for spectrum analyzer component
 * Provides a complete UI with controls and statistics
 */
export class SpectrumAnalyzerUI {
  private container: HTMLElement;
  private analyzer: SpectrumAnalyzerComponent;
  private startButton: HTMLButtonElement;
  private stopButton: HTMLButtonElement;
  private peakLabel: HTMLElement;
  private statusLabel: HTMLElement;

  constructor(
    containerElement: HTMLElement,
    config?: Partial<SpectrumDisplayConfig>,
  ) {
    this.container = containerElement;

    // Create HTML structure
    this.container.innerHTML = `
      <div class="spectrum-analyzer-container">
        <div class="spectrum-analyzer-header">
          <span class="spectrum-analyzer-title">Real-time Spectrum Analyzer</span>
          <div class="spectrum-analyzer-controls">
            <button class="spectrum-analyzer-button" id="spectrum-start">Start</button>
            <button class="spectrum-analyzer-button" id="spectrum-stop" disabled>Stop</button>
          </div>
        </div>
        <div class="spectrum-analyzer-canvas-wrapper">
          <canvas class="spectrum-analyzer-canvas" id="spectrum-canvas"></canvas>
        </div>
        <div class="spectrum-analyzer-stats">
          <div class="spectrum-analyzer-stat">
            <span class="spectrum-analyzer-stat-label">Peak</span>
            <span class="spectrum-analyzer-stat-value" id="spectrum-peak">-∞ dB</span>
          </div>
          <div class="spectrum-analyzer-stat">
            <span class="spectrum-analyzer-stat-label">Status</span>
            <span class="spectrum-analyzer-stat-value" id="spectrum-status">Idle</span>
          </div>
        </div>
      </div>
    `;

    // Get elements
    const canvas = this.container.querySelector(
      "#spectrum-canvas",
    ) as HTMLCanvasElement;
    this.startButton = this.container.querySelector(
      "#spectrum-start",
    ) as HTMLButtonElement;
    this.stopButton = this.container.querySelector(
      "#spectrum-stop",
    ) as HTMLButtonElement;
    this.peakLabel = this.container.querySelector(
      "#spectrum-peak",
    ) as HTMLElement;
    this.statusLabel = this.container.querySelector(
      "#spectrum-status",
    ) as HTMLElement;

    // Create analyzer
    this.analyzer = new SpectrumAnalyzerComponent({
      canvas,
      ...config,
    });

    // Setup event listeners
    this.setupEventListeners();

    // Start updating stats
    this.startStatsUpdate();
  }

  /**
   * Setup button event listeners
   */
  private setupEventListeners(): void {
    this.startButton.addEventListener("click", () => this.start());
    this.stopButton.addEventListener("click", () => this.stop());
  }

  /**
   * Start spectrum monitoring
   */
  async start(): Promise<void> {
    try {
      await this.analyzer.start();
      this.startButton.disabled = true;
      this.stopButton.disabled = false;
      this.startButton.classList.add("active");
      this.statusLabel.textContent = "Monitoring";
    } catch (error) {
      console.error("Failed to start spectrum analyzer:", error);
      this.statusLabel.textContent = "Error";
    }
  }

  /**
   * Stop spectrum monitoring
   */
  async stop(): Promise<void> {
    try {
      await this.analyzer.stop();
      this.startButton.disabled = false;
      this.stopButton.disabled = true;
      this.startButton.classList.remove("active");
      this.statusLabel.textContent = "Idle";
      this.peakLabel.textContent = "-∞ dB";
    } catch (error) {
      console.error("Failed to stop spectrum analyzer:", error);
    }
  }

  /**
   * Start updating statistics display
   */
  private startStatsUpdate(): void {
    setInterval(() => {
      const spectrum = this.analyzer.getSpectrum();
      if (spectrum && isFinite(spectrum.peak_magnitude)) {
        this.peakLabel.textContent = `${spectrum.peak_magnitude.toFixed(1)} dB`;
      }
    }, 100);
  }

  /**
   * Get the underlying analyzer component
   */
  getAnalyzer(): SpectrumAnalyzerComponent {
    return this.analyzer;
  }

  /**
   * Cleanup
   */
  destroy(): void {
    this.analyzer.destroy();
  }
}

/**
 * Create a spectrum analyzer UI and attach to DOM
 */
export function createSpectrumAnalyzerUI(
  selector: string,
  config?: Partial<SpectrumDisplayConfig>,
): SpectrumAnalyzerUI | null {
  const element = document.querySelector(selector) as HTMLElement;
  if (!element) {
    console.error(`Element not found: ${selector}`);
    return null;
  }

  return new SpectrumAnalyzerUI(element, config);
}
