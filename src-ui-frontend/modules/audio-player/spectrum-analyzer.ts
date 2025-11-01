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
    if (!this.ctx) return;

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

    // Draw horizontal dB grid lines ON TOP of spectrum bars
    this.drawVerticalGridLines(width, height);

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
    const isDarkMode = this.config.colorScheme === "dark";

    // Draw horizontal frequency lines (original grid functionality)
    this.ctx.strokeStyle = isDarkMode ? "rgba(255, 255, 255, 0.1)" : "rgba(0, 0, 0, 0.1)";
    this.ctx.lineWidth = 1;
    const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqMarkers.forEach(freq => {
      const x = this.freqToX(freq, width);
      this.ctx.beginPath();
      this.ctx.moveTo(x, 0);
      this.ctx.lineTo(x, height - 10);
      this.ctx.stroke();
    });
  }

  /**
   * Draw vertical dB grid lines
   */
  private drawVerticalGridLines(width: number, height: number): void {
    const isDarkMode = this.config.colorScheme === "dark";
    
    // Draw horizontal lines for dB levels (0, -10, -20, -30, -40, -50, -60)
    const dbLevels = [0, -10, -20, -30, -40, -50, -60];
    
    dbLevels.forEach(db => {
      const y = this.dbToY(db, height);
      
      // Set dotted line style - full opacity
      const lineColor = isDarkMode ? "rgba(255, 255, 255, 1)" : "rgba(0, 0, 0, 1)";
      this.ctx.strokeStyle = lineColor;
      this.ctx.lineWidth = 1;
      this.ctx.setLineDash([2, 3]); // Dotted pattern: 2px dash, 3px gap
      
      // Draw horizontal line across full width
      this.ctx.beginPath();
      this.ctx.moveTo(0, y);
      this.ctx.lineTo(width, y);
      this.ctx.stroke();
    });
    
    // Reset line dash to solid
    this.ctx.setLineDash([]);
  }

  /**
   * Convert dB to Y coordinate (inverted because canvas Y increases downward)
   */
  private dbToY(db: number, height: number): number {
    const normalized = (db + this.config.dbRange) / this.config.dbRange;
    return height - 10 - (normalized * (height - 10));
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
      // Draw bars from bottom, leaving 10px for labels
      this.ctx.fillRect(x, height - 10 - barHeight, barWidth, barHeight);
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

    this.ctx.font = "9px monospace";
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";

    // Frequency labels under each bar - reduced to every other label
    const freqLabels = [
      { freq: 20, label: "20" },
      { freq: 40, label: "40" },
      { freq: 60, label: "60" },
      { freq: 100, label: "100" },
      { freq: 200, label: "200" },
      { freq: 400, label: "400" },
      { freq: 600, label: "600" },
      { freq: 1000, label: "1k" },
      { freq: 2000, label: "2k" },
      { freq: 4000, label: "4k" },
      { freq: 6000, label: "6k" },
      { freq: 10000, label: "10k" },
      { freq: 20000, label: "20k" },
    ];

    for (const { freq, label } of freqLabels) {
      if (freq >= this.config.minFreq && freq <= this.config.maxFreq) {
        const x = this.freqToX(freq, width);
        const y = height - 8;

        // Draw background for better visibility
        this.ctx.fillStyle = bgColor;
        this.ctx.fillRect(x - 20, y - 6, 40, 10);

        // Draw text
        this.ctx.fillStyle = labelColor;
        this.ctx.fillText(label, x, y - 4);
      }
    }

    // dB labels on vertical axis (left side) - adjust for compact height
    this.ctx.textAlign = "right";
    this.ctx.textBaseline = "middle";
    // Show fewer labels for compact height
    for (let i = 0; i <= 3; i++) {
      const db = -i * (this.config.dbRange / 3);
      // Adjust Y positioning for 72px height
      const y = 8 + (i * (height - 18)) / 3;

      const label = `${db.toFixed(0)}dB`;

      // Draw background for better visibility
      this.ctx.fillStyle = bgColor;
      this.ctx.fillRect(0, y - 6, 30, 12);

      // Draw text
      this.ctx.fillStyle = labelColor;
      this.ctx.fillText(label, 28, y);
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
    // Use smaller left padding for compact canvas
    return 30 + normalized * (width - 35);
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

    // Use full height minus 10px for labels
    return normalized * (height - 10);
  }

  /**
   * Get color based on magnitude
   */
  private getMagnitudeColor(magnitude: number): string {
    if (!isFinite(magnitude)) {
      return this.config.colorScheme === "dark" ? "#333333" : "#eeeeee";
    }

    // Color gradient: blue -> green -> yellow -> red
    // Dark mode colors are now lighter for better visibility
    if (magnitude < -40) {
      return this.config.colorScheme === "dark" ? "#4fc3f7" : "#8ab4f8";
    } else if (magnitude < -20) {
      return this.config.colorScheme === "dark" ? "#66bb6a" : "#81c995";
    } else if (magnitude < -10) {
      return this.config.colorScheme === "dark" ? "#ffeb3b" : "#fdd835";
    } else if (magnitude < 0) {
      return this.config.colorScheme === "dark" ? "#ff9800" : "#ff6f00";
    } else {
      return this.config.colorScheme === "dark" ? "#ef5350" : "#d32f2f";
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
