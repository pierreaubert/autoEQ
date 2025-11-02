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
import {
  VisualEQConfig,
  type ExtendedFilterParam,
  type FilterParam,
  FILTER_TYPES
} from "./visual-eq-config";

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

// Re-export EQ types from visual-eq-config for backward compatibility
export type { FilterParam, ExtendedFilterParam } from "./visual-eq-config";
export { FILTER_TYPES } from "./visual-eq-config";

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
  private gainNode: GainNode | null = null;
  private isAudioPlaying: boolean = false;
  private isAudioPaused: boolean = false;
  private audioStartTime: number = 0;
  private audioPauseTime: number = 0;
  private audioAnimationFrame: number | null = null;
  private currentAudioPath: string | null = null; // Track current audio file path for Rust backend

  // Visual EQ Configuration
  private visualEQConfig: VisualEQConfig | null = null;
  private eqEnabled: boolean = true; // Backward compatibility property
  private eqFilters: any[] = []; // Backward compatibility property
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
  private loudnessPollingActive: boolean = false;
  private replayGainDisplay: HTMLElement | null = null;
  private peakDisplay: HTMLElement | null = null;

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
  private playBtn: HTMLButtonElement | null = null;
  private pauseBtn: HTMLButtonElement | null = null;
  private stopBtn: HTMLButtonElement | null = null;
  private eqOnBtn: HTMLButtonElement | null = null;
  private eqOffBtn: HTMLButtonElement | null = null;
  private eqConfigBtn: HTMLButtonElement | null = null;
  private eqMiniCanvas: HTMLCanvasElement | null = null;
  private statusText: HTMLElement | null = null;
  private positionText: HTMLElement | null = null;
  private durationText: HTMLElement | null = null;
  private progressFill: HTMLElement | null = null;

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

  // Streaming manager
  private streamingManager: StreamingManager;

  // Spectrum analyzer component
  private spectrumAnalyzer: SpectrumAnalyzerComponent | null = null;

  // Event handlers
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

    // Initialize streaming manager
    this.streamingManager = new StreamingManager({
      onStateChange: (state) => this.handleStateChange(state),
      onPositionUpdate: (position, duration) =>
        this.handlePositionUpdate(position, duration ?? 0),
      onError: (error) => this.handleError(error),
      onFileLoaded: (info) => this.handleFileLoaded(info),
    });

    this.init();
  }

  private async init(): Promise<void> {
    try {
      await this.setupAudioContext();
      this.createUI();
      
      // Initialize Visual EQ Configuration after UI is created
      if (this.config.enableEQ) {
        // Get mini canvas after UI is created
        const eqMiniCanvas = this.container.querySelector('.eq-mini-canvas') as HTMLCanvasElement | null;
        
        this.visualEQConfig = new VisualEQConfig(
          this.container,
          this.instanceId,
          this.streamingManager,
          {
            onFilterParamsChange: (filterParams) => {
              // Sync local state without calling updateFilterParams to avoid recursion
              // The VisualEQConfig has already handled the update
            },
            onEQToggle: (enabled) => {
              // Sync local state
              this.eqEnabled = enabled;
              // Notify external callback
              this.callbacks.onEQToggle?.(enabled);
            },
            onAutoGainChange: (enabled) => {
              this.autoGain = enabled;
              console.log('[AudioPlayer] Auto gain changed to:', enabled);
            },
            onLoudnessCompensationChange: (enabled) => {
              this.loudnessCompensation = enabled;
              console.log('[AudioPlayer] Loudness compensation changed to:', enabled);
            },
            onSplAmplitudeChange: (amplitude) => {
              this.splAmplitude = amplitude;
              console.log('[AudioPlayer] SPL amplitude changed to:', amplitude, 'dB');
            },
            getAutoGain: () => this.autoGain,
            getLoudnessCompensation: () => this.loudnessCompensation,
            getSplAmplitude: () => this.splAmplitude,
          },
          eqMiniCanvas
        );
      }
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
  }

  private handlePositionUpdate(position: number, duration: number): void {
    if (this.positionText) {
      this.positionText.textContent = this.formatTime(position);
    }
    if (this.durationText) {
      this.durationText.textContent = this.formatTime(duration);
    }
    if (this.progressFill && duration > 0) {
      const progress = (position / duration) * 100;
      this.progressFill.style.width = `${progress}%`;
    }
  }

  private handleError(error: string): void {
    console.error("[AudioPlayer] Backend error:", error);
    this.callbacks.onError?.(error);
    this.setStatus("Error: " + error);
  }

  private handleFileLoaded(info: AudioFileInfo): void {
    console.log("[AudioPlayer] File loaded:", info);
    this.updateAudioInfo();
    this.showAudioStatus(true);
    this.setListenButtonEnabled(true);
    this.setStatus("Ready");
  }

  private async setupAudioContext(): Promise<void> {
    this.audioContext = new AudioContext();
    this.gainNode = this.audioContext.createGain();
    this.gainNode.connect(this.audioContext.destination);
  }

  private createUI(): void {
    const selectId = `demo-audio-select-${this.instanceId}`;

    const html = `
<div class="audio-player">
  <div class="audio-control-row">

    <!-- Block: Track Selection (compact) -->
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
        <button type="button" class="file-upload-btn">Load a file: 📁</button>
      </div>
    </div>

    <!-- Block: Playback Controls (vertical) -->
    <div class="audio-playback-controls">
      <div class="audio-status-display">
        <div class="status-display-row">
          <div class="audio-status-text">No audio selected</div>
          <div class="audio-time-display">
            <span class="audio-position">--:--</span> / <span class="audio-duration">--:--</span>
          </div>
        </div>
        <div class="audio-progress-row">
          <div class="audio-progress-bar">
            <div class="audio-progress-fill"></div>
          </div>
        </div>
      </div>
      <div class="playback-controls-row">
        <button type="button" class="listen-button" disabled>▶️</button>
        <button type="button" class="pause-button" disabled>⏸️</button>
        <button type="button" class="stop-button" disabled>⏹️</button>
      </div>
    </div>

    <!-- Block: Frequency Analyzer -->
    <div class="frequency-analyzer">
      <canvas class="spectrum-canvas" width="520" height="72"></canvas>
    </div>

    <!-- Block: EQ Controls -->
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

    <!-- Block: Audio Metrics (vertical) -->
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
          <div class="metric-label">ReplayGain</div>
          <div class="metric-row">
            <span id="metrics-replay-gain" class="metric-value">--</span>
            <span class="metric-separator">/</span>
            <span id="metrics-peak" class="metric-value">--</span>
          </div>
        </div>
      </div>
    </div>

  </div>
</div>

    `;

    this.container.innerHTML = html;
    this.cacheUIElements();
  }

  private cacheUIElements(): void {
    this.demoSelect = this.container.querySelector(".demo-audio-select");
    this.playBtn = this.container.querySelector(".listen-button");
    this.pauseBtn = this.container.querySelector(".pause-button");
    this.stopBtn = this.container.querySelector(".stop-button");
    this.eqOnBtn = this.container.querySelector(".eq-on-btn");
    this.eqOffBtn = this.container.querySelector(".eq-off-btn");
    this.eqConfigBtn = this.container.querySelector(".eq-config-btn");
    this.eqMiniCanvas = this.container.querySelector(".eq-mini-canvas");

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
    this.replayGainDisplay = this.container.querySelector("#metrics-replay-gain");
    this.peakDisplay = this.container.querySelector("#metrics-peak");

    if (this.spectrumCanvas) {
      this.spectrumCtx = this.spectrumCanvas.getContext("2d");
      // Initialize spectrum analyzer component
      if (this.config.enableSpectrum) {
        // Detect system color scheme
        const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        const colorScheme = prefersDark ? "dark" : "light";
        
        this.spectrumAnalyzer = new SpectrumAnalyzerComponent({
          canvas: this.spectrumCanvas,
          pollInterval: 100,
          minFreq: 20,
          maxFreq: 20000,
          dbRange: 60,
          colorScheme: colorScheme,
          showLabels: true,
          showGrid: true,
        });
      }
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
    this.playBtn?.addEventListener("click", () => {
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
    if (this.eqOnBtn && this.visualEQConfig) {
      this.eqOnBtn.addEventListener("click", () => {
        this.visualEQConfig!.setEQEnabled(true);
        this.updateEQButtonStates(true);
        this.callbacks.onEQToggle?.(true);
      });
    }

    if (this.eqOffBtn && this.visualEQConfig) {
      this.eqOffBtn.addEventListener("click", () => {
        this.visualEQConfig!.setEQEnabled(false);
        this.updateEQButtonStates(false);
        this.callbacks.onEQToggle?.(false);
      });
    }

    if (this.eqConfigBtn && this.visualEQConfig) {
      this.eqConfigBtn.addEventListener("click", () => {
        this.visualEQConfig!.openEQModal(this.eqConfigBtn!);
      });
    }
  }

  // ===== AUDIO LOADING METHODS =====

  async loadDemoTrack(trackName: string): Promise<void> {
    const trackPath = this.config.demoTracks?.[trackName];
    if (!trackPath) {
      throw new Error(`Demo track "${trackName}" not found`);
    }

    try {
      // Resolve the resource path
      const resolvedPath = await resolveResource(trackPath);
      await this.loadAudioFilePath(resolvedPath);
    } catch (error) {
      console.error(`Failed to load demo track "${trackName}":`, error);
      this.callbacks.onError?.(`Failed to load demo track: ${error}`);
    }
  }

  async loadAudioFilePath(filePath: string): Promise<void> {
    try {
      this.currentAudioPath = filePath;
      await this.streamingManager.loadAudioFilePath(filePath);
      this.setStatus("Loading...");
    } catch (error) {
      console.error("Failed to load audio file:", error);
      this.callbacks.onError?.(`Failed to load audio file: ${error}`);
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
  }

  // ===== UI HELPER METHODS =====

  private formatTrackName(key: string): string {
    return key
      .replace(/_/g, " ")
      .replace(/\b\w/g, (l) => l.toUpperCase());
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

  private setStatus(status: string): void {
    if (this.statusText) {
      this.statusText.textContent = status;
    }
  }

  private setListenButtonEnabled(enabled: boolean): void {
    if (this.playBtn) {
      this.playBtn.disabled = !enabled;
    }
  }

  private showAudioStatus(show: boolean): void {
    // Show/hide time display and progress bar
    const timeDisplay = this.container.querySelector(".audio-time-display");
    const progressBar = this.container.querySelector(".audio-progress-row");

    if (timeDisplay) {
      (timeDisplay as HTMLElement).style.display = show ? "block" : "none";
    }
    if (progressBar) {
      (progressBar as HTMLElement).style.display = show ? "block" : "none";
    }
  }

  // ===== PLAYBACK METHODS =====

  async play(): Promise<void> {
    try {
      if (!this.currentAudioPath) {
        this.callbacks.onError?.("No audio file selected");
        return;
      }

      // Get filter parameters from VisualEQConfig if available
      let filters: Array<{frequency: number; q: number; gain: number}> = [];
      if (this.visualEQConfig && this.visualEQConfig.isEQEnabled()) {
        const filterParams = this.visualEQConfig.getFilterParams();
        filters = filterParams
          .filter((p) => p.enabled)
          .map((p) => ({
            frequency: p.frequency,
            q: p.q,
            gain: p.gain,
          }));
      }

      await this.streamingManager.play(filters);

      this.isAudioPlaying = true;
      this.isAudioPaused = false;
      this.audioStartTime = Date.now();

      // Start spectrum analyzer
      if (this.spectrumAnalyzer) {
        await this.spectrumAnalyzer.start();
      }

      // Start loudness monitoring
      console.log('[AudioPlayer] Enabling and starting loudness monitoring...');
      await this.streamingManager.enableLoudnessMonitoring();
      this.streamingManager.startLoudnessPolling(100, (loudnessInfo) => {
        this.updateLoudnessDisplay(loudnessInfo);
      });

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Playing");

      this.callbacks.onPlay?.();
    } catch (error) {
      console.error("Failed to play audio:", error);
      this.callbacks.onError?.("Failed to play audio: " + error);
    }
  }

  async pause(): Promise<void> {
    try {
      await this.streamingManager.pause();
      this.isAudioPlaying = false;
      this.isAudioPaused = true;
      this.audioPauseTime = Date.now();

      // Stop loudness monitoring
      this.streamingManager.stopLoudnessPolling();

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Paused");

      this.callbacks.onStop?.();
    } catch (error) {
      console.error("Failed to pause audio:", error);
      this.callbacks.onError?.("Failed to pause audio: " + error);
    }
  }

  async stop(): Promise<void> {
    try {
      await this.streamingManager.stop();
      this.isAudioPlaying = false;
      this.isAudioPaused = false;

      // Stop spectrum analyzer
      if (this.spectrumAnalyzer) {
        await this.spectrumAnalyzer.stop();
      }

      // Stop loudness monitoring
      this.streamingManager.stopLoudnessPolling();
      await this.streamingManager.disableLoudnessMonitoring();
      this.updateLoudnessDisplay(null); // Reset display

      // Reset position display
      if (this.positionText) {
        this.positionText.textContent = "--:--";
      }
      if (this.progressFill) {
        this.progressFill.style.width = "0%";
      }

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Stopped");

      this.callbacks.onStop?.();
    } catch (error) {
      console.error("Failed to stop audio:", error);
      this.callbacks.onError?.("Failed to stop audio: " + error);
    }
  }

  async resume(): Promise<void> {
    try {
      await this.streamingManager.resume();
      this.isAudioPlaying = true;
      this.isAudioPaused = false;

      // Restart loudness monitoring
      await this.streamingManager.enableLoudnessMonitoring();
      this.streamingManager.startLoudnessPolling(100, (loudnessInfo) => {
        this.updateLoudnessDisplay(loudnessInfo);
      });

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Playing");

      this.callbacks.onPlay?.();
    } catch (error) {
      console.error("Failed to resume audio:", error);
      this.callbacks.onError?.("Failed to resume audio: " + error);
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
      }, 300);
    } else if (this.pauseClickCount === 2) {
      // Second click - stop
      this.stop();
      this.pauseClickCount = 0;

      // Clear the timer
      if (this.pauseClickTimer) {
        clearTimeout(this.pauseClickTimer);
        this.pauseClickTimer = null;
      }
    }
  }

  private updatePlaybackUI(): void {
    const isPlaying = this.isAudioPlaying;
    const isPaused = this.isAudioPaused;

    // Update button states based on playback status
    if (this.playBtn) {
      this.playBtn.disabled = isPlaying;
    }

    if (this.pauseBtn) {
      this.pauseBtn.disabled = !isPlaying;
    }

    if (this.stopBtn) {
      this.stopBtn.disabled = !isPlaying && !isPaused;
    }
  }

  private updateEQButtonStates(enabled: boolean): void {
    if (this.eqOnBtn) {
      if (enabled) {
        this.eqOnBtn.classList.add("active");
      } else {
        this.eqOnBtn.classList.remove("active");
      }
    }

    if (this.eqOffBtn) {
      if (enabled) {
        this.eqOffBtn.classList.remove("active");
      } else {
        this.eqOffBtn.classList.add("active");
      }
    }
  }

  // ===== LOUDNESS MONITORING =====

  private updateLoudnessDisplay(loudnessInfo: {momentary_lufs: number; shortterm_lufs: number; peak: number} | null): void {
    console.log('[AudioPlayer] Updating loudness display:', loudnessInfo);
    console.log('[AudioPlayer] Display elements:', {
      momentary: this.loudnessDisplayMomentary,
      shortterm: this.loudnessDisplayShortterm
    });
    
    if (!loudnessInfo) {
      // Reset to -∞ when no data
      if (this.loudnessDisplayMomentary) {
        this.loudnessDisplayMomentary.textContent = '-∞';
      }
      if (this.loudnessDisplayShortterm) {
        this.loudnessDisplayShortterm.textContent = '-∞';
      }
      if (this.peakDisplay) {
        this.peakDisplay.textContent = '--';
      }
      if (this.replayGainDisplay) {
        this.replayGainDisplay.textContent = '--';
      }
      return;
    }

    // Update momentary LUFS (M)
    if (this.loudnessDisplayMomentary) {
      const mValue = loudnessInfo.momentary_lufs;
      const text = (mValue !== null && isFinite(mValue)) ? mValue.toFixed(1) : '-∞';
      console.log('[AudioPlayer] Setting momentary to:', text);
      this.loudnessDisplayMomentary.textContent = text;
    }

    // Update short-term LUFS (S)
    if (this.loudnessDisplayShortterm) {
      const sValue = loudnessInfo.shortterm_lufs;
      const text = (sValue !== null && isFinite(sValue)) ? sValue.toFixed(1) : '-∞';
      console.log('[AudioPlayer] Setting shortterm to:', text);
      this.loudnessDisplayShortterm.textContent = text;
    }

    // Update peak value
    if (this.peakDisplay) {
      const peakValue = loudnessInfo.peak;
      const text = (peakValue !== null && peakValue !== undefined && isFinite(peakValue)) 
        ? peakValue.toFixed(2) 
        : '--';
      console.log('[AudioPlayer] Setting peak to:', text);
      this.peakDisplay.textContent = text;
    }

    // TODO: Update ReplayGain when backend provides it
    // For now, keep it as '--'
  }

  // ===== PUBLIC API METHODS =====

  getCurrentTrack(): string | null {
    return this.demoSelect?.value || null;
  }

  isPlaying(): boolean {
    return this.isAudioPlaying;
  }

  // ===== EQ FILTER MANAGEMENT - delegate to VisualEQConfig =====

  updateFilterParams(filterParams: Partial<ExtendedFilterParam>[]): void {
    if (this.visualEQConfig) {
      this.visualEQConfig.updateFilterParams(filterParams);
    }
  }

  // Clear all EQ filters
  clearEQFilters(): void {
    if (this.visualEQConfig) {
      this.visualEQConfig.clearEQFilters();
    }
  }

  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled; // Update local property for backward compatibility
    if (this.visualEQConfig) {
      this.visualEQConfig.setEQEnabled(enabled);
    }
  }

  isEQEnabled(): boolean {
    // For backward compatibility, always return the local property
    // since tests might set it directly
    return this.eqEnabled;
  }

  getFilterParams(): ExtendedFilterParam[] {
    return this.visualEQConfig?.getFilterParams() ?? [];
  }

  // ===== OUTPUT DEVICE MANAGEMENT =====

  setOutputDevice(deviceId: string): void {
    this.outputDeviceId = deviceId;
    // Note: Actual device routing would need to be implemented
    // if using Web Audio API directly. With streaming manager,
    // the backend handles device selection.
  }

  // ===== CLEANUP =====

  destroy(): void {
    // Stop audio
    this.stop();

    // Destroy VisualEQConfig
    if (this.visualEQConfig) {
      this.visualEQConfig.destroy();
      this.visualEQConfig = null;
    }

    // Remove event listeners
    if (this.resizeHandler) {
      window.removeEventListener("resize", this.resizeHandler);
    }

    // Clear audio context
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }
  }
}
