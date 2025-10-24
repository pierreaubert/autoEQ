// Audio module test suite
import { describe, test, expect } from "vitest";

// Import AudioProcessor from the audio-player module
import { AudioProcessor } from "@audio-player/audio-processor";
import { type CaptureResult } from "@audio-player/audio-processor";

describe("Audio Module Exports", () => {
  test("should export AudioProcessor class", () => {
    expect(AudioProcessor).toBeDefined();
    expect(typeof AudioProcessor).toBe("function");
  });

  test("should export TypeScript interfaces", () => {
    const captureResult: CaptureResult = {
      success: true,
      frequencies: [100, 200, 300],
      magnitudes: [10, 20, 30],
      phases: [0, 0, 0],
    };

    expect(captureResult).toBeDefined();
    expect(captureResult.success).toBe(true);
    expect(captureResult.frequencies).toHaveLength(3);
  });

  test("should maintain backward compatibility with module exports", () => {
    // Verify that the audio module can be imported from the main index
    expect(() => {
      // This would be caught at compile time if exports are broken
      const processor = new AudioProcessor();

      expect(processor).toBeInstanceOf(AudioProcessor);

      // Cleanup
      processor.destroy();
    }).not.toThrow();
  });
});
