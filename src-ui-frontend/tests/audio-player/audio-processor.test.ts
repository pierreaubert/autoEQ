// Tests for AudioProcessor functionality
import { describe, test, expect, beforeEach, vi, afterEach } from "vitest";
import {
  AudioProcessor,
  type CaptureResult,
} from "@audio-player/audio-processor";

// Mock Web Audio API
const mockAudioContext = {
  createGain: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    gain: { value: 1 },
  })),
  createAnalyser: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    fftSize: 2048,
    smoothingTimeConstant: 0.8,
    frequencyBinCount: 1024,
    getByteFrequencyData: vi.fn(),
    getFloatFrequencyData: vi.fn((array) => {
      // Fill with mock frequency data
      for (let i = 0; i < array.length; i++) {
        array[i] = Math.random() * -60; // dB values
      }
    }),
  })),
  createBuffer: vi.fn(
    (channels: number, length: number, sampleRate: number) => ({
      duration: length / sampleRate,
      length: length,
      numberOfChannels: channels,
      sampleRate: sampleRate,
      getChannelData: vi.fn(() => new Float32Array(length)),
    }),
  ),
  createBufferSource: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    buffer: null,
    onended: null,
  })),
  createBiquadFilter: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    type: "peaking",
    frequency: { value: 1000 },
    Q: { value: 1 },
    gain: { value: 0 },
  })),
  createOscillator: vi.fn(() => ({
    type: "sine",
    frequency: {
      value: 1000,
      setValueAtTime: vi.fn(),
      exponentialRampToValueAtTime: vi.fn(),
    },
    connect: vi.fn(),
    disconnect: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
  })),
  createChannelMerger: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  createChannelSplitter: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  createScriptProcessor: vi.fn(() => ({
    onaudioprocess: null,
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  createMediaStreamSource: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  decodeAudioData: vi.fn(),
  resume: vi.fn(),
  close: vi.fn(),
  destination: {},
  currentTime: 0,
  state: "running",
  sampleRate: 44100,
};

// Mock global AudioContext
(globalThis as any).AudioContext = vi.fn(function () {
  return mockAudioContext;
});
(globalThis as any).webkitAudioContext = vi.fn(function () {
  return mockAudioContext;
});

// Mock MediaDevices API
const mockMediaStream = {
  getTracks: vi.fn(() => [
    {
      stop: vi.fn(),
      kind: "audio",
      enabled: true,
    },
  ]),
};

// Mock navigator.mediaDevices
Object.defineProperty(navigator, "mediaDevices", {
  value: {
    getUserMedia: vi.fn(),
  },
  writable: true,
  configurable: true,
});

// Mock fetch for audio loading
globalThis.fetch = vi.fn();

// Mock setTimeout to speed up tests
let timeoutId = 0;
vi.stubGlobal("setTimeout", (callback: Function, delay?: number) => {
  // Speed up tests by reducing delays
  const actualDelay = Math.min(delay || 0, 10); // Max 10ms delay
  return (global.setTimeout as any)(callback, actualDelay);
});

describe("AudioProcessor", () => {
  let audioProcessor: AudioProcessor;

  beforeEach(() => {
    // Reset all mocks
    vi.clearAllMocks();

    // Reset getUserMedia mock
    (navigator.mediaDevices.getUserMedia as any).mockResolvedValue(
      mockMediaStream,
    );

    audioProcessor = new AudioProcessor();
  });

  afterEach(() => {
    if (audioProcessor) {
      audioProcessor.destroy();
    }
  });

  describe("Constructor and Initialization", () => {
    test("should create AudioProcessor instance", () => {
      expect(audioProcessor).toBeInstanceOf(AudioProcessor);
    });

    test("should initialize with default configuration", () => {
      expect(audioProcessor).toBeDefined();
      expect(audioProcessor.isCapturing()).toBe(false);
    });
  });

  describe("Public API", () => {
    test("should return capturing state", () => {
      expect(audioProcessor.isCapturing()).toBe(false);
    });

    test("should return playing state", () => {
      expect(audioProcessor.isPlaying()).toBe(false);
    });

    test("should have capture support check", () => {
      expect(typeof audioProcessor.isCaptureSupported()).toBe("boolean");
    });
  });

  describe("Capture Process", () => {
    test("should start capture and return result", async () => {
      // Set a very short duration for testing (50ms instead of 10 seconds)
      audioProcessor.setSweepDuration(0.05);
      const result = await audioProcessor.startCapture();

      expect(result).toBeDefined();
      expect(typeof result.success).toBe("boolean");
      expect(Array.isArray(result.frequencies)).toBe(true);
      expect(Array.isArray(result.magnitudes)).toBe(true);
    });

    test("should handle capture failure gracefully", async () => {
      const error = new Error("Microphone access denied");
      (navigator.mediaDevices.getUserMedia as any).mockRejectedValue(error);

      const result = await audioProcessor.startCapture();

      expect(result.success).toBe(false);
      expect(result.error).toBeDefined();
    });

    test("should stop capture when requested", () => {
      audioProcessor.stopCapture();
      expect(audioProcessor.isCapturing()).toBe(false);
    });
  });

  describe("Audio Loading", () => {
    test("should load audio from URL", async () => {
      const mockArrayBuffer = new ArrayBuffer(1024);
      const mockAudioBuffer = {
        duration: 10,
        sampleRate: 44100,
        numberOfChannels: 2,
      };

      (globalThis.fetch as any).mockResolvedValue({
        ok: true,
        arrayBuffer: () => Promise.resolve(mockArrayBuffer),
      });

      mockAudioContext.decodeAudioData.mockResolvedValue(mockAudioBuffer);

      await expect(
        audioProcessor.loadAudioFromUrl("/test.wav"),
      ).resolves.not.toThrow();
    });

    test("should handle audio loading errors", async () => {
      (globalThis.fetch as any).mockRejectedValue(new Error("Network error"));

      await expect(
        audioProcessor.loadAudioFromUrl("/test.wav"),
      ).rejects.toThrow("Network error");
    });
  });

  describe("Cleanup and Resource Management", () => {
    test("should cleanup resources properly", () => {
      expect(() => audioProcessor.destroy()).not.toThrow();
    });

    test("should handle cleanup when no resources allocated", () => {
      const newProcessor = new AudioProcessor();
      expect(() => newProcessor.destroy()).not.toThrow();
    });
  });
});
