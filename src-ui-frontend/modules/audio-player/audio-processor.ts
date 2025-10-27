/**
 * Removed: WebAudio/WebRTC-based AudioProcessor.
 * This project is Tauri-only now. Use CamillaAudioManager for playback/recording.
 */

// Kept only for temporary compatibility; any usage will throw immediately.
export type CaptureResult = never;

export class AudioProcessor {
  constructor() {
    throw new Error(
      "AudioProcessor has been removed. Use CamillaAudioManager instead.",
    );
  }
  destroy(): void {
    // no-op
  }
}
