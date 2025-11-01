// Standalone Audio Player Module
// Extracted from audio-processor.ts and related UI components

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { resolveResource } from "@tauri-apps/api/path";
import { CamillaAudioManager, AudioFileInfo } from "../audio-manager-camilla";

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
  private camillaManager: CamillaAudioManager | null = null; // Camilla backend manager
  private currentFilterParams: FilterParam[] = [
    { frequency: 100, q: 1.0, gain: 0, enabled: true },
    { frequency: 1000, q: 1.0, gain: 0, enabled: true },
    { frequency: 10000, q: 1.0, gain: 0, enabled: true },
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
      this.setupCamillaManager();
      this._createEQModal();
      this.createUI();
      this.setupEventListeners();
      console.log("AudioPlayer initialized successfully");
    } catch (error) {
      console.error("Failed to initialize AudioPlayer:", error);
      this.callbacks.onError?.("Failed to initialize audio player: " + error);
    }
  }

  private setupCamillaManager(): void {
    this.camillaManager = new CamillaAudioManager({
      onStateChange: (state) => {
        console.log("[AudioPlayer] Camilla state changed:", state);
        if (state === "playing") {
          this.isAudioPlaying = true;
          this.isAudioPaused = false;
        } else if (state === "paused") {
          this.isAudioPlaying = false;
          this.isAudioPaused = true;
        } else if (state === "idle") {
          this.isAudioPlaying = false;
          this.isAudioPaused = false;
        } else if (state === "ended") {
          // Song completed naturally
          console.log("[AudioPlayer] Playback completed");
          this.isAudioPlaying = false;
          this.isAudioPaused = false;
          // Reset position display
          if (this.positionText) {
            this.positionText.textContent = "--:--";
          }
          if (this.progressFill) {
            this.progressFill.style.width = "0%";
          }
          this.setStatus("Playback completed");
        }
        this.updatePlaybackUI();
      },
      onPositionUpdate: (position, duration) => {
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
      },
      onError: (error) => {
        console.error("[AudioPlayer] Camilla error:", error);
        this.callbacks.onError?.(error);
        this.setStatus("Error: " + error);
      },
      onFileLoaded: (info: AudioFileInfo) => {
        console.log("[AudioPlayer] File loaded:", info);
        if (this.durationText && info.duration_seconds) {
          this.durationText.textContent = this.formatTime(
            info.duration_seconds,
          );
        }
      },
    });
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
          <div class="audio-left-controls">
            <div class="demo-track-container">
              <label for="${selectId}" class="demo-track-label">Load a song</label>
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
              <div class="replay-gain-info" style="display: none; margin-top: 8px; font-size: 12px; color: var(--text-secondary);">
                Replay Gain: <span class="info-badge replay-gain-value">--</span> • Peak: <span class="info-badge replay-peak-value">--</span>
              </div>
            </div>
          </div>

          <div class="audio-center-controls">
            <div class="audio-playback-container">
              ${
                this.config.showProgress
                  ? `
                <div class="audio-status" style="display: flex;">
                  <div class="audio-info-compact">
                    <span class="audio-status-text">Ready</span> •
                    <span class="audio-position">--:--</span> •
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
            </div>
          </div>

          <div class="audio-right-controls">
            ${
              this.config.enableSpectrum
                ? `
              <div class="frequency-analyzer" style="display: flex;">
                <canvas class="spectrum-canvas"></canvas>
                ${
                  this.config.showFrequencyLabels
                    ? `
                  <div class="frequency-labels">
                    <span class="freq-label" data-range="sub-bass">Sub Bass<br><small>&lt;60Hz</small></span>
                    <span class="freq-label" data-range="bass">Bass<br><small>60-250Hz</small></span>
                    <span class="freq-label" data-range="low-mid">Low Mid<br><small>250-500Hz</small></span>
                    <span class="freq-label" data-range="mid">Mid<br><small>500-2kHz</small></span>
                    <span class="freq-label" data-range="high-mid">High Mid<br><small>2-4kHz</small></span>
                    <span class="freq-label" data-range="presence">Presence<br><small>4-6kHz</small></span>
                    <span class="freq-label" data-range="brilliance">Brilliance<br><small>6-20kHz</small></span>
                  </div>
                `
                    : ""
                }
              </div>
            `
                : ""
            }
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
    }

    if (this.spectrumCanvas) {
      this.spectrumCtx = this.spectrumCanvas.getContext("2d");
      // Set canvas dimensions
      this.resizeSpectrumCanvas();
      // Initialize spectrum analyzer immediately if enabled
      if (this.config.enableSpectrum) {
        this.initializeSpectrumDisplay();
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
      console.log("[ReplayGain] Container initial display:", this.replayGainContainer.style.display);
    }
  }

  private setupEventListeners(): void {
    // Handle window resize for spectrum canvas
    this.resizeHandler = () => {
      if (this.spectrumCanvas && this.config.enableSpectrum) {
        this.resizeSpectrumCanvas();
      }
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

      // Position the modal above the gear button
      const buttonRect = this.eqConfigBtn.getBoundingClientRect();
      const modalWidth = 450; // Match CSS width
      const modalHeight = 350; // Approximate height

      // Calculate position - center above the button
      let left = buttonRect.left + buttonRect.width / 2 - modalWidth / 2;
      let top = buttonRect.top - modalHeight - 10; // 10px gap

      // Keep modal within viewport
      const padding = 10;
      if (left < padding) left = padding;
      if (left + modalWidth > window.innerWidth - padding) {
        left = window.innerWidth - modalWidth - padding;
      }

      // If not enough space above, show below
      if (top < padding) {
        top = buttonRect.bottom + 10;
      }

      // Apply positioning
      this.eqModal.style.left = `${left}px`;
      this.eqModal.style.top = `${top}px`;

      console.log("[EQ Debug] Modal positioned at:", { left, top, buttonRect });

      // Show backdrop and modal
      this.eqBackdrop.classList.add("visible");
      this.eqModal.classList.add("visible");

      console.log("[EQ Debug] Modal classes after show:", {
        modal: this.eqModal.className,
        backdrop: this.eqBackdrop.className,
      });

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

    const table = document.createElement("table");
    table.innerHTML = `
      <thead>
        <tr>
          <th>Enabled</th>
          <th>Frequency (Hz)</th>
          <th>Q</th>
          <th>Gain (dB)</th>
        </tr>
      </thead>
      <tbody>
        ${this.currentFilterParams
          .map(
            (filter, index) => `
          <tr>
            <td><input type="checkbox" data-index="${index}" class="eq-enabled" ${filter.enabled ? "checked" : ""}></td>
            <td><input type="number" data-index="${index}" class="eq-frequency" value="${filter.frequency.toFixed(1)}" step="1"></td>
            <td><input type="number" data-index="${index}" class="eq-q" value="${filter.q.toFixed(2)}" step="0.1"></td>
            <td><input type="number" data-index="${index}" class="eq-gain" value="${filter.gain.toFixed(2)}" step="0.1"></td>
          </tr>
        `,
          )
          .join("")}
      </tbody>
    `;

    this.eqTableContainer.innerHTML = "";
    eqSection.appendChild(header);
    eqSection.appendChild(table);
    this.eqTableContainer.appendChild(eqSection);

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
    const target = e.target as HTMLInputElement;
    const index = parseInt(target.dataset.index || "0", 10);
    const type = target.className.replace("eq-", "");

    if (isNaN(index) || !this.currentFilterParams[index]) return;

    let value: number | boolean;
    if (target.type === "checkbox") {
      value = target.checked;
    } else {
      value = parseFloat(target.value);
      if (isNaN(value)) return;
    }

    // Update the filter parameter - using type assertion for dynamic property access
    (
      this.currentFilterParams[index] as unknown as Record<
        string,
        number | boolean
      >
    )[type] = value;

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
    this.setStatus("Loading audio...");
    this.setListenButtonEnabled(false);

    try {
      // Use Camilla backend to load the audio file
      if (this.camillaManager) {
        await this.camillaManager.loadAudioFilePath(filePath);
        this.currentAudioPath = filePath;
        this.setStatus("Audio ready");
        this.setListenButtonEnabled(true);
        this.showAudioStatus(true);

        // Analyze ReplayGain for demo track
        await this.analyzeReplayGain(filePath);
      } else {
        throw new Error("Camilla audio manager not initialized");
      }
    } catch (error) {
      this.setStatus("Failed to load audio");
      this.callbacks.onError?.("Failed to load audio file: " + error);
      throw error;
    }
  }

  private clearAudio(): void {
    this.stop();
    this.stopSpectrumAnalysis();
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
  updateFilterParams(filterParams: Partial<FilterParam>[]): void {
    this.currentFilterParams = filterParams.map((p) => ({
      ...p,
      frequency: p.frequency || 0,
      q: p.q || 1,
      gain: p.gain || 0,
      enabled: p.enabled ?? true,
    }));

    // Recalculate and apply filters
    this.setupEQFilters();

    // If playing, reconnect audio chain to apply changes immediately
    if (this.isAudioPlaying && this.audioSource) {
      this.connectAudioChain();
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

    // Apply EQ changes during playback or pause
    if (this.audioSource) {
      if (this.isAudioPlaying) {
        // Audio is actively playing - reconnect chain directly
        this.connectAudioChain();
      } else if (this.isAudioPaused) {
        // Audio is paused - need to restart to rebuild the audio chain
        // Save current time position before restart
        const currentTime = this.getCurrentTimeWhilePaused();
        this.restartFromPosition(currentTime);
      }
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

  // Resize spectrum canvas to fit container
  private resizeSpectrumCanvas(): void {
    if (!this.spectrumCanvas) return;

    const container = this.spectrumCanvas.parentElement;
    if (!container) return;

    // Get the actual width of the container
    const rect = container.getBoundingClientRect();
    const width = Math.max(rect.width || container.clientWidth || 400, 200);
    const height = 52; // Fixed height matching CSS

    // Set canvas dimensions
    this.spectrumCanvas.width = width;
    this.spectrumCanvas.height = height;

    // Redraw idle spectrum if not playing
    if (!this.isAudioPlaying && this.config.enableSpectrum) {
      this.drawIdleSpectrum();
    }
  }

  // Initialize spectrum display even when not playing
  private initializeSpectrumDisplay(): void {
    if (!this.spectrumCanvas || !this.spectrumCtx) return;

    const frequencyAnalyzer = this.container.querySelector(
      ".frequency-analyzer",
    ) as HTMLElement;
    if (frequencyAnalyzer) {
      frequencyAnalyzer.style.display = "flex";
    }

    // Draw initial empty spectrum
    this.drawIdleSpectrum();
  }

  private drawIdleSpectrum(): void {
    if (!this.spectrumCanvas || !this.spectrumCtx) return;

    const width = this.spectrumCanvas.width;
    const height = this.spectrumCanvas.height;

    // Detect color scheme
    const isDarkMode =
      window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: dark)").matches;

    // Properly clear the canvas first
    this.spectrumCtx.clearRect(0, 0, width, height);

    // Fill background with theme-appropriate color
    this.spectrumCtx.fillStyle = isDarkMode
      ? "rgb(0, 0, 0)"
      : "rgb(255, 255, 255)";
    this.spectrumCtx.fillRect(0, 0, width, height);

    // Draw a subtle baseline to indicate the spectrum analyzer is ready
    const barsCount = Math.min(width / 2, 256);
    const barWidth = width / barsCount;

    for (let i = 0; i < barsCount; i++) {
      const baseHeight = 2; // Minimal height for idle state

      if (isDarkMode) {
        this.spectrumCtx.fillStyle = "rgba(88, 101, 242, 0.3)"; // Subtle blue
      } else {
        this.spectrumCtx.fillStyle = "rgba(0, 123, 255, 0.3)"; // Subtle blue
      }

      const x = i * barWidth;
      this.spectrumCtx.fillRect(
        x,
        height - baseHeight,
        barWidth - 1,
        baseHeight,
      );
    }
  }

  // Spectrum Analyzer
  private startSpectrumAnalysis(): void {
    if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) return;

    const frequencyAnalyzer = this.container.querySelector(
      ".frequency-analyzer",
    ) as HTMLElement;
    if (frequencyAnalyzer) {
      frequencyAnalyzer.style.display = "flex";
    }

    if (this.spectrumAnimationFrame) return; // Animation already running

    const dataArray = new Uint8Array(this.analyserNode.frequencyBinCount);

    const draw = () => {
      if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) {
        this.spectrumAnimationFrame = null;
        return;
      }

      const width = this.spectrumCanvas.width;
      const height = this.spectrumCanvas.height;

      // Detect color scheme and set appropriate colors
      const isDarkMode =
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: dark)").matches;

      // Properly clear the canvas first
      this.spectrumCtx.clearRect(0, 0, width, height);

      // Set background color based on theme
      this.spectrumCtx.fillStyle = isDarkMode
        ? "rgb(0, 0, 0)"
        : "rgb(255, 255, 255)";
      this.spectrumCtx.fillRect(0, 0, width, height);

      if (this.isAudioPlaying) {
        this.analyserNode.getByteFrequencyData(dataArray);

        // Use logarithmic frequency mapping (20Hz - 20kHz)
        const minFreq = 20;
        const maxFreq = 20000;
        const sampleRate = this.audioContext!.sampleRate;
        const nyquist = sampleRate / 2;
        const barsCount = Math.min(width / 2, 256); // Limit bars for performance
        const barWidth = width / barsCount;

        for (let i = 0; i < barsCount; i++) {
          // Calculate logarithmic frequency for this bar
          const logMin = Math.log10(minFreq);
          const logMax = Math.log10(maxFreq);
          const logFreq = logMin + (logMax - logMin) * (i / barsCount);
          const freq = Math.pow(10, logFreq);

          // Map frequency to FFT bin
          const binIndex = Math.round((freq / nyquist) * dataArray.length);
          const clampedBin = Math.min(binIndex, dataArray.length - 1);

          // Get magnitude and apply some smoothing by averaging nearby bins
          let magnitude = 0;
          const smoothingRange = Math.max(
            1,
            Math.floor(dataArray.length / barsCount / 2),
          );
          let count = 0;

          for (
            let j = Math.max(0, clampedBin - smoothingRange);
            j <= Math.min(dataArray.length - 1, clampedBin + smoothingRange);
            j++
          ) {
            magnitude += dataArray[j];
            count++;
          }
          magnitude = count > 0 ? magnitude / count : 0;

          const barHeight = (magnitude / 255) * height * 0.9; // Use 90% of height for better visuals

          // Use different colors based on theme and frequency
          if (isDarkMode) {
            // Dark mode: bright colors with frequency-based hues
            const hueShift = (i / barsCount) * 60; // 0-60 degrees (red to yellow)
            const intensity = Math.floor((barHeight / height) * 155 + 100);
            this.spectrumCtx.fillStyle = `hsl(${hueShift}, 80%, ${Math.min((intensity / 255) * 70 + 30, 90)}%)`;
          } else {
            // Light mode: darker colors with frequency-based variation
            const hueShift = (i / barsCount) * 240; // 0-240 degrees (red to blue)
            const saturation = 70 + (barHeight / height) * 30; // 70-100%
            const lightness = Math.max(20, 60 - (barHeight / height) * 40); // 60-20%
            this.spectrumCtx.fillStyle = `hsl(${hueShift}, ${saturation}%, ${lightness}%)`;
          }

          const x = i * barWidth;
          this.spectrumCtx.fillRect(
            x,
            height - barHeight,
            barWidth - 1,
            barHeight,
          );
        }
      } else {
        // When not playing, show idle spectrum
        this.drawIdleSpectrum();
      }

      this.spectrumAnimationFrame = requestAnimationFrame(draw);
    };

    draw();
  }

  private stopSpectrumAnalysis(): void {
    if (this.spectrumAnimationFrame) {
      cancelAnimationFrame(this.spectrumAnimationFrame);
      this.spectrumAnimationFrame = null;
    }

    // Keep spectrum analyzer visible but show idle state
    if (this.config.enableSpectrum) {
      this.drawIdleSpectrum();
    }
  }

  // Position Updates
  private startPositionUpdates(): void {
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

    if (!this.camillaManager) {
      throw new Error("Camilla audio manager not initialized");
    }

    try {
      // Convert current filter params to the format Camilla expects
      const filters = this.eqEnabled
        ? this.currentFilterParams
            .filter((p) => p.enabled)
            .map((p) => ({
              frequency: p.frequency,
              q: p.q,
              gain: p.gain,
            }))
        : [];

      await this.camillaManager.play(
        filters,
        this.outputDeviceId === "default" ? undefined : this.outputDeviceId,
      );
      this.callbacks.onPlay?.();
      console.log("Audio playback started successfully via Camilla backend");
    } catch (error) {
      console.error("Error during audio playback:", error);
      this.callbacks.onError?.("Playback failed: " + error);
      throw error;
    }
  }

  async pause(): Promise<void> {
    if (!this.camillaManager) {
      console.warn("Camilla audio manager not initialized");
      return;
    }

    try {
      await this.camillaManager.pause();
      console.log("Audio playback paused via Camilla backend");
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
    if (!this.camillaManager) {
      console.warn("Camilla audio manager not initialized");
      return;
    }

    try {
      await this.camillaManager.resume();
      console.log("Audio playback resumed via Camilla backend");
    } catch (error) {
      console.error("Error resuming playback:", error);
      this.callbacks.onError?.("Failed to resume: " + error);
    }
  }

  async stop(): Promise<void> {
    if (!this.camillaManager) {
      console.warn("Camilla audio manager not initialized");
      return;
    }

    try {
      await this.camillaManager.stop();
      this.callbacks.onStop?.();
      console.log("Audio playback stopped via Camilla backend");
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
      // Show loading state
      if (this.replayGainContainer) {
        console.log("[ReplayGain] Showing loading state");
        this.replayGainContainer.style.display = "block";
        const gainElement =
          this.replayGainContainer.querySelector(".replay-gain-value");
        const peakElement =
          this.replayGainContainer.querySelector(".replay-peak-value");
        if (gainElement) gainElement.textContent = "...";
        if (peakElement) peakElement.textContent = "...";
      } else {
        console.error("[ReplayGain] Container not found during loading state");
      }

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
    console.log(`[ReplayGain] Updating display - Gain: ${gain}, Peak: ${peak}`);
    console.log(`[ReplayGain] Container found:`, !!this.replayGainContainer);
    
    if (!this.replayGainContainer) {
      console.error("[ReplayGain] Container not found!");
      return;
    }

    console.log(`[ReplayGain] Container HTML:`, this.replayGainContainer.innerHTML);
    console.log(`[ReplayGain] Container current display:`, this.replayGainContainer.style.display);

    const gainElement =
      this.replayGainContainer.querySelector(".replay-gain-value");
    const peakElement =
      this.replayGainContainer.querySelector(".replay-peak-value");

    console.log(`[ReplayGain] Elements found - Gain:`, !!gainElement, "Peak:", !!peakElement);
    console.log(`[ReplayGain] Gain element:`, gainElement);
    console.log(`[ReplayGain] Peak element:`, peakElement);

    if (gainElement && peakElement) {
      // Format gain with sign and 2 decimal places
      const gainText =
        gain >= 0 ? `+${gain.toFixed(2)} dB` : `${gain.toFixed(2)} dB`;
      gainElement.textContent = gainText;

      // Format peak with 6 decimal places (matching backend precision)
      peakElement.textContent = peak.toFixed(6);

      console.log(`[ReplayGain] Text content set - Gain element text: "${gainElement.textContent}", Peak element text: "${peakElement.textContent}"`);
      console.log(`[ReplayGain] Display updated - Gain: "${gainText}", Peak: "${peak.toFixed(6)}"`);

      // Show the container
      this.replayGainContainer.style.display = "block";
      console.log(`[ReplayGain] Container display set to: ${this.replayGainContainer.style.display}`);
      console.log(`[ReplayGain] Container is visible:`, this.replayGainContainer.offsetHeight > 0);
    } else {
      console.error("[ReplayGain] Could not find gain or peak element within container");
    }
  }

  // Cleanup
  destroy(): void {
    this.stop();
    this.stopSpectrumAnalysis();

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
}
