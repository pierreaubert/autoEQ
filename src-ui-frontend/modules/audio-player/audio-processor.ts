/**
 * Removed: WebAudio/WebRTC-based AudioProcessor.
 * This project is Tauri-only now. Use AudioManagerRust for playback/recording.
 */

// Kept for type compatibility with capture modules
export interface CaptureResult {
  success: boolean;
  frequencies: number[];
  magnitudes: number[];
  phases?: number[];
  error?: string;
}

export class AudioProcessor {
  constructor() {
    throw new Error(
      "AudioProcessor has been removed. Use AudioManagerRust instead.",
    );
  }
  destroy(): void {
    // no-op
  }
}
