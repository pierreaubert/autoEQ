// Device Manager - DEPRECATED
// Audio capture functionality has been removed as it relied on WebRTC.
// Use backend CamillaDSP recording commands for audio capture features.

import { invoke } from "@tauri-apps/api/core";

/**
 * @deprecated Audio capture has been removed. Use backend audio_start_recording() command.
 */
export class DeviceManager {
  constructor(_autoInit?: boolean) {
    console.warn(
      "DeviceManager is deprecated. Audio capture removed (WebRTC dependency). " +
        "Use Tauri audio_start_recording() command for recording features.",
    );
  }

  /**
   * Get available audio devices from backend
   */
  async getDevices(): Promise<{ input: unknown[]; output: unknown[] }> {
    try {
      return await invoke("get_audio_devices");
    } catch (error) {
      console.error("Failed to get audio devices:", error);
      return { input: [], output: [] };
    }
  }

  /**
   * Start recording using backend CamillaDSP
   * @param outputPath Path where recording will be saved
   * @param inputDevice Optional input device name
   * @param sampleRate Sample rate (default: 48000)
   * @param channels Number of channels (default: 2)
   */
  async startRecording(
    outputPath: string,
    inputDevice?: string,
    sampleRate: number = 48000,
    channels: number = 2,
  ): Promise<void> {
    try {
      await invoke("audio_start_recording", {
        outputPath,
        inputDevice,
        sampleRate,
        channels,
      });
      console.log("Recording started via backend");
    } catch (error) {
      console.error("Failed to start recording:", error);
      throw error;
    }
  }

  /**
   * Stop recording
   */
  async stopRecording(): Promise<void> {
    try {
      await invoke("audio_stop_recording");
      console.log("Recording stopped");
    } catch (error) {
      console.error("Failed to stop recording:", error);
      throw error;
    }
  }

  // Stub methods for backwards compatibility
  async initializeDevices(): Promise<void> {
    console.warn("initializeDevices() is deprecated.");
  }

  async selectInputDevice(_deviceId: string): Promise<void> {
    console.warn(
      "selectInputDevice() is deprecated. Use startRecording() with inputDevice parameter.",
    );
  }

  async selectOutputDevice(_deviceId: string): Promise<void> {
    console.warn("selectOutputDevice() is deprecated.");
  }

  getSelectedInputDevice(): string | null {
    return null;
  }

  getSelectedOutputDevice(): string | null {
    return null;
  }

  isRecording(): boolean {
    console.warn(
      "isRecording() is deprecated. Query backend audio state instead.",
    );
    return false;
  }

  destroy(): void {
    // No-op
  }

  // Additional compatibility methods for capture-controller
  async enumerateDevices(): Promise<{ input: unknown[]; output: unknown[] }> {
    return await this.getDevices();
  }

  getDeviceList(
    _type: "input" | "output",
  ): Array<{ value: string; label: string; info?: string }> {
    console.warn("getDeviceList() is deprecated. Use getDevices() instead.");
    return [];
  }

  mapToWebAudioDeviceId(cpalDeviceId: string): string {
    console.warn("mapToWebAudioDeviceId() is deprecated. WebRTC removed.");
    return cpalDeviceId; // Pass through, no mapping needed
  }
}

// Compatibility exports
export { DeviceManager as AudioDeviceManager };
export default DeviceManager;
