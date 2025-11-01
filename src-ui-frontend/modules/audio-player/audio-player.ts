// Standalone Audio Player Module
// Extracted from audio-processor.ts and related UI components

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { resolveResource } from "@tauri-apps/api/path";
import {
  StreamingManager,
  type AudioFileInfo,
} from "../audio-manager-streaming";
import { SpectrumAnalyzerComponent } from "./spectrum-analyzer";

interface ReplayGainInfo {
  gain: number; // dB
  peak: number; // 0.0 to 1.0+
}

export interface AudioPlayerConfig {
  // Demo audio tracks configuration
  demoTracks?: { [key: string]: string };

  // EQ configuration
  enableEQ?: boolean;
  maxFilters?: number;

  // Spectrum analyzer configuration
  enableSpectrum?: boolean;
  fftSize?: number;
  smoothingTimeConstant?: number;

  // UI configuration
  showProgress?: boolean;
  showFrequencyLabels?: boolean;
  compactMode?: boolean;
}

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

export interface AudioPlayerCallbacks {
  onPlay?: () => void;
  onStop?: () => void;
  onEQToggle?: (enabled: boolean) => void;
  onTrackChange?: (trackName: string) => void;
  onError?: (error: string) => void;
}

export class AudioPlayer {
  private audioContext: AudioContext | null = null;
  private audioBuffer: AudioBuffer | null = null;
  private audioSource: AudioBufferSourceNode | null = null;
  private eqFilters: BiquadFilterNode[] = [];
  private gainNode: GainNode | null = null;
  private isAudioPlaying: boolean = false;
  private isAudioPaused: boolean = false;
  private audioStartTime: number = 0;
  private audioPauseTime: number = 0;
  private audioAnimationFrame: number | null = null;
  private currentAudioPath: string | null = null; // Track current audio file path for Rust backend
  private currentFilterParams: ExtendedFilterParam[] = [
    { frequency: 100, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
    { frequency: 1000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
    { frequency: 10000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
  ];
  private eqEnabled: boolean = true;
  private outputDeviceId: string = "default"; // Selected output device ID
  private audioElement: HTMLAudioElement | null = null; // For device routing

  // Playback configuration
  private loudnessCompensation: boolean = false;
  private splAmplitude: number = -20; // dB range: -30 to 0
  private autoGain: boolean = true; // Auto-gain enabled by default

  // Frequency analyzer
  private analyserNode: AnalyserNode | null = null;
  private spectrumCanvas: HTMLCanvasElement | null = null;
  private spectrumCtx: CanvasRenderingContext2D | null = null;
  private spectrumAnimationFrame: number | null = null;

  // Loudness monitoring
  private loudnessDisplayMomentary: HTMLElement | null = null;
  private loudnessDisplayShortterm: HTMLElement | null = null;

  // 30-bin spectrum analyzer constants
  private readonly SPECTRUM_BINS = 30;
  private readonly SPECTRUM_MIN_FREQ = 20;
  private readonly SPECTRUM_MAX_FREQ = 20000;
  private spectrumBinEdges: number[] = [];
  private spectrumBinCenters: number[] = [];
  private spectrumBinValues: number[] = []; // Smoothed values for display

  // UI Elements
  private container: HTMLElement;
  private demoSelect: HTMLSelectElement | null = null;
  private listenBtn: HTMLButtonElement | null = null;
  private pauseBtn: HTMLButtonElement | null = null;
  private stopBtn: HTMLButtonElement | null = null;
  private eqOnBtn: HTMLButtonElement | null = null;
  private eqOffBtn: HTMLButtonElement | null = null;
  private eqConfigBtn: HTMLButtonElement | null = null;
  private eqModal: HTMLElement | null = null;
  private eqBackdrop: HTMLElement | null = null;
  private eqModalCloseBtn: HTMLButtonElement | null = null;
  private playbackOptionsContainer: HTMLElement | null = null;
  private eqTableContainer: HTMLElement | null = null;
  private statusText: HTMLElement | null = null;
  private positionText: HTMLElement | null = null;
  private durationText: HTMLElement | null = null;
  private progressFill: HTMLElement | null = null;
  private eqFilterCountText: HTMLElement | null = null;
  private eqGainCompText: HTMLElement | null = null;
  private eqMiniCanvas: HTMLCanvasElement | null = null;
  private eqMiniCtx: CanvasRenderingContext2D | null = null;

  // EQ Graph properties
  private eqGraphCanvas: HTMLCanvasElement | null = null;
  private eqGraphCtx: CanvasRenderingContext2D | null = null;
  private selectedFilterIndex: number = -1;
  private isDraggingHandle: boolean = false;
  private dragMode: "ring" | "bar" | null = null;
  private dragStartX: number = 0;
  private dragStartY: number = 0;
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

  // ReplayGain
  private replayGainInfo: ReplayGainInfo | null = null;
  private replayGainContainer: HTMLElement | null = null;

  // Configuration
  private config: AudioPlayerConfig;
  private callbacks: AudioPlayerCallbacks;
  private instanceId: string;

  // Pause double-click tracking
  private pauseClickCount: number = 0;
  private pauseClickTimer: number | null = null;

  // Resize handler reference for cleanup
  private resizeHandler: (() => void) | null = null;

  // Streaming audio manager instance
  private streamingManager: StreamingManager;

  // Spectrum analyzer instance
  private spectrumAnalyzer: SpectrumAnalyzerComponent | null = null;

  // Loudness polling state
  private loudnessPollingActive: boolean = false;

  constructor(
    container: HTMLElement,
    config: AudioPlayerConfig = {},
    callbacks: AudioPlayerCallbacks = {},
  ) {
    if (!container) {
      throw new Error(
        "AudioPlayer: container element is required but was null/undefined",
      );
    }
    this.container = container;
    this.instanceId = "audio-player-" + Math.random().toString(36).substr(2, 9);
    this.config = {
      enableEQ: true,
      maxFilters: 10,
      enableSpectrum: true,
      fftSize: 4096,
      smoothingTimeConstant: 0.8,
      showProgress: true,
      showFrequencyLabels: true,
      compactMode: false,
      demoTracks: {
        classical: "public/demo-audio/classical.flac",
        country: "public/demo-audio/country.flac",
        edm: "public/demo-audio/edm.flac",
        female_vocal: "public/demo-audio/female_vocal.flac",
        jazz: "public/demo-audio/jazz.flac",
        piano: "public/demo-audio/piano.flac",
        rock: "public/demo-audio/rock.flac",
      },
      ...config,
    };
    this.callbacks = callbacks;

    // Initialize streaming manager
    this.streamingManager = new StreamingManager({
      onStateChange: (state) => this.handleStateChange(state),
      onPositionUpdate: (position, duration) =>
        this.handlePositionUpdate(position, duration),
      onError: (error) => this.handleError(error),
      onFileLoaded: (info) => this.handleFileLoaded(info),
    });

    this.init();
  }

  private _createEQModal(): void {
    console.log("[EQ Debug] Creating modal element");
    const existingModal = document.getElementById(
      this.instanceId + "-eq-modal",
    );
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
          <h3>Playback Configuration</h3>
          <button type="button" class="eq-modal-close-btn">&times;</button>
        </div>
        <div class="eq-modal-body">
          <div class="playback-options-container"></div>
          <div class="eq-table-container"></div>
        </div>
      </div>
    `;

    // Append both to body for proper layering
    document.body.appendChild(backdrop);
    document.body.appendChild(modal);
    console.log("[EQ Debug] Modal and backdrop inserted into body");
    console.log("[EQ Debug] Modal in DOM:", document.contains(modal));
    const foundModal = document.getElementById(this.instanceId + "-eq-modal");
    console.log("[EQ Debug] Can find modal after insertion:", !!foundModal);
  }

  private async init(): Promise<void> {
    try {
      await this.setupAudioContext();
      this._createEQModal();
      this.createUI();
      this.setupEventListeners();
      console.log("AudioPlayer initialized successfully");
    } catch (error) {
      console.error("Failed to initialize AudioPlayer:", error);
      this.callbacks.onError?.("Failed to initialize audio player: " + error);
    }
  }

  private handleStateChange(state: string): void {
    console.log("[AudioPlayer] Backend state changed:", state);

    if (state === "playing") {
      this.isAudioPlaying = true;
      this.isAudioPaused = false;
    } else if (state === "paused") {
      this.isAudioPlaying = false;
      this.isAudioPaused = true;
    } else if (state === "idle" || state === "ready") {
      this.isAudioPlaying = false;
      this.isAudioPaused = false;
      // Reset position display
      if (this.positionText) {
        this.positionText.textContent = "--:--";
      }
      if (this.progressFill) {
        this.progressFill.style.width = "0%";
      }
      this.setStatus("Ready");
    } else if (state === "error") {
      this.isAudioPlaying = false;
      this.isAudioPaused = false;
      this.setStatus("Error");
    }

    this.updatePlaybackUI();
  }

  private handlePositionUpdate(position: number, duration?: number): void {
    if (this.positionText) {
      this.positionText.textContent = this.formatTime(position);
    }
    if (this.durationText && duration) {
      this.durationText.textContent = this.formatTime(duration);
    }
    if (this.progressFill && duration) {
      const progress = (position / duration) * 100;
      this.progressFill.style.width = `${Math.min(progress, 100)}%`;
    }
  }

  private handleError(error: string): void {
    console.error("[AudioPlayer] Backend error:", error);
    this.callbacks.onError?.(error);
    this.setStatus("Error: " + error);
  }

  private handleFileLoaded(info: AudioFileInfo): void {
    console.log("[AudioPlayer] File loaded:", info);
    // Update UI with file info
    if (this.durationText && info.duration_seconds) {
      this.durationText.textContent = this.formatTime(info.duration_seconds);
    }
    this.setStatus("Ready");
    // Enable playback controls
    if (this.listenBtn) {
      this.listenBtn.disabled = false;
    }
  }

  private async setupAudioContext(): Promise<void> {
    try {
      this.audioContext = new (window.AudioContext ||
        (window as typeof window & { webkitAudioContext?: typeof AudioContext })
          .webkitAudioContext ||
        AudioContext)();
      this.gainNode = this.audioContext.createGain();

      if (this.config.enableSpectrum) {
        this.analyserNode = this.audioContext.createAnalyser();
        this.analyserNode.fftSize = this.config.fftSize || 2048;
        this.analyserNode.smoothingTimeConstant =
          this.config.smoothingTimeConstant || 0.8;
      }
    } catch (error) {
      console.error("Failed to initialize audio context:", error);
      throw error;
    }
  }

  private createUI(): void {
    const selectId = `demo-audio-select-${this.instanceId}`;
    console.log("[EQ Debug] Creating UI with config:", {
      enableEQ: this.config.enableEQ,
      enableSpectrum: this.config.enableSpectrum,
      showProgress: this.config.showProgress,
    });

    const html = `
      <div class="audio-player">
        <div class="audio-control-row">
          <!-- Block 1: Track Selection (compact) -->
          <div class="audio-left-controls">
            <div class="demo-track-container">
              <div class="demo-track-select-row">
                <select id="${selectId}" class="demo-audio-select">
                <option value="">Pick a track...</option>
                  ${Object.keys(this.config.demoTracks || {})
                    .map(
                      (key) =>
                        `<option value="${key}">${this.formatTrackName(key)}</option>`,
                    )
                    .join("")}
                </select>
                <button type="button" class="file-upload-btn" title="Load Audio file">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path>
                    <polyline points="13 2 13 9 20 9"></polyline>
                  </svg>
                </button>
              </div>
            </div>
          </div>

          <!-- Block 2: Playback Controls (compact) -->
          <div class="audio-center-controls">
            <div class="audio-playback-container">
              <div class="audio-playback-controls">
                <button type="button" class="listen-button" disabled>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M8 5V19L19 12L8 5Z"/></svg>
                </button>
                <button type="button" class="pause-button" disabled>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M6 4H10V20H6V4ZM14 4H18V20H14V4Z"/></svg>
                </button>
                <button type="button" class="stop-button" disabled>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M6 6H18V18H6V6Z"/></svg>
                </button>
              </div>
              ${
                this.config.showProgress
                  ? `
                <div class="audio-status" style="display: flex; flex-direction: column; gap: 4px;">
                  <div class="audio-info-compact">
                    <span class="audio-status-text">Ready</span> •
                    <span class="audio-position">--:--</span> /
                    <span class="audio-duration">--:--</span>
                  </div>
                  <div class="audio-progress">
                    <div class="audio-progress-bar">
                      <div class="audio-progress-fill" style="width: 0%;"></div>
                    </div>
                  </div>
                </div>
              `
                  : ""
              }
            </div>
          </div>

          <!-- Block 3: Spectrum Analyzer (full height) -->
          <div class="audio-right-controls audio-spectrum-block">
            ${
              this.config.enableSpectrum
                ? `
              <div class="frequency-analyzer">
                <div class="spectrum-container">
                  <canvas class="spectrum-canvas"></canvas>
                </div>
              </div>
            `
                : ""
            }
          </div>

          <!-- Block 4: EQ Controls -->
          <div class="audio-eq-controls">
            ${
              this.config.enableEQ
                ? `
              <div class="eq-control-section">
                <div class="eq-mini-graph">
                  <canvas class="eq-mini-canvas" width="160" height="50"></canvas>
                </div>
                <div class="eq-controls-row">
                  <div class="eq-info-text">
                    <span class="eq-filter-count">#0</span>
                    <span class="eq-gain-compensation">0dB</span>
                  </div>
                  <div class="eq-toggle-buttons" tabindex="0">
                    <button type="button" class="eq-toggle-btn eq-on-btn active">On</button>
                    <button type="button" class="eq-toggle-btn eq-off-btn">Off</button>
                    <button type="button" class="eq-toggle-btn eq-config-btn">⚙️</button>
                  </div>
                </div>
              </div>
            `
                : ""
            }
          </div>

          <!-- Block 5: Audio Metrics (vertical) -->
          <div class="audio-metrics-block">
            <div class="metrics-display">
              <div class="metric-section">
                <div class="metric-label">LUFS M/S</div>
                <div class="metric-row">
                  <span class="metric-label-small">M</span>
                  <span id="metrics-lufs-m" class="metric-value loudness-momentary">-∞</span>
                  <span class="metric-separator">/</span>
                  <span class="metric-label-small">S</span>
                  <span id="metrics-lufs-s" class="metric-value loudness-shortterm">-∞</span>
                </div>
              </div>
              <div class="metric-section">
                <div class="metric-label">Replay Gain / Peak</div>
                <div class="metric-row">
                  <span id="metrics-replay-gain" class="metric-value replay-gain-value">0.00</span>
                  <span class="metric-unit">dB</span>
                  <span class="metric-separator">/</span>
                  <span id="metrics-peak" class="metric-value replay-peak-value">0.000</span>
                  <span class="metric-unit">dB</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;

    console.log(
      "[EQ Debug] Generated HTML contains gear button:",
      html.includes("eq-config-btn"),
    );
    this.container.innerHTML = html;
    console.log("[EQ Debug] HTML injected into container");
    this.cacheUIElements();
  }

  private cacheUIElements(): void {
    console.log(
      "[EQ Debug] Caching UI elements from container:",
      this.container,
    );
    console.log(
      "[EQ Debug] Container HTML:",
      this.container.innerHTML.substring(0, 500) + "...",
    );

    this.demoSelect = this.container.querySelector(".demo-audio-select");
    this.listenBtn = this.container.querySelector(".listen-button");
    this.pauseBtn = this.container.querySelector(".pause-button");
    this.stopBtn = this.container.querySelector(".stop-button");
    this.eqOnBtn = this.container.querySelector(".eq-on-btn");
    this.eqOffBtn = this.container.querySelector(".eq-off-btn");
    this.eqConfigBtn = this.container.querySelector(".eq-config-btn");

    console.log("[EQ Debug] Elements found:", {
      demoSelect: !!this.demoSelect,
      listenBtn: !!this.listenBtn,
      eqOnBtn: !!this.eqOnBtn,
      eqOffBtn: !!this.eqOffBtn,
      eqConfigBtn: !!this.eqConfigBtn,
    });

    console.log("[EQ Debug] Gear button element:", this.eqConfigBtn);
    console.log("[EQ Debug] Gear button found:", !!this.eqConfigBtn);

    // Check if EQ buttons container exists
    const eqButtonsContainer =
      this.container.querySelector(".eq-toggle-buttons");
    console.log("[EQ Debug] EQ buttons container found:", !!eqButtonsContainer);
    if (eqButtonsContainer) {
      console.log(
        "[EQ Debug] EQ buttons container HTML:",
        eqButtonsContainer.innerHTML,
      );
    }
    this.statusText = this.container.querySelector(".audio-status-text");
    this.positionText = this.container.querySelector(".audio-position");
    this.durationText = this.container.querySelector(".audio-duration");
    this.progressFill = this.container.querySelector(".audio-progress-fill");
    this.spectrumCanvas = this.container.querySelector(".spectrum-canvas");
    this.loudnessDisplayMomentary = this.container.querySelector(
      ".loudness-momentary",
    );
    this.loudnessDisplayShortterm = this.container.querySelector(
      ".loudness-shortterm",
    );

    // Modal and backdrop elements are in the body
    this.eqModal = document.getElementById(this.instanceId + "-eq-modal");
    this.eqBackdrop = document.getElementById(this.instanceId + "-eq-backdrop");
    console.log(
      "[EQ Debug] Modal element lookup ID:",
      this.instanceId + "-eq-modal",
    );
    console.log("[EQ Debug] Modal element found:", this.eqModal);
    console.log("[EQ Debug] Backdrop element found:", this.eqBackdrop);
    if (this.eqModal) {
      this.eqModalCloseBtn = this.eqModal.querySelector(".eq-modal-close-btn");
      this.playbackOptionsContainer = this.eqModal.querySelector(
        ".playback-options-container",
      );
      this.eqTableContainer = this.eqModal.querySelector(".eq-table-container");

      // Cache EQ graph canvas
      this.eqGraphCanvas = this.eqModal.querySelector(".eq-graph-canvas");
      if (this.eqGraphCanvas) {
        this.eqGraphCtx = this.eqGraphCanvas.getContext("2d");
        this.resizeEQGraphCanvas();
      }
    }

    if (this.spectrumCanvas) {
      this.spectrumCtx = this.spectrumCanvas.getContext("2d");
      // Initialize spectrum analyzer component
      if (this.config.enableSpectrum) {
        this.spectrumAnalyzer = new SpectrumAnalyzerComponent({
          canvas: this.spectrumCanvas,
          pollInterval: 100,
          minFreq: 20,
          maxFreq: 20000,
          dbRange: 60,
          colorScheme: "dark",
          showLabels: true,
          showGrid: false,
        });
      }
    }

    // Cache EQ info elements
    this.eqFilterCountText = this.container.querySelector(".eq-filter-count");
    this.eqGainCompText = this.container.querySelector(".eq-gain-compensation");
    this.eqMiniCanvas = this.container.querySelector(".eq-mini-canvas");
    if (this.eqMiniCanvas) {
      this.eqMiniCtx = this.eqMiniCanvas.getContext("2d");
      this.drawEQMiniGraph();
    }

    // Cache ReplayGain elements
    this.replayGainContainer =
      this.container.querySelector(".replay-gain-info");
    console.log("[ReplayGain] Container cached:", !!this.replayGainContainer);
    if (this.replayGainContainer) {
      console.log("[ReplayGain] Container element:", this.replayGainContainer);
      console.log(
        "[ReplayGain] Container initial display:",
        this.replayGainContainer.style.display,
      );
    }
  }

  private setupEventListeners(): void {
    // Handle window resize
    this.resizeHandler = () => {
      // Spectrum analyzer handles its own resizing
    };
    window.addEventListener("resize", this.resizeHandler);

    // Demo track selection
    this.demoSelect?.addEventListener("change", async (e) => {
      const trackName = (e.target as HTMLSelectElement).value;
      if (trackName) {
        await this.loadDemoTrack(trackName);
        this.callbacks.onTrackChange?.(trackName);
      } else {
        this.clearAudio();
      }
    });

    // File upload button
    const uploadBtn = this.container.querySelector(".file-upload-btn");
    uploadBtn?.addEventListener("click", async () => {
      try {
        const selectedPath = await open({
          multiple: false,
          filters: [
            {
              name: "Audio",
              extensions: ["wav", "flac", "mp3", "ogg", "m4a", "aac", "opus"],
            },
          ],
        });

        if (typeof selectedPath === "string") {
          await this.loadAudioFilePath(selectedPath);
        }
      } catch (error) {
        console.error("File selection failed:", error);
        this.callbacks.onError?.("File selection failed: " + error);
      }
    });

    // Playback controls
    this.listenBtn?.addEventListener("click", () => {
      // If truly paused, resume; otherwise, play from beginning
      if (this.isAudioPaused) {
        this.resume();
      } else {
        this.play();
      }
    });

    this.pauseBtn?.addEventListener("click", () => {
      this.handlePauseClick();
    });

    this.stopBtn?.addEventListener("click", () => {
      this.stop();
    });

    // EQ controls
    this.eqOnBtn?.addEventListener("click", () => this.setEQEnabled(true));
    this.eqOffBtn?.addEventListener("click", () => this.setEQEnabled(false));

    // Initialize EQ info display
    if (this.config.enableEQ) {
      this.updateEQInfo(0, 0);
      this.drawEQMiniGraph();
    }
    if (this.eqConfigBtn) {
      console.log("[EQ Debug] Adding click event listener to gear button");
      this.eqConfigBtn.addEventListener("click", () => {
        console.log("[EQ Debug] Gear button clicked - event triggered");
        try {
          console.log("[EQ Debug] Executing modal show logic");
          this.openEQModal();
        } catch (error) {
          console.error("[EQ Debug] Error in click handler:", error);
        }
      });
      console.log("[EQ Debug] Click event listener attached to gear button");
    } else {
      console.error(
        "[EQ Debug] Gear button not found, cannot attach event listener",
      );
    }
    this.eqModalCloseBtn?.addEventListener("click", () => this.closeEQModal());
    this.eqBackdrop?.addEventListener("click", () => this.closeEQModal());

    // Keyboard controls for EQ buttons
    const eqButtonsContainer = this.container.querySelector(
      ".eq-toggle-buttons",
    ) as HTMLElement;
    if (eqButtonsContainer) {
      eqButtonsContainer.addEventListener("keydown", (e: KeyboardEvent) => {
        switch (e.key) {
          case "ArrowLeft":
            e.preventDefault();
            this.setEQEnabled(true);
            console.log("EQ enabled via left arrow key");
            break;
          case "ArrowRight":
            e.preventDefault();
            this.setEQEnabled(false);
            console.log("EQ disabled via right arrow key");
            break;
          case " ": // Space bar
            e.preventDefault();
            this.setEQEnabled(!this.eqEnabled);
            console.log("EQ toggled via space key:", this.eqEnabled);
            break;
        }
      });
    }

    // EQ Graph interactions
    if (this.eqGraphCanvas) {
      this.eqGraphCanvas.addEventListener("mousedown", (e) =>
        this.handleGraphMouseDown(e),
      );
      this.eqGraphCanvas.addEventListener("mousemove", (e) =>
        this.handleGraphMouseMove(e),
      );
      this.eqGraphCanvas.addEventListener("mouseup", (e) =>
        this.handleGraphMouseUp(e),
      );
      this.eqGraphCanvas.addEventListener("mouseleave", (e) =>
        this.handleGraphMouseUp(e),
      );
      this.eqGraphCanvas.style.cursor = "crosshair";
    }
  }

  private openEQModal(): void {
    console.log("[EQ Debug] Attempting to show modal");
    console.log("[EQ Debug] Current modal state:", {
      exists: !!this.eqModal,
      backdropExists: !!this.eqBackdrop,
      id: this.eqModal?.id,
      className: this.eqModal?.className,
      parentElement: this.eqModal?.parentElement?.tagName,
    });

    if (this.eqModal && this.eqBackdrop && this.eqConfigBtn) {
      this.renderEQTable();

      // Center the modal on screen
      const padding = 20;
      const maxWidth = Math.min(window.innerWidth - padding * 2, 1200);
      const maxHeight = Math.min(
        window.innerHeight - padding * 2,
        window.innerHeight * 0.85,
      );

      const left = (window.innerWidth - maxWidth) / 2;
      const top = (window.innerHeight - maxHeight) / 2;

      // Apply positioning and sizing
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

      // Show backdrop and modal
      this.eqBackdrop.classList.add("visible");
      this.eqModal.classList.add("visible");

      console.log("[EQ Debug] Modal classes after show:", {
        modal: this.eqModal.className,
        backdrop: this.eqBackdrop.className,
      });

      // Compute and draw EQ graph
      this.computeEQResponse();

      // Add click outside handler
      document.addEventListener("mousedown", this.handleClickOutside, true);
    } else {
      console.error(
        "[EQ Debug] Modal, backdrop, or gear button element is null or undefined",
      );
    }
  }

  private closeEQModal(): void {
    if (this.eqModal) {
      this.eqModal.classList.remove("visible");
    }
    if (this.eqBackdrop) {
      this.eqBackdrop.classList.remove("visible");
    }
    document.removeEventListener("mousedown", this.handleClickOutside, true);
  }

  private handleClickOutside = (event: MouseEvent): void => {
    if (
      this.eqModal &&
      !this.eqModal.contains(event.target as Node) &&
      !this.eqConfigBtn?.contains(event.target as Node)
    ) {
      this.closeEQModal();
    }
  };

  private renderEQTable(): void {
    console.log("[EQ Debug] Rendering playback configuration");
    console.log(
      "[EQ Debug] Playback options container:",
      this.playbackOptionsContainer,
    );
    console.log("[EQ Debug] EQ table container:", this.eqTableContainer);
    console.log("[EQ Debug] Current filter params:", this.currentFilterParams);

    if (!this.playbackOptionsContainer || !this.eqTableContainer) {
      console.error("[EQ Debug] Container not found");
      return;
    }

    // Render playback options section
    this.renderPlaybackOptions();

    // Render EQ table section
    const eqSection = document.createElement("div");
    eqSection.className = "eq-section";

    const header = document.createElement("h4");
    header.textContent = "Equalizer Configuration";

    // Create a container for the graph
    const graphContainer = document.createElement("div");
    graphContainer.className = "eq-graph-container";
    const canvas = document.createElement("canvas");
    canvas.className = "eq-graph-canvas";
    graphContainer.appendChild(canvas);

    const table = document.createElement("table");
    table.className = "eq-table-vertical";
    table.innerHTML = `
      <thead>
        <tr>
          <th class="eq-row-label"></th>
          ${this.currentFilterParams
            .map(
              (filter, index) => `
            <th data-filter-index="${index}" class="eq-column-header ${index === this.selectedFilterIndex ? "selected" : ""}">
              Filter ${index + 1}
            </th>
          `,
            )
            .join("")}
        </tr>
      </thead>
      <tbody>
        <tr class="eq-row">
          <td class="eq-row-label">Type</td>
          ${this.currentFilterParams
            .map(
              (filter, index) => `
            <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
              <select data-index="${index}" class="eq-filter-type">
                ${Object.entries(FILTER_TYPES)
                  .map(
                    ([type, info]) => `
                  <option value="${type}" ${filter.filter_type === type ? "selected" : ""}>
                    ${info.icon} ${info.shortName}
                  </option>
                `,
                  )
                  .join("")}
              </select>
            </td>
          `,
            )
            .join("")}
        </tr>
        <tr class="eq-row">
          <td class="eq-row-label">Enable</td>
          ${this.currentFilterParams
            .map(
              (filter, index) => `
            <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
              <input type="checkbox" data-index="${index}" class="eq-enabled" ${filter.enabled ? "checked" : ""}>
            </td>
          `,
            )
            .join("")}
        </tr>
        <tr class="eq-row">
          <td class="eq-row-label">Freq (Hz)</td>
          ${this.currentFilterParams
            .map(
              (filter, index) => `
            <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
              <input type="number" data-index="${index}" class="eq-frequency" value="${filter.frequency.toFixed(1)}" step="1">
            </td>
          `,
            )
            .join("")}
        </tr>
        <tr class="eq-row">
          <td class="eq-row-label">Gain (dB)</td>
          ${this.currentFilterParams
            .map(
              (filter, index) => `
            <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
              <input type="number" data-index="${index}" class="eq-gain" value="${filter.gain.toFixed(2)}" step="0.1">
            </td>
          `,
            )
            .join("")}
        </tr>
        <tr class="eq-row">
          <td class="eq-row-label">Q</td>
          ${this.currentFilterParams
            .map(
              (filter, index) => `
            <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
              <input type="number" data-index="${index}" class="eq-q" value="${filter.q.toFixed(2)}" step="0.1">
            </td>
          `,
            )
            .join("")}
        </tr>
      </tbody>
    `;

    this.eqTableContainer.innerHTML = "";
    eqSection.appendChild(header);
    eqSection.appendChild(graphContainer);
    eqSection.appendChild(table);
    this.eqTableContainer.appendChild(eqSection);

    // Update the canvas reference and context
    this.eqGraphCanvas = canvas;
    this.eqGraphCtx = canvas.getContext("2d");
    this.resizeEQGraphCanvas();

    // Add column click handler to sync with graph selection
    table.addEventListener("click", (e) => {
      const cell = (e.target as HTMLElement).closest("td, th") as HTMLElement;
      if (cell && cell.dataset.filterIndex) {
        const index = parseInt(cell.dataset.filterIndex, 10);
        this.selectedFilterIndex = index;
        this.drawEQGraph();
      }
    });

    table.addEventListener("input", (e) => this.handleEQTableChange(e));
  }

  private renderPlaybackOptions(): void {
    if (!this.playbackOptionsContainer) return;

    const optionsHTML = `
      <div class="playback-options-section">
        <h4>Playback Options</h4>
        <div class="playback-option-item">
          <label class="playback-option-label">
            <input type="checkbox" id="loudness-compensation" class="loudness-compensation-toggle" ${this.loudnessCompensation ? "checked" : ""}>
            <span>Loudness Compensation</span>
          </label>
        </div>
        <div class="playback-option-item spl-slider-container" style="display: ${this.loudnessCompensation ? "flex" : "none"};">
          <label class="playback-option-label" for="spl-amplitude">
            <span>SPL Amplitude: <span class="spl-value">${this.splAmplitude}</span> dB</span>
          </label>
          <input type="range" id="spl-amplitude" class="spl-slider" min="-30" max="0" step="1" value="${this.splAmplitude}">
        </div>
        <div class="playback-option-item">
          <label class="playback-option-label">
            <input type="checkbox" id="auto-gain" class="auto-gain-toggle" ${this.autoGain ? "checked" : ""}>
            <span>Auto-Gain</span>
          </label>
          <div class="auto-gain-warning" style="display: ${this.autoGain ? "none" : "block"};">
            <span class="warning-icon">⚠️</span>
            <span class="warning-text">Disabling auto-gain may cause clipping</span>
          </div>
        </div>
      </div>
    `;

    this.playbackOptionsContainer.innerHTML = optionsHTML;

    // Attach event listeners
    const loudnessToggle = this.playbackOptionsContainer.querySelector(
      ".loudness-compensation-toggle",
    ) as HTMLInputElement;
    const splSliderContainer = this.playbackOptionsContainer.querySelector(
      ".spl-slider-container",
    ) as HTMLElement;
    const splSlider = this.playbackOptionsContainer.querySelector(
      ".spl-slider",
    ) as HTMLInputElement;
    const splValueDisplay = this.playbackOptionsContainer.querySelector(
      ".spl-value",
    ) as HTMLElement;
    const autoGainToggle = this.playbackOptionsContainer.querySelector(
      ".auto-gain-toggle",
    ) as HTMLInputElement;
    const autoGainWarning = this.playbackOptionsContainer.querySelector(
      ".auto-gain-warning",
    ) as HTMLElement;

    if (loudnessToggle) {
      loudnessToggle.addEventListener("change", (e) => {
        this.loudnessCompensation = (e.target as HTMLInputElement).checked;
        if (splSliderContainer) {
          splSliderContainer.style.display = this.loudnessCompensation
            ? "flex"
            : "none";
        }
        console.log("Loudness compensation:", this.loudnessCompensation);
      });
    }

    if (splSlider && splValueDisplay) {
      splSlider.addEventListener("input", (e) => {
        this.splAmplitude = parseInt((e.target as HTMLInputElement).value, 10);
        splValueDisplay.textContent = this.splAmplitude.toString();
        console.log("SPL amplitude:", this.splAmplitude);
      });
    }

    if (autoGainToggle && autoGainWarning) {
      autoGainToggle.addEventListener("change", (e) => {
        this.autoGain = (e.target as HTMLInputElement).checked;
        autoGainWarning.style.display = this.autoGain ? "none" : "block";
        console.log("Auto-gain:", this.autoGain);
      });
    }
  }

  private handleEQTableChange(e: Event): void {
    const target = e.target as HTMLInputElement | HTMLSelectElement;
    const index = parseInt(target.dataset.index || "0", 10);
    let type = target.className.replace("eq-", "");

    if (isNaN(index) || !this.currentFilterParams[index]) return;

    let value: number | boolean | string;
    if (target instanceof HTMLSelectElement) {
      type = "filter_type";
      value = target.value;
    } else if (target.type === "checkbox") {
      value = target.checked;
    } else {
      value = parseFloat(target.value);
      if (isNaN(value as number)) return;
    }

    // Update the filter parameter
    (
      this.currentFilterParams[index] as unknown as Record<
        string,
        number | boolean | string
      >
    )[type] = value;

    // Select this filter in the graph
    this.selectedFilterIndex = index;

    // Request graph update
    this.requestEQResponseUpdate();

    // Redraw graph to show selection and updated values
    this.drawEQGraph();

    // Update filter parameters - this will also update the display
    this.updateFilterParams(this.currentFilterParams);
  }

  private formatTrackName(key: string): string {
    return key
      .split("_")
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");
  }

  private async loadDemoTrack(trackName: string): Promise<void> {
    const url = this.config.demoTracks?.[trackName];
    if (!url) {
      throw new Error(`Demo track '${trackName}' not found`);
    }

    try {
      const fileName = url.split("/").pop();
      if (!fileName) {
        throw new Error("Invalid demo track URL");
      }
      const filePath = await resolveResource(`public/demo-audio/${fileName}`);
      await this.loadAudioFilePath(filePath);
    } catch (error) {
      this.setStatus("Failed to load audio");
      this.callbacks.onError?.("Failed to load demo track: " + error);
      throw error;
    }
  }

  public async loadAudioFilePath(filePath: string): Promise<void> {
    this.setStatus("Loading...");

    try {
      // Load the file via streaming manager
      this.currentAudioPath = filePath;
      await this.streamingManager.loadAudioFilePath(filePath);
      // File info will be received via onFileLoaded callback

      this.setListenButtonEnabled(true);
      this.showAudioStatus(true);

      // Analyze ReplayGain for the track
      await this.analyzeReplayGain(filePath);
    } catch (error) {
      this.setStatus("Failed to load audio");
      this.callbacks.onError?.("Failed to load audio file: " + error);
      throw error;
    }
  }

  private clearAudio(): void {
    this.stop();
    this.audioBuffer = null;
    this.setListenButtonEnabled(false);
    this.showAudioStatus(false);
    this.setStatus("No audio selected");

    // Hide ReplayGain display
    if (this.replayGainContainer) {
      this.replayGainContainer.style.display = "none";
    }
    this.replayGainInfo = null;
  }

  private setStatus(status: string): void {
    if (this.statusText) {
      this.statusText.textContent = status;
    }
  }

  private setListenButtonEnabled(enabled: boolean): void {
    if (this.listenBtn) {
      this.listenBtn.disabled = !enabled;
      if (enabled) {
        this.listenBtn.classList.remove("disabled");
      } else {
        this.listenBtn.classList.add("disabled");
      }
    }
  }

  private showAudioStatus(_show: boolean): void {
    // Progress bar is always visible now, so we don't hide it
    // This method is kept for backward compatibility but does nothing
    const audioStatus = this.container.querySelector(
      ".audio-status",
    ) as HTMLElement;
    if (audioStatus && this.config.showProgress) {
      audioStatus.style.display = "flex"; // Always show if progress is enabled
    }
  }

  private updateAudioInfo(): void {
    if (this.audioBuffer && this.durationText) {
      const duration = this.audioBuffer.duration;
      this.durationText.textContent = this.formatTime(duration);
    }
  }

  private formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  // EQ Filter Management
  updateFilterParams(filterParams: Partial<ExtendedFilterParam>[]): void {
    this.currentFilterParams = filterParams.map((p) => ({
      ...p,
      frequency: p.frequency || 0,
      q: p.q || 1,
      gain: p.gain || 0,
      enabled: p.enabled ?? true,
      filter_type: p.filter_type || "Peak",
    })) as ExtendedFilterParam[];

    // Recalculate and apply filters
    this.setupEQFilters();

    // If playing, update filters in real-time
    if (this.isAudioPlaying && this.eqEnabled) {
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

  // Clear all EQ filters
  clearEQFilters(): void {
    this.currentFilterParams = [];
    this.setupEQFilters();

    // Update display to show no filters
    this.updateEQInfo(0, 0);
    this.drawEQMiniGraph();

    // If playing, reconnect to remove all filters
    if (this.isAudioPlaying && this.audioSource) {
      this.connectAudioChain();
    }
  }

  private setupEQFilters(): void {
    if (!this.audioContext || !this.gainNode) return;

    // Clear existing filters
    this.eqFilters.forEach((filter) => filter.disconnect());
    this.eqFilters = [];

    // Calculate maximum positive gain for compensation
    let maxPositiveGain = 0;
    let activeFilterCount = 0;

    // Create new filters from parameters
    this.currentFilterParams.forEach((param) => {
      if (param.enabled && Math.abs(param.gain) > 0.1) {
        // Only create filter if enabled and gain is significant
        const filter = this.audioContext!.createBiquadFilter();
        filter.type = "peaking";
        filter.frequency.value = param.frequency;
        filter.Q.value = param.q;
        filter.gain.value = param.gain;
        this.eqFilters.push(filter);
        activeFilterCount++;

        // Track maximum positive gain
        if (param.gain > maxPositiveGain) {
          maxPositiveGain = param.gain;
        }
      }
    });

    // Apply gain compensation to prevent clipping
    if (maxPositiveGain > 0) {
      const compensationGain = Math.pow(10, -maxPositiveGain / 20); // Convert dB to linear scale
      this.gainNode.gain.value = compensationGain;
      console.log(
        `Applied gain compensation: -${maxPositiveGain.toFixed(1)} dB (${compensationGain.toFixed(3)} linear)`,
      );
    } else {
      this.gainNode.gain.value = 1.0; // No compensation needed
    }

    // Update EQ info display
    this.updateEQInfo(activeFilterCount, maxPositiveGain);

    // Draw mini EQ graph
    this.drawEQMiniGraph();

    console.log(
      `Created ${this.eqFilters.length} EQ filters with gain compensation`,
    );
  }

  private connectAudioChain(): void {
    if (!this.audioSource || !this.gainNode || !this.audioContext) {
      console.error("Cannot connect audio chain - missing components");
      return;
    }

    console.log(
      "Connecting audio chain with",
      this.eqFilters.length,
      "EQ filters",
    );
    let currentNode: AudioNode = this.audioSource;

    // Connect EQ filters in series if EQ is enabled
    if (this.eqEnabled) {
      this.eqFilters.forEach((filter, index) => {
        console.log(`Connecting EQ filter ${index + 1}`);
        currentNode.connect(filter);
        currentNode = filter;
      });
    }

    // Connect to gain and analyzer
    currentNode.connect(this.gainNode);

    // Handle output device routing
    let finalDestination: AudioNode;

    if (this.outputDeviceId !== "default") {
      // For specific output devices, use MediaStreamDestination + Audio element
      try {
        const destination = this.audioContext.createMediaStreamDestination();

        // Create or reuse audio element
        if (!this.audioElement) {
          this.audioElement = new Audio();
          this.audioElement.autoplay = true;
        }

        this.audioElement.srcObject = destination.stream;

        // Set the output device
        if ("setSinkId" in this.audioElement) {
          (
            this.audioElement as HTMLAudioElement & {
              setSinkId: (id: string) => Promise<void>;
            }
          )
            .setSinkId(this.outputDeviceId)
            .catch((error: unknown) => {
              console.warn("Failed to set audio element sink ID:", error);
            });
        }

        finalDestination = destination;
      } catch (error) {
        console.error("Failed to set up device routing, using default:", error);
        finalDestination = this.audioContext.destination;
      }
    } else {
      // Use default output device
      finalDestination = this.audioContext.destination;
    }

    if (this.analyserNode) {
      this.gainNode.connect(this.analyserNode);
      this.analyserNode.connect(finalDestination);
    } else {
      this.gainNode.connect(finalDestination);
    }
  }

  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled;

    // Update button states
    if (this.eqOnBtn && this.eqOffBtn) {
      if (enabled) {
        this.eqOnBtn.classList.add("active");
        this.eqOffBtn.classList.remove("active");
      } else {
        this.eqOnBtn.classList.remove("active");
        this.eqOffBtn.classList.add("active");
      }
    }

    // Apply EQ changes in real-time if playing
    if (this.isAudioPlaying) {
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
    }

    // Recalculate active filters and compensation when toggling
    let activeFilterCount = 0;
    let maxPositiveGain = 0;

    if (enabled) {
      // Count active filters and calculate compensation when EQ is on
      this.currentFilterParams.forEach((param) => {
        if (param.enabled && Math.abs(param.gain) > 0.1) {
          activeFilterCount++;
          if (param.gain > maxPositiveGain) {
            maxPositiveGain = param.gain;
          }
        }
      });
    }

    // Update EQ info display
    this.updateEQInfo(
      enabled ? activeFilterCount : 0,
      enabled ? maxPositiveGain : 0,
    );

    // Update mini graph to show enabled/disabled state
    this.drawEQMiniGraph();

    console.log(`EQ ${enabled ? "enabled" : "disabled"}`);
    this.callbacks.onEQToggle?.(enabled);
  }

  // Set output device for audio playback
  setOutputDevice(deviceId: string): void {
    this.outputDeviceId = deviceId || "default";
    console.log(`Audio player output device set to: ${this.outputDeviceId}`);

    // If we have an audio element, update its sink ID
    if (this.audioElement && "setSinkId" in this.audioElement) {
      (
        this.audioElement as HTMLAudioElement & {
          setSinkId: (id: string) => Promise<void>;
        }
      )
        .setSinkId(this.outputDeviceId)
        .catch((error: unknown) => {
          console.warn("Failed to set audio element sink ID:", error);
        });
    }
  }

  getOutputDevice(): string {
    return this.outputDeviceId;
  }

  // Update EQ info display
  private updateEQInfo(filterCount: number, compensationDb: number): void {
    if (this.eqFilterCountText) {
      this.eqFilterCountText.textContent = `#${filterCount}`;
    }
    if (this.eqGainCompText) {
      this.eqGainCompText.textContent =
        compensationDb > 0 ? `-${compensationDb.toFixed(1)}dB` : "0dB";
    }
  }

  // Draw mini EQ graph
  private drawEQMiniGraph(): void {
    if (!this.eqMiniCanvas || !this.eqMiniCtx || !this.audioContext) return;

    const ctx = this.eqMiniCtx;
    const width = this.eqMiniCanvas.width;
    const height = this.eqMiniCanvas.height;

    // Detect color scheme
    const isDarkMode =
      window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: dark)").matches;

    // Properly clear the canvas - reset all state
    ctx.clearRect(0, 0, width, height);

    // Fill background
    ctx.fillStyle = isDarkMode
      ? "rgba(0, 0, 0, 0.2)"
      : "rgba(255, 255, 255, 0.2)";
    ctx.fillRect(0, 0, width, height);

    // Draw grid line at 0dB
    ctx.strokeStyle = isDarkMode
      ? "rgba(255, 255, 255, 0.1)"
      : "rgba(0, 0, 0, 0.1)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, height / 2);
    ctx.lineTo(width, height / 2);
    ctx.stroke();

    // Check if we should draw the EQ curve
    const hasFilters = this.currentFilterParams.some(
      (p) => p.enabled && Math.abs(p.gain) > 0.1,
    );

    if (!hasFilters) {
      // Draw flat line when no active filters
      ctx.strokeStyle = isDarkMode
        ? "rgba(255, 255, 255, 0.3)"
        : "rgba(0, 0, 0, 0.3)";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(0, height / 2);
      ctx.lineTo(width, height / 2);
      ctx.stroke();
      return;
    }

    // Calculate frequency response
    const numPoints = width;
    const minFreq = 20;
    const maxFreq = 20000;

    // First pass: calculate actual min/max gain values across the spectrum
    let minGainValue = 0;
    let maxGainValue = 0;
    const gains: number[] = [];

    for (let x = 0; x < numPoints; x++) {
      // Calculate frequency for this x position (logarithmic scale)
      const logMin = Math.log10(minFreq);
      const logMax = Math.log10(maxFreq);
      const logFreq = logMin + (logMax - logMin) * (x / numPoints);
      const freq = Math.pow(10, logFreq);

      // Calculate combined gain at this frequency
      let totalGain = 0;
      this.currentFilterParams.forEach((param) => {
        if (param.enabled && Math.abs(param.gain) > 0.1) {
          // Simplified peaking filter response calculation
          const relativeFreq = freq / param.frequency;
          void relativeFreq; // Used for documentation
          const distance = Math.abs(Math.log2(relativeFreq));
          const attenuation = Math.exp(-Math.pow(distance * param.q, 2));
          totalGain += param.gain * attenuation;
        }
      });

      gains.push(totalGain);
      minGainValue = Math.min(minGainValue, totalGain);
      maxGainValue = Math.max(maxGainValue, totalGain);
    }

    // Set y-axis range to [min-1, max+1] for better visualization
    const yMin = minGainValue - 1;
    const yMax = maxGainValue + 1;
    const yRange = yMax - yMin;

    // Calculate 0dB line position for grid
    const zeroLineY =
      yRange !== 0 ? height - ((0 - yMin) / yRange) * height : height / 2;

    // Clear and redraw grid line at 0dB with new scale
    ctx.strokeStyle = isDarkMode
      ? "rgba(255, 255, 255, 0.1)"
      : "rgba(0, 0, 0, 0.1)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, zeroLineY);
    ctx.lineTo(width, zeroLineY);
    ctx.stroke();

    // Use different colors/opacity based on whether EQ is enabled
    if (this.eqEnabled) {
      ctx.strokeStyle = isDarkMode ? "#4dabf7" : "#007bff";
      ctx.lineWidth = 2;
    } else {
      // Show dimmed curve when EQ is disabled
      ctx.strokeStyle = isDarkMode
        ? "rgba(77, 171, 247, 0.4)"
        : "rgba(0, 123, 255, 0.4)";
      ctx.lineWidth = 1.5;
    }
    ctx.beginPath();

    // Second pass: draw the curve using calculated gains and optimized y-axis
    for (let x = 0; x < numPoints; x++) {
      const totalGain = gains[x];

      // Scale to canvas using optimized y-axis range
      const y =
        yRange !== 0
          ? height - ((totalGain - yMin) / yRange) * height
          : height / 2;

      if (x === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }

    ctx.stroke();

    // Draw filter center frequencies as dots
    this.currentFilterParams.forEach((param) => {
      if (param.enabled && Math.abs(param.gain) > 0.1) {
        // Calculate x position for this frequency
        const logMin = Math.log10(minFreq);
        const logMax = Math.log10(maxFreq);
        const logFreq = Math.log10(param.frequency);
        const x = ((logFreq - logMin) / (logMax - logMin)) * width;

        // Calculate y position for the gain using optimized y-axis range
        const y =
          yRange !== 0
            ? height - ((param.gain - yMin) / yRange) * height
            : height / 2;

        // Draw dot with opacity based on EQ enabled state
        if (this.eqEnabled) {
          ctx.fillStyle =
            param.gain > 0
              ? isDarkMode
                ? "#57f287"
                : "#28a745"
              : isDarkMode
                ? "#ed4245"
                : "#dc3545";
        } else {
          // Dimmed dots when EQ is disabled
          ctx.fillStyle =
            param.gain > 0
              ? isDarkMode
                ? "rgba(87, 242, 135, 0.4)"
                : "rgba(40, 167, 69, 0.4)"
              : isDarkMode
                ? "rgba(237, 66, 69, 0.4)"
                : "rgba(220, 53, 69, 0.4)";
        }
        ctx.beginPath();
        ctx.arc(x, y, this.eqEnabled ? 2 : 1.5, 0, Math.PI * 2);
        ctx.fill();
      }
    });
  }

  // Position Updates
  private startPositionUpdates(): void {
    let updateCount = 0;
    const updatePosition = () => {
      if (!this.isAudioPlaying) {
        this.audioAnimationFrame = null;
        return;
      }

      const currentTime = this.getCurrentTime();
      const duration = this.getDuration();

      if (this.positionText) {
        this.positionText.textContent = this.formatTime(currentTime);
      }

      if (this.progressFill && duration > 0) {
        const progress = (currentTime / duration) * 100;
        this.progressFill.style.width = `${Math.min(progress, 100)}%`;
      }

      // Update metrics display every 5 frames to reduce overhead
      if (updateCount++ % 5 === 0) {
        this.updateMetricsDisplay();
      }

      this.audioAnimationFrame = requestAnimationFrame(updatePosition);
    };

    updatePosition();
  }

  private getCurrentTime(): number {
    if (!this.audioContext || !this.isAudioPlaying) return 0;
    return this.audioContext.currentTime - this.audioStartTime;
  }

  private getCurrentTimeWhilePaused(): number {
    // Return the saved pause time when audio is paused
    if (this.isAudioPaused) {
      return this.audioPauseTime;
    }
    return this.getCurrentTime();
  }

  private async restartFromPosition(startTime: number): Promise<void> {
    console.log("Restarting audio from position:", startTime);

    if (!this.audioContext || !this.audioBuffer) {
      console.error("Cannot restart: missing audio context or buffer");
      return;
    }

    // Resume audio context if suspended
    if (this.audioContext.state === "suspended") {
      await this.audioContext.resume();
    }

    // Stop current source
    if (this.audioSource) {
      try {
        this.audioSource.stop();
      } catch (_error) {
        // Ignore errors if already stopped
      }
      this.audioSource = null;
    }

    try {
      // Create new audio source
      this.audioSource = this.audioContext.createBufferSource();
      this.audioSource.buffer = this.audioBuffer;

      // Reconnect audio chain with current EQ settings
      this.connectAudioChain();

      // Start from the saved position
      this.audioSource.start(0, startTime);
      this.audioStartTime = this.audioContext.currentTime - startTime;
      this.isAudioPlaying = false; // Keep it paused
      this.isAudioPaused = true;
      this.audioPauseTime = startTime;

      this.audioSource.onended = () => {
        console.log("Audio playback ended");
        this.isAudioPlaying = false;
        this.isAudioPaused = false;
        this.audioSource = null;
        this.updatePlaybackUI();
        if (this.audioAnimationFrame) {
          cancelAnimationFrame(this.audioAnimationFrame);
          this.audioAnimationFrame = null;
        }
      };

      // Immediately suspend to keep it paused
      this.audioContext.suspend();
      this.updatePlaybackUI();
      console.log("Audio restarted and paused at position:", startTime);
    } catch (error) {
      console.error("Error during audio restart:", error);
      this.callbacks.onError?.("Failed to restart audio: " + error);
    }
  }

  private getDuration(): number {
    return this.audioBuffer ? this.audioBuffer.duration : 0;
  }

  // Playback Controls
  async play(): Promise<void> {
    console.log("Play method called");

    if (!this.currentAudioPath) {
      throw new Error("No audio file loaded");
    }

    try {
      // Convert current filter params to the format streaming backend expects
      const filters = this.eqEnabled
        ? this.currentFilterParams
            .filter((p) => p.enabled)
            .map((p) => ({
              frequency: p.frequency,
              q: p.q,
              gain: p.gain,
            }))
        : [];

      // Start spectrum analyzer BEFORE playback if enabled
      if (this.spectrumAnalyzer && this.config.enableSpectrum) {
        try {
          await this.spectrumAnalyzer.start();
          console.log("Spectrum analyzer started before playback");
        } catch (error) {
          console.error("Failed to start spectrum analyzer:", error);
        }
      }

      // Enable loudness monitoring in backend
      try {
        await invoke("stream_enable_loudness_monitoring");
        console.log("Loudness monitoring enabled");
      } catch (error) {
        console.error("Failed to enable loudness monitoring:", error);
      }

      // Start loudness polling
      if (!this.loudnessPollingActive) {
        this.streamingManager.startLoudnessPolling(100, (loudnessInfo) => {
          console.log("[Loudness] Polling callback - info:", loudnessInfo);
          if (loudnessInfo) {
            const momentaryElement = document.getElementById("metrics-lufs-m");
            const shorttermElement = document.getElementById("metrics-lufs-s");

            console.log(
              "[Loudness] Elements found - M:",
              !!momentaryElement,
              "S:",
              !!shorttermElement,
            );

            if (momentaryElement && shorttermElement) {
              const momentaryText =
                loudnessInfo.momentary_lufs === -Infinity
                  ? "-∞"
                  : loudnessInfo.momentary_lufs.toFixed(1);
              const shorttermText =
                loudnessInfo.shortterm_lufs === -Infinity
                  ? "-∞"
                  : loudnessInfo.shortterm_lufs.toFixed(1);

              console.log(
                "[Loudness] Updating - M:",
                momentaryText,
                "S:",
                shorttermText,
              );

              momentaryElement.textContent = momentaryText;
              shorttermElement.textContent = shorttermText;
            }
          }
        });
        this.loudnessPollingActive = true;
        console.log("Loudness polling started");
      }

      await this.streamingManager.play(filters);

      this.callbacks.onPlay?.();
      console.log("Audio playback started successfully via streaming backend");
    } catch (error) {
      console.error("Error during audio playback:", error);
      this.callbacks.onError?.("Playback failed: " + error);
      throw error;
    }
  }

  async pause(): Promise<void> {
    try {
      await this.streamingManager.pause();
      console.log("Audio paused via streaming backend");
    } catch (error) {
      console.error("Error pausing playback:", error);
      this.callbacks.onError?.("Failed to pause: " + error);
    }
  }

  private restart(): void {
    console.log("Restarting audio playback");
    this.stop();
    // Small delay to ensure stop is complete
    setTimeout(() => {
      this.play();
    }, 50);
  }

  async resume(): Promise<void> {
    try {
      await this.streamingManager.resume();
      console.log("Audio resumed via streaming backend");
    } catch (error) {
      console.error("Error resuming playback:", error);
      this.callbacks.onError?.("Failed to resume: " + error);
    }
  }

  async stop(): Promise<void> {
    try {
      // Stop spectrum analyzer if running
      if (this.spectrumAnalyzer) {
        try {
          await this.spectrumAnalyzer.stop();
          console.log("Spectrum analyzer stopped");
        } catch (error) {
          console.error("Failed to stop spectrum analyzer:", error);
        }
      }

      await this.streamingManager.stop();

      // Stop loudness polling
      if (this.loudnessPollingActive) {
        this.streamingManager.stopLoudnessPolling();
        this.loudnessPollingActive = false;
        console.log("Loudness polling stopped");
      }

      // Disable loudness monitoring in backend
      try {
        await invoke("stream_disable_loudness_monitoring");
        console.log("Loudness monitoring disabled");
      } catch (error) {
        console.error("Failed to disable loudness monitoring:", error);
      }

      if (this.positionText) {
        this.positionText.textContent = "--:--";
      }
      if (this.progressFill) {
        this.progressFill.style.width = "0%";
      }

      this.callbacks.onStop?.();
      console.log("Audio playback stopped via streaming backend");
    } catch (error) {
      console.error("Error stopping playback:", error);
      this.callbacks.onError?.("Failed to stop: " + error);
    }
  }

  private handlePauseClick(): void {
    this.pauseClickCount++;

    if (this.pauseClickCount === 1) {
      // First click - pause
      this.pause();

      // Set a timer to reset click count
      this.pauseClickTimer = window.setTimeout(() => {
        this.pauseClickCount = 0;
        this.pauseClickTimer = null;
      }, 500); // 500ms window for double-click
    } else if (this.pauseClickCount === 2) {
      // Second click - restart
      if (this.pauseClickTimer) {
        clearTimeout(this.pauseClickTimer);
        this.pauseClickTimer = null;
      }
      this.pauseClickCount = 0;
      this.restart();
    }
  }

  private updatePlaybackUI(): void {
    const isPlaying = this.isAudioPlaying;
    const isPaused = this.isAudioPaused;

    // Update button states based on playback status
    if (this.listenBtn) {
      this.listenBtn.disabled = isPlaying;
    }

    if (this.pauseBtn) {
      this.pauseBtn.disabled = !isPlaying;
    }

    if (this.stopBtn) {
      this.stopBtn.disabled = !isPlaying && !isPaused;
    }

    if (this.statusText) {
      let status = "Audio ready";
      if (isPaused) {
        status = this.eqEnabled ? "Paused (EQ On)" : "Paused (EQ Off)";
      } else if (isPlaying) {
        status = this.eqEnabled ? "Playing (EQ On)" : "Playing (EQ Off)";
      }
      this.statusText.textContent = status;
    }
  }

  // Public API
  isPlaying(): boolean {
    return this.isAudioPlaying;
  }

  isEQEnabled(): boolean {
    return this.eqEnabled;
  }

  getCurrentTrack(): string | null {
    return this.demoSelect?.value || null;
  }

  // ReplayGain analysis
  private async analyzeReplayGain(filePath: string): Promise<void> {
    console.log(`[ReplayGain] Starting analysis for: ${filePath}`);
    try {
      // Show loading state in metrics block
      const gainElement = document.getElementById("metrics-replay-gain");
      const peakElement = document.getElementById("metrics-peak");
      if (gainElement) gainElement.textContent = "...";
      if (peakElement) peakElement.textContent = "...";

      // Call Tauri command
      console.log("[ReplayGain] Invoking backend analysis");
      const info = await invoke<ReplayGainInfo>("analyze_replaygain", {
        filePath,
      });

      console.log(`[ReplayGain] Received result from backend:`, info);

      // Store the result
      this.replayGainInfo = info;

      // Update display
      this.updateReplayGainDisplay(info.gain, info.peak);
    } catch (error) {
      console.error("Failed to analyze ReplayGain:", error);
      // Hide display on error
      if (this.replayGainContainer) {
        this.replayGainContainer.style.display = "none";
      }
      this.replayGainInfo = null;
    }
  }

  private updateReplayGainDisplay(gain: number, peak: number): void {
    // Update metrics block with replay gain values
    const gainElement = document.getElementById("metrics-replay-gain");
    const peakElement = document.getElementById("metrics-peak");

    if (gainElement && peakElement) {
      const gainText = gain >= 0 ? `+${gain.toFixed(2)}` : `${gain.toFixed(2)}`;
      const peakText = peak.toFixed(3);

      gainElement.textContent = gainText;
      peakElement.textContent = peakText;
    }

    // Also update the old replay gain container if it exists (for backward compatibility)
    if (this.replayGainContainer) {
      const gainElement =
        this.replayGainContainer.querySelector(".replay-gain-value");
      const peakElement =
        this.replayGainContainer.querySelector(".replay-peak-value");

      if (gainElement && peakElement) {
        const gainText =
          gain >= 0 ? `+${gain.toFixed(2)} dB` : `${gain.toFixed(2)} dB`;
        gainElement.textContent = gainText;
        peakElement.textContent = peak.toFixed(6);
        this.replayGainContainer.style.display = "block";
      }
    }
  }

  // Update metrics display with loudness and replay gain data
  private updateMetricsDisplay(): void {
    // Update replay gain and peak from replay gain info
    if (this.replayGainInfo) {
      const gainElement = document.getElementById("metrics-replay-gain");
      const peakElement = document.getElementById("metrics-peak");

      if (gainElement && peakElement) {
        const gainText =
          this.replayGainInfo.gain >= 0
            ? `+${this.replayGainInfo.gain.toFixed(2)}`
            : `${this.replayGainInfo.gain.toFixed(2)}`;
        const peakText = this.replayGainInfo.peak.toFixed(3);

        gainElement.textContent = gainText;
        peakElement.textContent = peakText;
      }
    }

    // Fetch and update loudness data asynchronously
    this.streamingManager
      .getLoudness()
      .then((loudnessInfo) => {
        console.log("[Metrics] Got loudness info:", loudnessInfo);

        if (loudnessInfo) {
          const momentaryElement = document.getElementById("metrics-lufs-m");
          const shorttermElement = document.getElementById("metrics-lufs-s");

          console.log(
            "[Metrics] Elements found - M:",
            !!momentaryElement,
            "S:",
            !!shorttermElement,
          );

          if (momentaryElement && shorttermElement) {
            const momentaryText =
              loudnessInfo.momentary_lufs === -Infinity
                ? "-∞"
                : loudnessInfo.momentary_lufs.toFixed(1);
            const shorttermText =
              loudnessInfo.shortterm_lufs === -Infinity
                ? "-∞"
                : loudnessInfo.shortterm_lufs.toFixed(1);

            console.log(
              "[Metrics] Updating LUFS - M:",
              momentaryText,
              "S:",
              shorttermText,
            );

            momentaryElement.textContent = momentaryText;
            shorttermElement.textContent = shorttermText;
          }
        } else {
          console.log("[Metrics] No loudness info available");
        }
      })
      .catch((error) => {
        console.error("[Metrics] Failed to get loudness:", error);
      });
  }

  // Cleanup
  destroy(): void {
    this.stop();
    if (this.spectrumAnalyzer) {
      this.spectrumAnalyzer.destroy();
    }

    // Remove window resize listener
    if (this.resizeHandler) {
      window.removeEventListener("resize", this.resizeHandler);
      this.resizeHandler = null;
    }

    // Remove modal and backdrop from DOM
    const modal = document.getElementById(this.instanceId + "-eq-modal");
    const backdrop = document.getElementById(this.instanceId + "-eq-backdrop");
    if (modal) modal.remove();
    if (backdrop) backdrop.remove();

    this.eqFilters.forEach((filter) => filter.disconnect());
    this.eqFilters = [];

    if (this.gainNode) {
      this.gainNode.disconnect();
    }

    if (this.analyserNode) {
      this.analyserNode.disconnect();
    }

    if (this.audioContext && this.audioContext.state !== "closed") {
      this.audioContext.close();
    }

    this.audioContext = null;
    this.audioBuffer = null;
    this.gainNode = null;
    this.analyserNode = null;
  }

  // ===== EQ GRAPH IMPLEMENTATION =====

  private resizeEQGraphCanvas(): void {
    if (!this.eqGraphCanvas) return;
    const container = this.eqGraphCanvas.parentElement;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const width = Math.max(rect.width || 600, 400);
    const height = 300;
    this.eqGraphCanvas.width = width;
    this.eqGraphCanvas.height = height;
    this.drawEQGraph();
  }

  private async computeEQResponse(): Promise<void> {
    if (!this.currentFilterParams || this.currentFilterParams.length === 0) {
      this.eqResponseData = null;
      return;
    }
    try {
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
    }, 60);
  }

  private drawEQGraph(): void {
    if (!this.eqGraphCanvas || !this.eqGraphCtx) {
      console.log("[EQ Graph] Canvas or context not available");
      return;
    }
    const ctx = this.eqGraphCtx;
    const width = this.eqGraphCanvas.width;
    const height = this.eqGraphCanvas.height;
    const isDarkMode = window.matchMedia?.(
      "(prefers-color-scheme: dark)",
    ).matches;

    // Compute dynamic Y-axis range from response data
    if (this.eqResponseData) {
      this.computeDynamicYAxisRange();
    }

    console.log(
      "[EQ Graph] Drawing graph - canvas:",
      width,
      "x",
      height,
      "data:",
      !!this.eqResponseData,
    );
    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = isDarkMode ? "rgb(26, 26, 26)" : "rgb(255, 255, 255)";
    ctx.fillRect(0, 0, width, height);
    this.drawGrid(ctx, width, height, isDarkMode);
    if (this.eqResponseData) {
      console.log("[EQ Graph] Drawing response curves");
      this.drawIndividualResponses(ctx, width, height, isDarkMode);
      this.drawCombinedResponse(ctx, width, height, isDarkMode);
    } else {
      console.log("[EQ Graph] No response data available");
    }
    this.drawFilterHandles(ctx, width, height, isDarkMode);
  }

  private computeDynamicYAxisRange(): void {
    if (!this.eqResponseData) return;

    let minGain = Infinity;
    let maxGain = -Infinity;

    // Check combined response
    if (
      this.eqResponseData.combined_response &&
      Array.isArray(this.eqResponseData.combined_response)
    ) {
      this.eqResponseData.combined_response.forEach((gain: number) => {
        minGain = Math.min(minGain, gain);
        maxGain = Math.max(maxGain, gain);
      });
    }

    // Check individual responses (could be array or object)
    if (this.eqResponseData.individual_responses) {
      const responses = this.eqResponseData.individual_responses;
      if (Array.isArray(responses)) {
        // If it's an array of arrays
        responses.forEach((response: number[]) => {
          if (Array.isArray(response)) {
            response.forEach((gain: number) => {
              minGain = Math.min(minGain, gain);
              maxGain = Math.max(maxGain, gain);
            });
          }
        });
      } else if (typeof responses === "object") {
        // If it's an object with numeric keys
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

    // Set dynamic range with 1dB padding
    if (minGain !== Infinity && maxGain !== -Infinity) {
      this.eqGraphMinGain = minGain - 1;
      this.eqGraphMaxGain = maxGain + 1;
      console.log(
        "[EQ Graph] Dynamic Y-axis range:",
        this.eqGraphMinGain,
        "to",
        this.eqGraphMaxGain,
      );
    }
  }

  private drawGrid(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    isDarkMode: boolean,
  ): void {
    ctx.strokeStyle = isDarkMode
      ? "rgba(255, 255, 255, 0.1)"
      : "rgba(0, 0, 0, 0.1)";
    ctx.lineWidth = 1;
    const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqMarkers.forEach((freq) => {
      const x = this.freqToX(freq, width);
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    });
    const gainMarkers = [-18, -12, -6, 0, 3];
    gainMarkers.forEach((gain) => {
      const y = this.gainToY(gain, height);
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      if (gain === 0) {
        ctx.lineWidth = 2;
        ctx.strokeStyle = isDarkMode
          ? "rgba(255, 255, 255, 0.3)"
          : "rgba(0, 0, 0, 0.3)";
      }
      ctx.stroke();
      ctx.lineWidth = 1;
      ctx.strokeStyle = isDarkMode
        ? "rgba(255, 255, 255, 0.1)"
        : "rgba(0, 0, 0, 0.1)";
    });
    ctx.fillStyle = isDarkMode
      ? "rgba(255, 255, 255, 0.5)"
      : "rgba(0, 0, 0, 0.5)";
    ctx.font = "10px sans-serif";
    freqMarkers.forEach((freq) => {
      const x = this.freqToX(freq, width);
      const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
      ctx.fillText(label, x + 2, height - 4);
    });
    gainMarkers.forEach((gain) => {
      const y = this.gainToY(gain, height);
      ctx.fillText(`${gain > 0 ? "+" : ""}${gain}dB`, 4, y - 2);
    });
  }

  private drawCombinedResponse(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    isDarkMode: boolean,
  ): void {
    if (!this.eqResponseData?.combined_response) return;
    const { frequencies, combined_response } = this.eqResponseData;
    ctx.strokeStyle = isDarkMode ? "#4dabf7" : "#007bff";
    ctx.lineWidth = 2;
    ctx.beginPath();
    frequencies.forEach((freq: number, i: number) => {
      const x = this.freqToX(freq, width);
      const y = this.gainToY(combined_response[i], height);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
  }

  private drawIndividualResponses(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    isDarkMode: boolean,
  ): void {
    if (!this.eqResponseData?.individual_responses) return;
    const { frequencies, individual_responses } = this.eqResponseData;
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
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    });
  }

  private drawFilterHandles(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    isDarkMode: boolean,
  ): void {
    this.currentFilterParams.forEach((filter, idx) => {
      if (!filter.enabled) return;
      const x = this.freqToX(filter.frequency, width);
      const y = this.gainToY(filter.gain, height);
      const isSelected = idx === this.selectedFilterIndex;
      ctx.strokeStyle = isSelected
        ? isDarkMode
          ? "#fa5252"
          : "#dc3545"
        : isDarkMode
          ? "#4dabf7"
          : "#007bff";
      ctx.lineWidth = isSelected ? 3 : 2;
      ctx.fillStyle = isDarkMode
        ? "rgba(77, 171, 247, 0.3)"
        : "rgba(0, 123, 255, 0.3)";
      ctx.beginPath();
      ctx.arc(x, y, isSelected ? 8 : 6, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      if (isSelected) {
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

  private freqToX(freq: number, width: number): number {
    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const logFreq = Math.log10(
      Math.max(this.EQ_GRAPH_MIN_FREQ, Math.min(this.EQ_GRAPH_MAX_FREQ, freq)),
    );
    return ((logFreq - logMin) / (logMax - logMin)) * width;
  }

  private xToFreq(x: number, width: number): number {
    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const logFreq = logMin + (x / width) * (logMax - logMin);
    return Math.pow(10, logFreq);
  }

  private gainToY(gain: number, height: number): number {
    const range = this.eqGraphMaxGain - this.eqGraphMinGain;
    const normalized = (gain - this.eqGraphMinGain) / range;
    return height - normalized * height;
  }

  private yToGain(y: number, height: number): number {
    const range = this.eqGraphMaxGain - this.eqGraphMinGain;
    const normalized = (height - y) / height;
    return this.eqGraphMinGain + normalized * range;
  }

  private handleGraphMouseDown(e: MouseEvent): void {
    if (!this.eqGraphCanvas) return;
    const rect = this.eqGraphCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const width = this.eqGraphCanvas.width;
    const height = this.eqGraphCanvas.height;
    const clickedFreq = this.xToFreq(x, width);

    let closestIdx = -1;
    let minFreqDist = Infinity;
    let dragMode: "ring" | "bar" | null = null;

    // First, check if clicking on Q bar of selected filter
    if (this.selectedFilterIndex >= 0) {
      const filter = this.currentFilterParams[this.selectedFilterIndex];
      if (filter && filter.enabled) {
        const filterX = this.freqToX(filter.frequency, width);
        const filterY = this.gainToY(filter.gain, height);
        const barWidth = 40 / filter.q;
        const dx = x - filterX;
        const dy = y - filterY;
        if (Math.abs(dy) < 5 && Math.abs(dx) < barWidth) {
          closestIdx = this.selectedFilterIndex;
          dragMode = "bar";
        }
      }
    }

    // If not on Q bar, find closest filter by frequency
    if (closestIdx < 0) {
      this.currentFilterParams.forEach((filter, idx) => {
        if (!filter.enabled) return;
        const freqDist = Math.abs(filter.frequency - clickedFreq);
        if (freqDist < minFreqDist) {
          closestIdx = idx;
          minFreqDist = freqDist;
          dragMode = "ring";
        }
      });
    }

    if (closestIdx >= 0) {
      this.selectedFilterIndex = closestIdx;
      this.isDraggingHandle = true;
      this.dragMode = dragMode;
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
    if (this.dragMode === "ring") {
      filter.frequency = Math.max(
        this.EQ_GRAPH_MIN_FREQ,
        Math.min(this.EQ_GRAPH_MAX_FREQ, this.xToFreq(x, width)),
      );
      filter.gain = Math.max(
        this.eqGraphMinGain,
        Math.min(this.eqGraphMaxGain, this.yToGain(y, height)),
      );
    } else if (this.dragMode === "bar") {
      const deltaX = x - this.dragStartX;
      const qDelta = deltaX / 20;
      filter.q = Math.max(
        this.EQ_GRAPH_MIN_Q,
        Math.min(this.EQ_GRAPH_MAX_Q, filter.q + qDelta),
      );
      this.dragStartX = x;
    }
    // Update table inputs to reflect graph changes
    this.updateTableInputs();
    this.requestEQResponseUpdate();
    this.drawEQGraph();
  }

  private handleGraphMouseUp(e: MouseEvent): void {
    if (this.isDraggingHandle) {
      this.isDraggingHandle = false;
      this.dragMode = null;
      this.updateFilterParams(this.currentFilterParams);
    }
  }

  private updateTableInputs(): void {
    if (!this.eqTableContainer) return;
    const filter = this.currentFilterParams[this.selectedFilterIndex];
    if (!filter) return;

    const table = this.eqTableContainer.querySelector("table");
    if (!table) return;

    // Find cells in the selected column
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
}
