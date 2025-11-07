import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  SpectrumAnalyzerComponent,
  type SpectrumInfo,
} from "../modules/audio-player/spectrum-analyzer";
import { invoke } from "@tauri-apps/api/core";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("SpectrumAnalyzerComponent", () => {
  let canvas: HTMLCanvasElement;
  let component: SpectrumAnalyzerComponent;

  beforeEach(() => {
    // Clear all mocks before each test
    vi.clearAllMocks();

    // Create a canvas element
    canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 400;
    document.body.appendChild(canvas);

    // Create component
    component = new SpectrumAnalyzerComponent({
      canvas,
      pollInterval: 100,
      minFreq: 20,
      maxFreq: 20000,
      dbRange: 60,
      colorScheme: "dark",
      showLabels: true,
      showGrid: true,
    });
  });

  afterEach(() => {
    component.destroy();
    document.body.removeChild(canvas);
  });

  it("should create component with default config", () => {
    const defaultComponent = new SpectrumAnalyzerComponent({
      canvas,
    });

    expect(defaultComponent).toBeDefined();
    expect(defaultComponent.isActive()).toBe(false);
    expect(defaultComponent.getSpectrum()).toBeNull();
  });

  it("should initialize canvas context", () => {
    expect(component).toBeDefined();
    expect(canvas.width).toBeGreaterThan(0);
    expect(canvas.height).toBeGreaterThan(0);
  });

  it("should start monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();

    expect(mockInvoke).toHaveBeenCalledWith(
      "stream_enable_spectrum_monitoring",
    );
    expect(component.isActive()).toBe(true);
  });

  it("should stop monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();
    expect(component.isActive()).toBe(true);

    await component.stop();

    expect(mockInvoke).toHaveBeenCalledWith(
      "stream_disable_spectrum_monitoring",
    );
    expect(component.isActive()).toBe(false);
  });

  it("should handle start errors gracefully", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error("Test error"));

    await expect(component.start()).rejects.toThrow("Test error");
    expect(component.isActive()).toBe(false);
  });

  it("should not start if already monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();
    const callCount = mockInvoke.mock.calls.length;

    await component.start();

    expect(mockInvoke.mock.calls.length).toBe(callCount);
  });

  it("should not stop if not monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);

    await component.stop();

    expect(mockInvoke).not.toHaveBeenCalledWith(
      "stream_disable_spectrum_monitoring",
    );
  });

  it("should handle spectrum data", async () => {
    const mockSpectrum: SpectrumInfo = {
      frequencies: [20, 100, 1000, 10000, 20000],
      magnitudes: [-40, -30, -20, -10, -5],
      peak_magnitude: -5,
    };

    // Simulate receiving spectrum data
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(mockSpectrum);

    await component.start();

    // Wait for polling to get data
    await new Promise((resolve) => setTimeout(resolve, 150));

    const spectrum = component.getSpectrum();
    expect(spectrum).toBeDefined();
    expect(spectrum?.frequencies.length).toBe(5);
    expect(spectrum?.peak_magnitude).toBe(-5);
  });

  it("should handle null spectrum data", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(null);

    await component.start();

    // Wait for polling
    await new Promise((resolve) => setTimeout(resolve, 150));

    const spectrum = component.getSpectrum();
    expect(spectrum).toBeNull();
  });

  it("should resize canvas", () => {
    const originalWidth = canvas.width;
    canvas.style.width = "600px";

    component.resize();

    // Canvas should be resized (actual size depends on DPR)
    expect(canvas.width).toBeGreaterThan(0);
  });

  it("should cleanup on destroy", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();
    expect(component.isActive()).toBe(true);

    component.destroy();

    expect(component.isActive()).toBe(false);
  });

  it("should handle different color schemes", () => {
    const darkComponent = new SpectrumAnalyzerComponent({
      canvas,
      colorScheme: "dark",
    });

    const lightComponent = new SpectrumAnalyzerComponent({
      canvas,
      colorScheme: "light",
    });

    expect(darkComponent).toBeDefined();
    expect(lightComponent).toBeDefined();
  });

  it("should render without data", () => {
    // Should not throw
    expect(() => {
      component["render"]();
    }).not.toThrow();
  });

  it("should handle infinite magnitude values", async () => {
    const mockSpectrum: SpectrumInfo = {
      frequencies: [20, 100, 1000],
      magnitudes: [-Infinity, -30, -Infinity],
      peak_magnitude: -30,
    };

    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(mockSpectrum);

    await component.start();
    await new Promise((resolve) => setTimeout(resolve, 150));

    // Should not throw
    expect(() => {
      component["render"]();
    }).not.toThrow();
  });
});
