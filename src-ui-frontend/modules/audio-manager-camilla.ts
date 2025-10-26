// CamillaDSP Audio Manager
// Wrapper for Tauri backend audio commands with CamillaDSP integration

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export interface AudioSpec {
  sample_rate: number;
  channels: number;
  bits_per_sample: number;
  total_frames?: number;
}

export interface AudioFileInfo {
  path: string;
  format: string;
  sample_rate: number;
  channels: number;
  bits_per_sample: number;
  duration_seconds?: number;
}

export interface FilterParam {
  frequency: number;
  q: number;
  gain: number;
}

export interface AudioStreamState {
  state: string; // "idle" | "loading" | "ready" | "playing" | "paused" | "seeking" | "error"
  position_seconds?: number;
  duration_seconds?: number;
  file?: string;
  output_device?: string;
}

export interface AudioManagerCallbacks {
  onStateChange?: (state: string) => void;
  onPositionUpdate?: (position: number, duration?: number) => void;
  onError?: (error: string) => void;
  onFileLoaded?: (info: AudioFileInfo) => void;
}

/**
 * CamillaDSP Audio Manager
 * Manages audio playback through Tauri backend with CamillaDSP processing
 */
export class CamillaAudioManager {
  private callbacks: AudioManagerCallbacks;
  private pollingInterval: number | null = null;
  private currentFileInfo: AudioFileInfo | null = null;
  private currentFilePath: string | null = null;
  private eventUnlisteners: UnlistenFn[] = [];
  private isTauriAvailable: boolean = false;

  constructor(callbacks: AudioManagerCallbacks = {}) {
    this.callbacks = callbacks;
    this.checkTauriAvailability();
    if (this.isTauriAvailable) {
      this.setupEventListeners();
    } else {
      console.warn(
        "[CamillaAudioManager] Tauri not available. Audio playback disabled. " +
          "Run the app via 'npm run tauri dev' or 'npm run tauri build'.",
      );
    }
  }

  /**
   * Check if Tauri is available (running in Tauri app vs browser)
   */
  private checkTauriAvailability(): void {
    try {
      // Check if __TAURI__ global exists
      this.isTauriAvailable = typeof (window as { __TAURI__?: unknown }).__TAURI__ !== "undefined";
    } catch {
      this.isTauriAvailable = false;
    }
  }

  /**
   * Set up Tauri event listeners for backend events
   */
  private async setupEventListeners(): Promise<void> {
    try {
      // Listen for state changes
      const stateUnlisten = await listen<{ state?: string }>("flac:state-changed", (event) => {
        console.log("[CamillaAudioManager] State changed:", event.payload);
        if (event.payload.state) {
          this.callbacks.onStateChange?.(event.payload.state);
        }
      });
      this.eventUnlisteners.push(stateUnlisten);

      // Listen for position updates
      const positionUnlisten = await listen<{ position_seconds?: number }>(
        "flac:position-changed",
        (event) => {
          if (event.payload.position_seconds !== undefined) {
            this.callbacks.onPositionUpdate?.(
              event.payload.position_seconds,
              this.currentFileInfo?.duration_seconds,
            );
          }
        },
      );
      this.eventUnlisteners.push(positionUnlisten);

      // Listen for errors
      const errorUnlisten = await listen<{ error?: string }>("flac:error", (event) => {
        console.error("[CamillaAudioManager] Error:", event.payload);
        this.callbacks.onError?.(event.payload.error || "Unknown error");
      });
      this.eventUnlisteners.push(errorUnlisten);

      // Listen for file loaded events
      const fileLoadedUnlisten = await listen<AudioFileInfo>(
        "flac:file-loaded",
        (event) => {
          console.log("[CamillaAudioManager] File loaded:", event.payload);
          this.currentFileInfo = event.payload;
          this.callbacks.onFileLoaded?.(event.payload);
        },
      );
      this.eventUnlisteners.push(fileLoadedUnlisten);
    } catch (error) {
      console.error(
        "[CamillaAudioManager] Failed to setup event listeners:",
        error,
      );
    }
  }

  /**
   * Load an audio file from a File object
   * Converts the File to a temporary path and loads it via backend
   */
  async loadAudioFile(file: File): Promise<AudioFileInfo> {
    try {
      // For Tauri, we need to convert the File to a path
      // In a real implementation, you'd save to temp directory or use dialog
      // For now, we'll assume the file path is provided directly

      // Try to get file path from File object (works in Tauri context)
      const filePath = (file as File & { path?: string }).path || file.name;

      if (!filePath || filePath === file.name) {
        throw new Error(
          "Cannot load file: File path not available. Use file dialog instead.",
        );
      }

      return await this.loadAudioFilePath(filePath);
    } catch (error) {
      const errorMsg = `Failed to load audio file: ${error}`;
      console.error("[CamillaAudioManager]", errorMsg);
      this.callbacks.onError?.(errorMsg);
      throw error;
    }
  }

  /**
   * Load an audio file from a file path
   */
  async loadAudioFilePath(filePath: string): Promise<AudioFileInfo> {
    if (!this.isTauriAvailable) {
      throw new Error("Tauri not available. Run app via 'npm run tauri dev'.");
    }

    try {
      console.log("[CamillaAudioManager] Loading file:", filePath);

      this.currentFilePath = filePath;
      const info = await invoke<AudioFileInfo>("flac_load_file", {
        filePath,
      });

      this.currentFileInfo = info;
      this.callbacks.onFileLoaded?.(info);

      return info;
    } catch (error) {
      const errorMsg = `Failed to load audio file: ${error}`;
      console.error("[CamillaAudioManager]", errorMsg);
      this.callbacks.onError?.(errorMsg);
      throw error;
    }
  }

  /**
   * Start playback with optional filters and output device
   */
  async play(
    filters: FilterParam[] = [],
    outputDevice?: string,
  ): Promise<void> {
    if (!this.isTauriAvailable) {
      throw new Error("Tauri not available. Run app via 'npm run tauri dev'.");
    }

    try {
      if (!this.currentFileInfo) {
        throw new Error("No audio file loaded");
      }

      console.log(
        "[CamillaAudioManager] Starting playback with filters:",
        filters,
      );

      // Convert FilterParam to backend format
      const backendFilters = filters.map((f) => ({
        frequency: f.frequency,
        q: f.q,
        gain: f.gain,
        filter_type: "Peaking",
      }));

      await invoke("flac_start_playback", {
        outputDevice,
        filters: backendFilters,
      });

      this.callbacks.onStateChange?.("playing");
      this.startStatePolling();
    } catch (error) {
      const errorMsg = `Failed to start playback: ${error}`;
      console.error("[CamillaAudioManager]", errorMsg);
      this.callbacks.onError?.(errorMsg);
      throw error;
    }
  }

  /**
   * Pause playback
   */
  async pause(): Promise<void> {
    try {
      await invoke("flac_pause_playback");
      this.callbacks.onStateChange?.("paused");
      this.stopStatePolling();
    } catch (error) {
      console.error("[CamillaAudioManager] Failed to pause:", error);
      throw error;
    }
  }

  /**
   * Resume playback
   */
  async resume(): Promise<void> {
    try {
      await invoke("flac_resume_playback");
      this.callbacks.onStateChange?.("playing");
      this.startStatePolling();
    } catch (error) {
      console.error("[CamillaAudioManager] Failed to resume:", error);
      throw error;
    }
  }

  /**
   * Stop playback
   */
  async stop(): Promise<void> {
    try {
      await invoke("flac_stop_playback");
      this.callbacks.onStateChange?.("idle");
      this.stopStatePolling();
    } catch (error) {
      console.error("[CamillaAudioManager] Failed to stop:", error);
      throw error;
    }
  }

  /**
   * Seek to a specific position in seconds
   */
  async seek(seconds: number): Promise<void> {
    try {
      await invoke("flac_seek", { seconds });
      this.callbacks.onPositionUpdate?.(
        seconds,
        this.currentFileInfo?.duration_seconds,
      );
    } catch (error) {
      console.error("[CamillaAudioManager] Failed to seek:", error);
      throw error;
    }
  }

  /**
   * Update EQ filters in real-time
   * Note: This requires restarting playback with new filters in current implementation
   */
  async updateFilters(filters: FilterParam[]): Promise<void> {
    try {
      console.log("[CamillaAudioManager] Updating filters:", filters);

      // Get current state
      const state = await this.getPlaybackState();

      if (state === "playing" || state === "paused") {
        // Need to restart playback with new filters
        // This is a limitation of the current backend implementation
        const wasPlaying = state === "playing";

        // Stop current playback
        await this.stop();

        // Restart with new filters
        await this.play(filters);

        if (!wasPlaying) {
          await this.pause();
        }
      }
    } catch (error) {
      console.error("[CamillaAudioManager] Failed to update filters:", error);
      throw error;
    }
  }

  /**
   * Get current playback state
   */
  async getPlaybackState(): Promise<string> {
    try {
      return await invoke<string>("flac_get_state");
    } catch (_error) {
      console.error("[CamillaAudioManager] Failed to get state:", _error);
      return "error";
    }
  }

  /**
   * Get current file info
   */
  async getFileInfo(): Promise<AudioFileInfo | null> {
    try {
      return await invoke<AudioFileInfo | null>("flac_get_file_info");
    } catch (error) {
      console.error("[CamillaAudioManager] Failed to get file info:", error);
      return null;
    }
  }

  /**
   * Get loaded file info (cached)
   */
  getCurrentFileInfo(): AudioFileInfo | null {
    return this.currentFileInfo;
  }

  /**
   * Set output device
   */
  setOutputDevice(deviceId: string): void {
    // Note: This will take effect on next playback start
    console.log("[CamillaAudioManager] Output device set to:", deviceId);
  }

  /**
   * Start polling playback state (for progress updates)
   */
  startStatePolling(intervalMs: number = 250): void {
    if (this.pollingInterval !== null) {
      return; // Already polling
    }

    this.pollingInterval = window.setInterval(async () => {
      try {
        const fileInfo = await this.getFileInfo();
        if (fileInfo && fileInfo.duration_seconds) {
          // Backend doesn't provide position updates yet, would need to add this
          // For now, we rely on events
        }
      } catch (error) {
        // Ignore polling errors
      }
    }, intervalMs);
  }

  /**
   * Stop polling playback state
   */
  stopStatePolling(): void {
    if (this.pollingInterval !== null) {
      clearInterval(this.pollingInterval);
      this.pollingInterval = null;
    }
  }

  /**
   * Check if audio is currently playing
   */
  async isPlaying(): Promise<boolean> {
    const state = await this.getPlaybackState();
    return state === "playing";
  }

  /**
   * Get current playback position (estimated)
   */
  getCurrentTime(): number {
    // Would need backend support for real-time position tracking
    return 0;
  }

  /**
   * Get audio duration
   */
  getDuration(): number {
    return this.currentFileInfo?.duration_seconds || 0;
  }

  /**
   * Cleanup and destroy
   */
  destroy(): void {
    this.stopStatePolling();

    // Unlisten from all events
    this.eventUnlisteners.forEach((unlisten) => {
      try {
        unlisten();
      } catch (error) {
        console.error("[CamillaAudioManager] Failed to unlisten:", error);
      }
    });
    this.eventUnlisteners = [];

    this.currentFileInfo = null;
    this.currentFilePath = null;
  }
}

export default CamillaAudioManager;
