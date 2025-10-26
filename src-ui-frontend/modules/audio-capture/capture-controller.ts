/**
 * Capture Controller - Manages audio capture logic
 * Separates audio capture functionality from UI concerns
 */

import {
  AudioProcessor,
  type CaptureResult,
} from "@audio-player/audio-processor";
import { AudioDeviceManager } from "./device-manager";

export interface CaptureParameters {
  inputDevice: string;
  outputDevice: string;
  outputChannel: "left" | "right" | "both" | "default";
  signalType: "sweep" | "white" | "pink";
  duration: number;
  sampleRate: number;
  inputVolume: number; // 0-100
  outputVolume: number; // 0-100
}

export interface DeviceInfo {
  value: string;
  label: string;
  info?: string;
}

/**
 * Controller for managing audio capture operations
 */
export class CaptureController {
  private audioProcessor: AudioProcessor | null = null;
  private deviceManager: AudioDeviceManager;
  private isCapturing: boolean = false;

  constructor() {
    this.deviceManager = new AudioDeviceManager(true);
  }

  /**
   * Get list of available audio devices
   */
  async getAudioDevices(): Promise<{
    input: DeviceInfo[];
    output: DeviceInfo[];
  }> {
    await this.deviceManager.enumerateDevices();

    const inputList = this.deviceManager.getDeviceList("input");
    const outputList = this.deviceManager.getDeviceList("output");

    return {
      input: [{ value: "default", label: "System Default" }, ...inputList],
      output: [{ value: "default", label: "System Default" }, ...outputList],
    };
  }

  /**
   * Start audio capture with the given parameters
   */
  async startCapture(params: CaptureParameters): Promise<CaptureResult> {
    if (this.isCapturing) {
      throw new Error("Capture already in progress");
    }

    try {
      this.isCapturing = true;

      // Map device IDs from cpal to WebAudio
      const webAudioInputDevice = this.deviceManager.mapToWebAudioDeviceId(
        params.inputDevice,
      );
      const webAudioOutputDevice = this.deviceManager.mapToWebAudioDeviceId(
        params.outputDevice,
      );

      console.log("[CaptureController] Device ID mapping:", {
        cpalInput: params.inputDevice,
        webAudioInput: webAudioInputDevice,
        cpalOutput: params.outputDevice,
        webAudioOutput: webAudioOutputDevice,
      });

      // Create and configure audio processor
      this.audioProcessor = new AudioProcessor();
      this.audioProcessor.setSweepDuration(params.duration);
      this.audioProcessor.setOutputChannel(params.outputChannel);
      this.audioProcessor.setSampleRate(params.sampleRate);
      this.audioProcessor.setSignalType(params.signalType);
      this.audioProcessor.setCaptureVolume(params.inputVolume);
      this.audioProcessor.setOutputVolume(params.outputVolume);
      this.audioProcessor.setOutputDevice(webAudioOutputDevice);

      console.log("[CaptureController] Starting capture with parameters:", {
        ...params,
        webAudioInputDevice,
        webAudioOutputDevice,
      });

      // Start the actual capture
      const result =
        await this.audioProcessor.startCapture(webAudioInputDevice);

      // Cleanup on success
      if (result.success) {
        this.cleanup();
      }

      return result;
    } catch (error) {
      this.cleanup();
      throw error;
    }
  }

  /**
   * Stop the current capture
   */
  stopCapture(): void {
    console.log("[CaptureController] Stopping capture");

    if (this.audioProcessor) {
      this.audioProcessor.stopCapture();
    }

    this.cleanup();
  }

  /**
   * Clean up resources
   */
  private cleanup(): void {
    this.isCapturing = false;

    if (this.audioProcessor) {
      this.audioProcessor.destroy();
      this.audioProcessor = null;
    }
  }

  /**
   * Get the device manager instance
   */
  getDeviceManager(): AudioDeviceManager {
    return this.deviceManager;
  }

  /**
   * Check if capture is currently in progress
   */
  isCaptureInProgress(): boolean {
    return this.isCapturing;
  }

  /**
   * Destroy the controller and clean up resources
   */
  destroy(): void {
    this.stopCapture();
    this.cleanup();
  }
}
