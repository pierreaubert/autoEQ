import { invoke } from "@tauri-apps/api/core";

/**
 * Spectrum information from the Rust backend
 */
export interface SpectrumInfo {
  /** Frequency bin centers in Hz */
  frequencies: number[];
  /** Magnitude values in dB (relative to full scale) */
  magnitudes: number[];
  /** Peak magnitude across all bins */
  peak_magnitude: number;
}

/**
 * Configuration for spectrum display
 */
export interface SpectrumDisplayConfig {
  /** Canvas element to render to */
  canvas: HTMLCanvasElement;
  /** Polling interval in milliseconds (default: 100ms) */
  pollInterval?: number;
  /** Minimum frequency to display (default: 20 Hz) */
  minFreq?: number;
  /** Maximum frequency to display (default: 20000 Hz) */
  maxFreq?: number;
  /** dB range for display (default: 60 dB) */
  dbRange?: number;
  /** Color scheme: 'light' or 'dark' (default: 'dark') */
  colorScheme?: "light" | "dark";
  /** Show frequency labels (default: true) */
  showLabels?: boolean;
  /** Show grid (default: true) */
  showGrid?: boolean;
}

/**
 * Real-time spectrum analyzer component
 * Displays frequency spectrum from Rust backend
 */
export class SpectrumAnalyzerComponent {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private config: Required<SpectrumDisplayConfig>;
  private pollInterval: number | null = null;
  private isMonitoring = false;
  private currentSpectrum: SpectrumInfo | null = null;
  private animationFrameId: number | null = null;

  constructor(config: SpectrumDisplayConfig) {
    this.canvas = config.canvas;
    this.ctx = this.canvas.getContext("2d")!;

    this.config = {
      canvas: config.canvas,
      pollInterval: config.pollInterval ?? 100,
      minFreq: config.minFreq ?? 20,
      maxFreq: config.maxFreq ?? 20000,
      dbRange: config.dbRange ?? 60,
      colorScheme: config.colorScheme ?? "dark",
      showLabels: config.showLabels ?? true,
      showGrid: config.showGrid ?? true,
    };

    this.setupCanvas();
  }

  /**
   * Setup canvas size and DPI scaling
   */
  private setupCanvas(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();

    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;

    // Reset context after changing canvas size
    this.ctx = this.canvas.getContext("2d")!;
    this.ctx.scale(dpr, dpr);
  }

  /**
   * Start monitoring spectrum
   */
  async start(): Promise<void> {
    if (this.isMonitoring) return;

    try {
      await invoke("stream_enable_spectrum_monitoring");
      this.isMonitoring = true;
      this.startPolling();
      this.startRendering();
    } catch (error) {
      console.error("Failed to start spectrum monitoring:", error);
      throw error;
    }
  }

  /**
   * Stop monitoring spectrum
   */
  async stop(): Promise<void> {
    if (!this.isMonitoring) return;

    this.isMonitoring = false;
    this.stopPolling();
    this.stopRendering();

    try {
      await invoke("stream_disable_spectrum_monitoring");
    } catch (error) {
      console.error("Failed to stop spectrum monitoring:", error);
    }
  }

  /**
   * Start polling for spectrum data
   */
  private startPolling(): void {
    let pollCount = 0;
    this.pollInterval = window.setInterval(async () => {
      try {
        const spectrum = await invoke<SpectrumInfo | null>(
          "stream_get_spectrum",
        );
        if (spectrum) {
          this.currentSpectrum = spectrum;
          if (pollCount++ % 10 === 0) {
            console.log("[Spectrum] Received data:", {
              frequencies: spectrum.frequencies.length,
              magnitudes: spectrum.magnitudes.length,
              peak: spectrum.peak_magnitude,
            });
          }
        }
      } catch (error) {
        console.error("Failed to get spectrum:", error);
      }
    }, this.config.pollInterval);
  }

  /**
   * Stop polling for spectrum data
   */
  private stopPolling(): void {
    if (this.pollInterval !== null) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }
  }

  /**
   * Start rendering loop
   */
  private startRendering(): void {
    const render = () => {
      this.render();
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
   * Render the spectrum to canvas
   */
  private render(): void {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;

    // Get background color from CSS variables
    const bgColor = this.getComputedCSSVariable("--bg-secondary");

    // Clear canvas with theme color
    this.ctx.fillStyle = bgColor;
    this.ctx.fillRect(0, 0, width, height);

    if (!this.currentSpectrum || this.currentSpectrum.magnitudes.length === 0) {
      this.drawNoData(width, height);
      return;
    }

    if (!this.isMonitoring) {
      return;
    }

    // Draw grid and labels
    if (this.config.showGrid) {
      this.drawGrid(width, height);
    }

    // Draw spectrum bars
    this.drawSpectrum(width, height);

    // Draw labels
    if (this.config.showLabels) {
      this.drawLabels(width, height);
    }
  }

  /**
   * Draw "no data" message
   */
  private drawNoData(width: number, height: number): void {
    this.ctx.fillStyle =
      this.config.colorScheme === "dark" ? "#888888" : "#666666";
    this.ctx.font = "14px sans-serif";
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "middle";
    this.ctx.fillText("Waiting for audio...", width / 2, height / 2);
  }

  /**
   * Draw frequency grid and dB scale
   */
  private drawGrid(width: number, height: number): void {
    const gridColor =
      this.config.colorScheme === "dark" ? "#333333" : "#cccccc";
    this.ctx.strokeStyle = gridColor;
    this.ctx.lineWidth = 0.5;

    // Horizontal grid lines (dB scale)
    const dbStep = this.config.dbRange / 6;
    for (let i = 0; i <= 6; i++) {
      const y = height - (i * height) / 6;
      this.ctx.beginPath();
      this.ctx.moveTo(40, y);
      this.ctx.lineTo(width - 10, y);
      this.ctx.stroke();
    }

    // Vertical grid lines (frequency scale)
    const freqSteps = [100, 1000, 10000];
    for (const freq of freqSteps) {
      if (freq >= this.config.minFreq && freq <= this.config.maxFreq) {
        const x = this.freqToX(freq, width);
        this.ctx.beginPath();
        this.ctx.moveTo(x, 10);
        this.ctx.lineTo(x, height - 30);
        this.ctx.stroke();
      }
    }
  }

  /**
   * Draw spectrum bars
   */
  private drawSpectrum(width: number, height: number): void {
    if (!this.currentSpectrum) return;

    const spectrum = this.currentSpectrum;
    const padding = 40;

    for (let i = 0; i < spectrum.frequencies.length; i++) {
      const freq = spectrum.frequencies[i];
      const magnitude = spectrum.magnitudes[i];

      // Skip if frequency is outside display range
      if (freq < this.config.minFreq || freq > this.config.maxFreq) {
        continue;
      }

      const x = this.freqToX(freq, width);

      // Calculate bar width based on logarithmic spacing
      let nextFreq = this.config.maxFreq;
      if (i < spectrum.frequencies.length - 1) {
        nextFreq = spectrum.frequencies[i + 1];
      }
      const nextX = this.freqToX(nextFreq, width);
      const barWidth = Math.max(1, nextX - x - 1);

      const barHeight = this.dbToHeight(magnitude, height);

      // Color based on magnitude
      const color = this.getMagnitudeColor(magnitude);
      this.ctx.fillStyle = color;
      this.ctx.fillRect(x, height - 50 - barHeight, barWidth, barHeight);
    }
  }

  /**
   * Draw frequency and dB labels
   */
  private drawLabels(width: number, height: number): void {
    const labelColor =
      this.config.colorScheme === "dark" ? "#ffffff" : "#000000";
    const bgColor =
      this.config.colorScheme === "dark"
        ? "rgba(26, 26, 26, 0.9)"
        : "rgba(248, 249, 250, 0.9)";

    this.ctx.font = "11px monospace";
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";

    // Frequency labels under each bar - at the very bottom
    const freqLabels = [
      { freq: 20, label: "20Hz" },
      { freq: 100, label: "100Hz" },
      { freq: 1000, label: "1kHz" },
      { freq: 10000, label: "10kHz" },
      { freq: 20000, label: "20kHz" },
    ];

    for (const { freq, label } of freqLabels) {
      if (freq >= this.config.minFreq && freq <= this.config.maxFreq) {
        const x = this.freqToX(freq, width);
        const y = height - 18;

        // Draw background
        this.ctx.fillStyle = bgColor;
        this.ctx.fillRect(x - 20, y, 40, 16);

        // Draw text
        this.ctx.fillStyle = labelColor;
        this.ctx.fillText(label, x, y + 2);
      }
    }

    // dB labels on vertical axis (left side)
    this.ctx.textAlign = "right";
    this.ctx.textBaseline = "middle";
    for (let i = 0; i <= 6; i++) {
      const db = -i * (this.config.dbRange / 6);
      const y = 10 + (i * (height - 60)) / 6;

      const label = `${db.toFixed(0)}dB`;

      // Draw background
      this.ctx.fillStyle = bgColor;
      this.ctx.fillRect(0, y - 7, 38, 14);

      // Draw text
      this.ctx.fillStyle = labelColor;
      this.ctx.fillText(label, 36, y);
    }
  }

  /**
   * Convert frequency to x coordinate
   */
  private freqToX(freq: number, width: number): number {
    const logMin = Math.log10(this.config.minFreq);
    const logMax = Math.log10(this.config.maxFreq);
    const logFreq = Math.log10(freq);

    const normalized = (logFreq - logMin) / (logMax - logMin);
    return 40 + normalized * (width - 50);
  }

  /**
   * Convert dB magnitude to height
   */
  private dbToHeight(magnitude: number, height: number): number {
    if (!isFinite(magnitude)) {
      return 0;
    }

    // Clamp to display range
    const clamped = Math.max(-this.config.dbRange, Math.min(0, magnitude));
    const normalized = (clamped + this.config.dbRange) / this.config.dbRange;

    return normalized * (height - 40);
  }

  /**
   * Get color based on magnitude
   */
  private getMagnitudeColor(magnitude: number): string {
    if (!isFinite(magnitude)) {
      return this.config.colorScheme === "dark" ? "#333333" : "#eeeeee";
    }

    // Color gradient: blue -> green -> yellow -> red
    if (magnitude < -40) {
      return this.config.colorScheme === "dark" ? "#1a3a7a" : "#8ab4f8";
    } else if (magnitude < -20) {
      return this.config.colorScheme === "dark" ? "#1a7a3a" : "#81c995";
    } else if (magnitude < -10) {
      return this.config.colorScheme === "dark" ? "#7a7a1a" : "#fdd835";
    } else if (magnitude < 0) {
      return this.config.colorScheme === "dark" ? "#7a3a1a" : "#ff6f00";
    } else {
      return this.config.colorScheme === "dark" ? "#7a1a1a" : "#d32f2f";
    }
  }

  /**
   * Get current spectrum data
   */
  getSpectrum(): SpectrumInfo | null {
    return this.currentSpectrum;
  }

  /**
   * Check if monitoring is active
   */
  isActive(): boolean {
    return this.isMonitoring;
  }

  /**
   * Resize canvas
   */
  resize(): void {
    this.setupCanvas();
  }

  /**
   * Cleanup
   */
  destroy(): void {
    this.stop();
  }

  /**
   * Get computed CSS variable value
   */
  private getComputedCSSVariable(varName: string): string {
    const value = getComputedStyle(document.documentElement)
      .getPropertyValue(varName)
      .trim();
    return value || (varName === "--bg-secondary" ? "#2d2d2d" : "#ffffff");
  }
}
