// Audio device detection and capability utilities
// Uses Tauri backend instead of WebRTC

import { invoke } from "@tauri-apps/api/core";

export interface DeviceChannelInfo {
  inputChannels: number;
  outputChannels: number;
  deviceLabel: string;
  deviceId?: string;
}

/**
 * Get device capabilities from backend
 * @deprecated Use get_audio_devices() Tauri command directly
 */
export async function getDeviceMaxChannelCount(
  _deviceId?: string,
): Promise<number | null> {
  console.warn(
    "getDeviceMaxChannelCount() is deprecated. Use Tauri get_audio_devices() command.",
  );
  return 2; // Default to stereo
}

/**
 * Detect comprehensive device capabilities
 * @deprecated Use get_audio_devices() Tauri command directly
 */
export async function detectDeviceCapabilities(
  deviceId: string,
): Promise<DeviceChannelInfo | null> {
  console.warn(
    "detectDeviceCapabilities() is deprecated. Use Tauri get_audio_devices() command.",
  );

  try {
    // Call backend to get audio devices
    interface DeviceType {
      name: string;
      is_input?: boolean;
      default_config?: { channels?: number };
    }
    const devices: { input?: DeviceType[]; output?: DeviceType[] } = await invoke("get_audio_devices");

    // Find the requested device in input or output lists
    const allDevices = [...(devices.input || []), ...(devices.output || [])];

    const device = allDevices.find((d) => d.name === deviceId);

    if (!device) {
      return null;
    }

    // Extract channel information from default_config
    const channels = device.default_config?.channels || 2;

    return {
      inputChannels: device.is_input ? channels : 0,
      outputChannels: !device.is_input ? channels : 0,
      deviceLabel: device.name,
      deviceId: device.name,
    };
  } catch (error) {
    console.error("Error detecting device capabilities:", error);
    return null;
  }
}

/**
 * Check if a device can be accessed
 * @deprecated Use backend audio commands
 */
export async function checkDeviceAccess(_deviceId: string): Promise<boolean> {
  console.warn(
    "checkDeviceAccess() is deprecated. Backend handles device access.",
  );
  return true; // Backend will handle errors
}
