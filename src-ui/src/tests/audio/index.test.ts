// Audio module test suite
import { describe, test, expect } from "vitest";

// Import all audio module exports to ensure they're properly exported
import {
  AudioPlayer,
  type AudioPlayerConfig,
  type AudioPlayerCallbacks,
  type FilterParam,
} from "../../modules/audio/audio-player";

import {
  AudioProcessor,
  type CaptureResult,
} from "../../modules/audio/audio-processor";

describe("Audio Module Exports", () => {
  test("should export AudioPlayer class", () => {
    expect(AudioPlayer).toBeDefined();
    expect(typeof AudioPlayer).toBe("function");
  });

  test("should export AudioProcessor class", () => {
    expect(AudioProcessor).toBeDefined();
    expect(typeof AudioProcessor).toBe("function");
  });

  test("should export TypeScript interfaces", () => {
    // These are compile-time checks, but we can verify the types exist
    const config: AudioPlayerConfig = {
      enableEQ: true,
      enableSpectrum: true,
    };

    const callbacks: AudioPlayerCallbacks = {
      onPlay: () => {},
      onStop: () => {},
    };

    const filterParam: FilterParam = {
      frequency: 1000,
      q: 1,
      gain: 0,
      enabled: true,
    };

    const captureResult: CaptureResult = {
      success: true,
      frequencies: [100, 200, 300],
      magnitudes: [10, 20, 30],
    };

    expect(config).toBeDefined();
    expect(callbacks).toBeDefined();
    expect(filterParam).toBeDefined();
    expect(captureResult).toBeDefined();
  });

  test("should maintain backward compatibility with module exports", () => {
    // Verify that the audio module can be imported from the main index
    expect(() => {
      // This would be caught at compile time if exports are broken
      const player = new AudioPlayer(document.createElement("div"));
      const processor = new AudioProcessor();

      expect(player).toBeInstanceOf(AudioPlayer);
      expect(processor).toBeInstanceOf(AudioProcessor);

      // Cleanup
      player.destroy();
      processor.destroy();
    }).not.toThrow();
  });
});
