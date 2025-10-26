// Audio processing - DEPRECATED
// This file has been replaced by CamillaDSP backend integration.
// All audio functionality now uses Tauri commands to the Rust backend.

export interface CaptureResult {
  frequencies: number[];
  magnitudes: number[];
  phases: number[];
  success: boolean;
  error?: string;
}

/**
 * @deprecated This class is deprecated. Use CamillaAudioManager instead.
 * Audio capture and WebRTC functionality has been removed.
 * Use backend `audio_start_recording()` command for recording features.
 */
export class AudioProcessor {
  private capturing: boolean = false;

  constructor() {
    console.warn(
      "AudioProcessor is deprecated. Use CamillaAudioManager for playback."
    );
  }

  // Stub methods to maintain API compatibility during migration
  async loadAudioFile(file: File): Promise<void> {
    throw new Error(
      "loadAudioFile() is deprecated. Use CamillaAudioManager instead."
    );
  }

  async loadAudioFromUrl(url: string): Promise<void> {
    throw new Error(
      "loadAudioFromUrl() is deprecated. Use CamillaAudioManager instead."
    );
  }

  updateFilterParams(filterParams: number[]): void {
    console.warn("updateFilterParams() is deprecated. Use CamillaAudioManager.");
  }

  setEQEnabled(enabled: boolean): void {
    console.warn("setEQEnabled() is deprecated. Use CamillaAudioManager.");
  }

  getCurrentTime(): number {
    return 0;
  }

  getDuration(): number {
    return 0;
  }

  isPlaying(): boolean {
    return false;
  }

  isCapturing(): boolean {
    return this.capturing;
  }

  setupSpectrumAnalyzer(canvas: HTMLCanvasElement): void {
    console.warn("setupSpectrumAnalyzer() is deprecated.");
  }

  startSpectrumAnalysis(): void {
    console.warn("startSpectrumAnalysis() is deprecated.");
  }

  stopSpectrumAnalysis(): void {
    // No-op
  }

  // Device enumeration - replaced by backend
  async enumerateAudioDevices(): Promise<never[]> {
    console.warn(
      "enumerateAudioDevices() removed. Use Tauri get_audio_devices() command."
    );
    return [];
  }

  async enumerateAudioOutputDevices(): Promise<never[]> {
    console.warn(
      "enumerateAudioOutputDevices() removed. Use Tauri get_audio_devices() command."
    );
    return [];
  }

  // Audio capture - replaced by backend recording
  async startCapture(deviceId?: string): Promise<CaptureResult> {
    return {
      frequencies: [],
      magnitudes: [],
      phases: [],
      success: false,
      error:
        "Audio capture removed. Use backend audio_start_recording() command.",
    };
  }

  stopCapture(): void {
    this.capturing = false;
  }

  setSweepDuration(duration: number): void {
    // No-op
  }

  setOutputChannel(channel: "left" | "right" | "both" | "default"): void {
    // No-op
  }

  setSampleRate(rate: number): void {
    // No-op
  }

  setSignalType(type: "sweep" | "white" | "pink"): void {
    // No-op
  }

  setCaptureVolume(volume: number): void {
    // No-op
  }

  setOutputVolume(volume: number): void {
    // No-op
  }

  setOutputDevice(deviceId: string): void {
    // No-op
  }

  getOutputDevice(): string {
    return "default";
  }

  isCaptureSupported(): boolean {
    return false; // WebRTC removed
  }

  setupAudioStatusElements(elements: any): void {
    // No-op
  }

  async play(): Promise<void> {
    throw new Error("play() is deprecated. Use CamillaAudioManager.");
  }

  stop(): void {
    // No-op
  }

  destroy(): void {
    // No-op
  }
}
