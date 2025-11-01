// Audio Manager - Rust Backend Integration
// Wraps Tauri commands for audio playback, recording, and EQ control

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// ============================================================================
// Type Definitions
// ============================================================================

export interface FilterParams {
  frequency: number;
  q: number;
  gain: number;
  filter_type?: string;
}

export enum AudioState {
  Idle = "idle",
  Playing = "playing",
  Paused = "paused",
  Recording = "recording",
  Error = "error",
}

export interface AudioStreamState {
  state: AudioState;
  position_seconds: number;
  duration_seconds: number | null;
  current_file: string | null;
  output_device: string | null;
  input_device: string | null;
  sample_rate: number;
  channels: number;
  filters: FilterParams[];
  error_message: string | null;
}

// Event payloads
export interface AudioStateChangedEvent {
  state: string;
  file: string | null;
  output_device: string | null;
  input_device: string | null;
}

export interface AudioPositionUpdateEvent {
  position_seconds: number;
  duration_seconds: number | null;
}

export interface AudioErrorEvent {
  error: string;
}

export interface AudioSignalPeakEvent {
  peak: number;
}

// ============================================================================
// Audio Manager Class
// ============================================================================

export class AudioManagerRust {
  private stateChangeListeners: ((event: AudioStateChangedEvent) => void)[] =
    [];
  private positionUpdateListeners: ((
    event: AudioPositionUpdateEvent,
  ) => void)[] = [];
  private errorListeners: ((event: AudioErrorEvent) => void)[] = [];
  private signalPeakListeners: ((event: AudioSignalPeakEvent) => void)[] = [];

  private stateChangeUnlisten: UnlistenFn | null = null;
  private errorUnlisten: UnlistenFn | null = null;

  constructor() {
    this.setupEventListeners();
  }

  // ============================================================================
  // Event Listeners Setup
  // ============================================================================

  private async setupEventListeners(): Promise<void> {
    try {
      // Listen for state change events
      this.stateChangeUnlisten = await listen<AudioStateChangedEvent>(
        "audio:state-changed",
        (event) => {
          console.log("[AudioManager] State changed:", event.payload);
          this.stateChangeListeners.forEach((listener) =>
            listener(event.payload),
          );
        },
      );

      // Listen for error events
      this.errorUnlisten = await listen<AudioErrorEvent>(
        "audio:error",
        (event) => {
          console.error("[AudioManager] Audio error:", event.payload);
          this.errorListeners.forEach((listener) => listener(event.payload));
        },
      );

      console.log("[AudioManager] Event listeners registered");
    } catch (error) {
      console.error("[AudioManager] Failed to setup event listeners:", error);
    }
  }

  // ============================================================================
  // Public API - Event Subscription
  // ============================================================================

  /**
   * Subscribe to audio state changes (play, pause, stop, etc.)
   */
  onStateChange(listener: (event: AudioStateChangedEvent) => void): () => void {
    this.stateChangeListeners.push(listener);
    return () => {
      const index = this.stateChangeListeners.indexOf(listener);
      if (index > -1) {
        this.stateChangeListeners.splice(index, 1);
      }
    };
  }

  /**
   * Subscribe to position updates during playback
   */
  onPositionUpdate(
    listener: (event: AudioPositionUpdateEvent) => void,
  ): () => void {
    this.positionUpdateListeners.push(listener);
    return () => {
      const index = this.positionUpdateListeners.indexOf(listener);
      if (index > -1) {
        this.positionUpdateListeners.splice(index, 1);
      }
    };
  }

  /**
   * Subscribe to audio errors
   */
  onError(listener: (event: AudioErrorEvent) => void): () => void {
    this.errorListeners.push(listener);
    return () => {
      const index = this.errorListeners.indexOf(listener);
      if (index > -1) {
        this.errorListeners.splice(index, 1);
      }
    };
  }

  /**
   * Subscribe to signal peak updates (for VU meter)
   */
  onSignalPeak(listener: (event: AudioSignalPeakEvent) => void): () => void {
    this.signalPeakListeners.push(listener);
    return () => {
      const index = this.signalPeakListeners.indexOf(listener);
      if (index > -1) {
        this.signalPeakListeners.splice(index, 1);
      }
    };
  }

  // ============================================================================
  // Public API - Playback Control
  // ============================================================================

  /**
   * Start audio playback with optional EQ filters
   */
  async startPlayback(
    filePath: string,
    outputDevice: string | null = null,
    sampleRate: number = 48000,
    channels: number = 2,
    filters: FilterParams[] = [],
  ): Promise<void> {
    console.log(
      `[AudioManager] Starting playback: ${filePath} (${sampleRate}Hz, ${channels}ch, ${filters.length} filters)`,
    );

    try {
      await invoke("audio_start_playback", {
        filePath,
        outputDevice,
        sampleRate,
        channels,
        filters,
      });
      console.log("[AudioManager] Playback started successfully");
    } catch (error) {
      console.error("[AudioManager] Failed to start playback:", error);
      throw error;
    }
  }

  /**
   * Stop audio playback
   */
  async stopPlayback(): Promise<void> {
    console.log("[AudioManager] Stopping playback");

    try {
      await invoke("audio_stop_playback");
      console.log("[AudioManager] Playback stopped successfully");
    } catch (error) {
      console.error("[AudioManager] Failed to stop playback:", error);
      throw error;
    }
  }

  /**
   * Update EQ filters in real-time during playback
   */
  async updateFilters(filters: FilterParams[]): Promise<void> {
    console.log(`[AudioManager] Updating ${filters.length} filters`);

    try {
      await invoke("audio_update_filters", { filters });
      console.log("[AudioManager] Filters updated successfully");
    } catch (error) {
      console.error("[AudioManager] Failed to update filters:", error);
      throw error;
    }
  }

  /**
   * Get the current audio state
   */
  async getState(): Promise<AudioStreamState> {
    try {
      const state = (await invoke("audio_get_state")) as AudioStreamState;
      return state;
    } catch (error) {
      console.error("[AudioManager] Failed to get state:", error);
      throw error;
    }
  }

  // ============================================================================
  // Public API - Recording Control
  // ============================================================================

  /**
   * Start audio recording
   */
  async startRecording(
    outputPath: string,
    inputDevice: string | null = null,
    sampleRate: number = 48000,
    channels: number = 2,
  ): Promise<void> {
    console.log(
      `[AudioManager] Starting recording: ${outputPath} (${sampleRate}Hz, ${channels}ch)`,
    );

    try {
      await invoke("audio_start_recording", {
        outputPath,
        inputDevice,
        sampleRate,
        channels,
      });
      console.log("[AudioManager] Recording started successfully");
    } catch (_error) {
      console.error("[AudioManager] Failed to start recording:", _error);
      throw _error;
    }
  }

  /**
   * Stop audio recording
   */
  async stopRecording(): Promise<void> {
    console.log("[AudioManager] Stopping recording");

    try {
      await invoke("audio_stop_recording");
      console.log("[AudioManager] Recording stopped successfully");
    } catch (error) {
      console.error("[AudioManager] Failed to stop recording:", error);
      throw error;
    }
  }

  // ============================================================================
  // Public API - Signal Monitoring
  // ============================================================================

  /**
   * Get the current signal peak (for VU meter)
   */
  async getSignalPeak(): Promise<number> {
    try {
      const peak = (await invoke("audio_get_signal_peak")) as number;
      return peak;
    } catch (error) {
      console.error("[AudioManager] Failed to get signal peak:", error);
      return 0.0;
    }
  }

  /**
   * Get the current recording SPL (Sound Pressure Level) in dB
   * Only works during recording
   */
  async getRecordingSPL(): Promise<number> {
    try {
      const spl = (await invoke("audio_get_recording_spl")) as number;
      return spl;
    } catch (error) {
      // Silent fail - not recording or no signal
      return -96.0; // Silence floor
    }
  }

  /**
   * Start polling signal peak at regular intervals
   * Returns a function to stop polling
   */
  startSignalPeakPolling(intervalMs: number = 100): () => void {
    let polling = true;

    const poll = async () => {
      while (polling) {
        try {
          const peak = await this.getSignalPeak();
          this.signalPeakListeners.forEach((listener) => listener({ peak }));
        } catch (error) {
          // Ignore errors during polling
        }
        await new Promise((resolve) => setTimeout(resolve, intervalMs));
      }
    };

    poll();

    return () => {
      polling = false;
    };
  }

  // ============================================================================
  // Cleanup
  // ============================================================================

  /**
   * Clean up event listeners and resources
   */
  async dispose(): Promise<void> {
    console.log("[AudioManager] Disposing resources");

    if (this.stateChangeUnlisten) {
      this.stateChangeUnlisten();
      this.stateChangeUnlisten = null;
    }

    if (this.errorUnlisten) {
      this.errorUnlisten();
      this.errorUnlisten = null;
    }

    this.stateChangeListeners = [];
    this.positionUpdateListeners = [];
    this.errorListeners = [];
    this.signalPeakListeners = [];
  }
}

// Export singleton instance
export const audioManagerRust = new AudioManagerRust();
