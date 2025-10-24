/**
 * Audio Device Manager - Integrates cpal-based Tauri API with WebAudio
 * Provides unified interface for device enumeration and configuration
 */

import {
  getAudioDevices,
  setAudioDevice,
  getAudioConfig,
  getDeviceProperties,
  type AudioDevice,
  type AudioConfig,
  type AudioState,
  type AudioDevicesMap,
} from "@audio-player/audio-interface";

export interface UnifiedAudioDevice {
  deviceId: string;
  name: string;
  type: "input" | "output";
  isDefault: boolean;
  isWebAudio: boolean; // true if from WebAudio API, false if from cpal
  channels: number | null; // null if unknown
  sampleRates: number[];
  defaultSampleRate?: number;
  formats: string[];
  webAudioDevice?: MediaDeviceInfo; // Original WebAudio device if applicable
  cpalDevice?: AudioDevice; // Original cpal device if applicable
}

export interface DeviceSelectionResult {
  success: boolean;
  device?: UnifiedAudioDevice;
  config?: AudioConfig;
  error?: string;
}

/**
 * Enhanced Audio Device Manager that combines cpal and WebAudio capabilities
 */
export class AudioDeviceManager {
  private webAudioDevices: Map<string, MediaDeviceInfo> = new Map();
  private cpalDevices: Map<string, AudioDevice> = new Map();
  private unifiedDevices: Map<string, UnifiedAudioDevice> = new Map();
  private currentState: AudioState | null = null;
  private preferCpal: boolean = true; // Prefer cpal devices when available

  constructor(preferCpal: boolean = true) {
    this.preferCpal = preferCpal;
  }

  /**
   * Enumerate all available audio devices from both WebAudio and cpal
   */
  async enumerateDevices(): Promise<{
    input: UnifiedAudioDevice[];
    output: UnifiedAudioDevice[];
  }> {
    console.log("[DeviceManager] Enumerating devices from all sources...");

    // Clear previous device maps
    this.webAudioDevices.clear();
    this.cpalDevices.clear();
    this.unifiedDevices.clear();

    const inputDevices: UnifiedAudioDevice[] = [];
    const outputDevices: UnifiedAudioDevice[] = [];

    // First, enumerate WebAudio devices for mapping purposes
    // We need these to properly route audio in the browser
    await this.enumerateWebAudioDevices();

    // Get cpal devices first (if available)
    try {
      const cpalDeviceMap = await getAudioDevices();
      console.log("[DeviceManager] Got cpal devices:", {
        input: cpalDeviceMap.input.length,
        output: cpalDeviceMap.output.length,
      });

      // Process cpal input devices
      for (const device of cpalDeviceMap.input) {
        const unifiedDevice = this.cpalToUnified(device, "input");
        this.cpalDevices.set(unifiedDevice.deviceId, device);
        this.unifiedDevices.set(unifiedDevice.deviceId, unifiedDevice);
        inputDevices.push(unifiedDevice);
      }

      // Process cpal output devices
      for (const device of cpalDeviceMap.output) {
        const unifiedDevice = this.cpalToUnified(device, "output");
        this.cpalDevices.set(unifiedDevice.deviceId, device);
        this.unifiedDevices.set(unifiedDevice.deviceId, unifiedDevice);
        outputDevices.push(unifiedDevice);
      }
    } catch (error) {
      console.warn("[DeviceManager] Could not get cpal devices:", error);
      console.log("[DeviceManager] Falling back to WebAudio only");
    }

    // We enumerate both cpal and WebAudio devices
    // cpal devices are shown in the UI
    // WebAudio devices are used for actual capture/playback
    console.log(
      "[DeviceManager] Devices enumerated from both cpal and WebAudio",
    );
    console.log(
      "[DeviceManager] WebAudio devices available:",
      this.webAudioDevices.size,
    );

    console.log("[DeviceManager] Total unified devices:", {
      input: inputDevices.length,
      output: outputDevices.length,
    });

    return { input: inputDevices, output: outputDevices };
  }

  /**
   * Enumerate WebAudio devices for mapping
   */
  private async enumerateWebAudioDevices(): Promise<void> {
    try {
      // Request permissions first to get device labels
      // This is needed to properly map devices
      try {
        await navigator.mediaDevices.getUserMedia({ audio: true });
      } catch (e) {
        console.warn(
          "[DeviceManager] Could not get microphone permission for device labels",
        );
      }

      const devices = await navigator.mediaDevices.enumerateDevices();
      console.log(
        "[DeviceManager] WebAudio devices enumerated:",
        devices.length,
      );

      for (const device of devices) {
        if (device.kind === "audioinput" || device.kind === "audiooutput") {
          this.webAudioDevices.set(device.deviceId, device);
          console.log(
            `[DeviceManager] WebAudio ${device.kind}: "${device.label || "Unnamed"}" (${device.deviceId})`,
          );
        }
      }
    } catch (error) {
      console.error(
        "[DeviceManager] Failed to enumerate WebAudio devices:",
        error,
      );
    }
  }

  /**
   * Map a cpal device ID to a WebAudio device ID
   * This is crucial for getUserMedia() to work properly
   */
  mapToWebAudioDeviceId(cpalDeviceId: string): string {
    // If it's already a WebAudio device ID, return as-is
    if (!cpalDeviceId.startsWith("cpal_")) {
      return cpalDeviceId;
    }

    // Get the unified device
    const unifiedDevice = this.unifiedDevices.get(cpalDeviceId);
    if (!unifiedDevice || !unifiedDevice.cpalDevice) {
      console.warn(
        `[DeviceManager] No unified device found for: ${cpalDeviceId}`,
      );
      return "default";
    }

    const deviceName = unifiedDevice.name.toLowerCase();
    const deviceType = unifiedDevice.type;

    // Look for a matching WebAudio device by name
    let bestMatch: MediaDeviceInfo | null = null;
    let bestScore = 0;

    for (const [id, webDevice] of this.webAudioDevices.entries()) {
      // Check if it's the right type (input/output)
      if (deviceType === "input" && webDevice.kind !== "audioinput") continue;
      if (deviceType === "output" && webDevice.kind !== "audiooutput") continue;

      const webDeviceName = webDevice.label.toLowerCase();

      // Calculate similarity score
      let score = 0;

      // Exact match
      if (webDeviceName === deviceName) {
        score = 100;
      }
      // Contains the full name
      else if (
        webDeviceName.includes(deviceName) ||
        deviceName.includes(webDeviceName)
      ) {
        score = 80;
      }
      // Partial word matches
      else {
        const cpalWords = deviceName.split(/[\s\-_]+/);
        const webWords = webDeviceName.split(/[\s\-_]+/);

        for (const cpalWord of cpalWords) {
          for (const webWord of webWords) {
            if (cpalWord.length > 3 && webWord.length > 3) {
              if (cpalWord.includes(webWord) || webWord.includes(cpalWord)) {
                score += 20;
              }
            }
          }
        }
      }

      if (score > bestScore) {
        bestScore = score;
        bestMatch = webDevice;
      }
    }

    if (bestMatch && bestScore >= 20) {
      console.log(
        `[DeviceManager] Mapped "${unifiedDevice.name}" -> "${bestMatch.label}" (${bestMatch.deviceId}) [score: ${bestScore}]`,
      );
      return bestMatch.deviceId;
    }

    console.warn(
      `[DeviceManager] No WebAudio match for "${unifiedDevice.name}", using default`,
    );
    return "default";
  }

  /**
   * Convert cpal device to unified format
   */
  private cpalToUnified(
    device: AudioDevice,
    type: "input" | "output",
  ): UnifiedAudioDevice {
    // Extract unique sample rates from supported configs
    const sampleRates = Array.from(
      new Set(device.supported_configs.map((c) => c.sample_rate)),
    ).sort((a, b) => a - b);

    // Get channel count - check multiple sources
    let channels: number | null = null;

    // First, try to get from default_config
    if (device.default_config && device.default_config.channels) {
      const ch = device.default_config.channels;
      if (typeof ch === "number" && ch > 0) {
        channels = ch;
      }
    }

    // If not found, try supported_configs
    if (
      channels === null &&
      device.supported_configs &&
      device.supported_configs.length > 0
    ) {
      // Collect all unique channel counts
      const channelSet = new Set<number>();
      for (const config of device.supported_configs) {
        if (
          config.channels &&
          typeof config.channels === "number" &&
          config.channels > 0
        ) {
          channelSet.add(config.channels);
        }
      }

      if (channelSet.size > 0) {
        // Use the maximum channel count available
        channels = Math.max(...Array.from(channelSet));
      }
    }

    // Extract unique formats
    const formats = Array.from(
      new Set(device.supported_configs.map((c) => c.sample_format)),
    );

    const unifiedDevice = {
      deviceId: `cpal_${type}_${device.name.replace(/\s+/g, "_")}`,
      name: device.name,
      type,
      isDefault: device.is_default,
      isWebAudio: false,
      channels,
      sampleRates,
      defaultSampleRate: device.default_config?.sample_rate,
      formats,
      cpalDevice: device,
    };

    return unifiedDevice;
  }

  /**
   * Convert WebAudio device to unified format
   */
  private async webAudioToUnified(
    device: MediaDeviceInfo,
  ): Promise<UnifiedAudioDevice> {
    const type = device.kind === "audioinput" ? "input" : "output";

    // Try to get device capabilities - don't guess if we don't know
    let channels: number | null = null;
    let sampleRate: number | undefined = undefined;

    if (type === "input") {
      try {
        // Try to get actual capabilities
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            deviceId: { exact: device.deviceId },
            channelCount: { ideal: 32 },
          },
        });

        const track = stream.getAudioTracks()[0];
        const settings = track.getSettings();

        if (settings.channelCount) {
          channels = settings.channelCount;
        }

        // Get sample rate from audio context
        const audioContext = new AudioContext();
        sampleRate = audioContext.sampleRate;
        audioContext.close();

        // Clean up stream
        stream.getTracks().forEach((t) => t.stop());
      } catch (error) {
        console.warn(
          "[DeviceManager] Could not probe WebAudio device:",
          device.label,
          error,
        );
      }
    } else {
      // For output devices, we cannot directly probe them via WebAudio API
      // Don't make assumptions - report null for unknown
      console.log(
        "[DeviceManager] Output device channel count unknown (WebAudio):",
        device.label,
      );

      // Get sample rate from audio context
      try {
        const audioContext = new AudioContext();
        sampleRate = audioContext.sampleRate;
        audioContext.close();
      } catch (error) {
        console.warn(
          "[DeviceManager] Could not get audio context sample rate:",
          error,
        );
      }
    }

    return {
      deviceId: device.deviceId,
      name:
        device.label ||
        `${type === "input" ? "Microphone" : "Speaker"} ${device.deviceId.substr(0, 8)}`,
      type,
      isDefault: device.deviceId === "default",
      isWebAudio: true,
      channels,
      sampleRates: sampleRate ? [sampleRate] : [],
      defaultSampleRate: sampleRate,
      formats: ["f32"], // WebAudio typically uses float32
      webAudioDevice: device,
    };
  }

  /**
   * Select and configure an audio device
   */
  async selectDevice(
    deviceId: string,
    config?: Partial<AudioConfig>,
  ): Promise<DeviceSelectionResult> {
    const device = this.unifiedDevices.get(deviceId);

    if (!device) {
      return {
        success: false,
        error: `Device ${deviceId} not found`,
      };
    }

    console.log("[DeviceManager] Selecting device:", device.name, config);

    // If it's a cpal device, configure it through Tauri
    if (device.cpalDevice) {
      try {
        // Build config with defaults
        const fullConfig: AudioConfig = {
          sample_rate: config?.sample_rate || device.defaultSampleRate || 48000,
          channels:
            config?.channels ||
            (device.channels !== null ? Math.min(2, device.channels) : 2),
          buffer_size: config?.buffer_size,
          sample_format: (config?.sample_format ||
            device.formats[0] ||
            "f32") as any,
        };

        // Set the device configuration via Tauri
        const result = await setAudioDevice(
          device.cpalDevice.name,
          device.type === "input",
          fullConfig,
        );

        console.log("[DeviceManager] Device configured:", result);

        return {
          success: true,
          device,
          config: fullConfig,
        };
      } catch (error) {
        console.error("[DeviceManager] Error configuring cpal device:", error);
        return {
          success: false,
          error: String(error),
        };
      }
    }

    // For WebAudio devices, just return success as they're configured on use
    return {
      success: true,
      device,
      config: {
        sample_rate: device.defaultSampleRate || 48000,
        channels: Math.min(
          config?.channels || 2,
          device.channels !== null ? device.channels : 2,
        ),
        buffer_size: config?.buffer_size,
        sample_format: "f32",
      },
    };
  }

  /**
   * Get current audio configuration state
   */
  async getCurrentState(): Promise<AudioState | null> {
    try {
      this.currentState = await getAudioConfig();
      return this.currentState;
    } catch (error) {
      console.error("[DeviceManager] Error getting audio state:", error);
      return null;
    }
  }

  /**
   * Get detailed properties for a specific device
   */
  async getDeviceDetails(deviceId: string): Promise<any> {
    const device = this.unifiedDevices.get(deviceId);

    if (!device) {
      throw new Error(`Device ${deviceId} not found`);
    }

    // If it's a cpal device, get detailed properties from Tauri
    if (device.cpalDevice) {
      try {
        return await getDeviceProperties(
          device.cpalDevice.name,
          device.type === "input",
        );
      } catch (error) {
        console.error(
          "[DeviceManager] Error getting device properties:",
          error,
        );
      }
    }

    // Return basic info for WebAudio devices
    return {
      name: device.name,
      type: device.type,
      channels: device.channels,
      sampleRates: device.sampleRates,
      formats: device.formats,
      isWebAudio: true,
    };
  }

  /**
   * Find best matching device by criteria
   */
  findBestDevice(
    type: "input" | "output",
    criteria?: {
      preferredChannels?: number;
      preferredSampleRate?: number;
      preferDefault?: boolean;
    },
  ): UnifiedAudioDevice | null {
    const devices = Array.from(this.unifiedDevices.values()).filter(
      (d) => d.type === type,
    );

    if (devices.length === 0) {
      return null;
    }

    // If preferring default, return it
    if (criteria?.preferDefault) {
      const defaultDevice = devices.find((d) => d.isDefault);
      if (defaultDevice) return defaultDevice;
    }

    // Score devices based on criteria
    let bestDevice = devices[0];
    let bestScore = 0;

    for (const device of devices) {
      let score = 0;

      // Prefer cpal devices
      if (!device.isWebAudio) score += 10;

      // Match channel count
      if (criteria?.preferredChannels && device.channels !== null) {
        if (device.channels >= criteria.preferredChannels) {
          score += 5;
        }
      }

      // Match sample rate
      if (criteria?.preferredSampleRate) {
        if (device.sampleRates.includes(criteria.preferredSampleRate)) {
          score += 5;
        }
      }

      // Prefer devices with more capabilities
      score += device.sampleRates.length;
      if (device.channels !== null) {
        score += device.channels;
      }

      if (score > bestScore) {
        bestScore = score;
        bestDevice = device;
      }
    }

    return bestDevice;
  }

  /**
   * Create UI-friendly device list for dropdowns
   */
  getDeviceList(
    type: "input" | "output",
  ): Array<{ value: string; label: string; info?: string }> {
    const devices = Array.from(this.unifiedDevices.values()).filter(
      (d) => d.type === type,
    );

    const result = devices.map((device) => {
      // Build info string parts
      const infoParts: string[] = [];

      // Add channel count if available
      if (
        device.channels !== null &&
        device.channels !== undefined &&
        device.channels > 0
      ) {
        infoParts.push(`${device.channels}ch`);
      }

      // Add sample rate if available
      if (device.defaultSampleRate) {
        infoParts.push(`${Math.round(device.defaultSampleRate / 1000)}kHz`);
      }

      // Add default indicator
      if (device.isDefault) {
        infoParts.push("(Default)");
      }

      const info = infoParts.length > 0 ? infoParts.join(" ") : undefined;

      return {
        value: device.deviceId,
        label: device.name,
        info: info,
      };
    });

    return result;
  }
}
