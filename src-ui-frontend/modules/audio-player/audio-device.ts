// Audio device detection and capability utilities

export interface DeviceChannelInfo {
  inputChannels: number;
  outputChannels: number;
  deviceLabel: string;
  deviceId: string;
}

/**
 * Get the maximum channel count supported by an audio device
 * by requesting a high ideal channel count and checking what the browser returns
 */
export async function getDeviceMaxChannelCount(
  deviceId?: string,
): Promise<number | null> {
  try {
    // 1. Request user media with a high ideal channel count.
    // This prompts the browser to open the device with the max channels it can.
    const constraints: MediaStreamConstraints = {
      audio: {
        channelCount: { ideal: 128 },
      },
    };

    // Add device ID constraint if provided
    if (deviceId && deviceId !== "default") {
      (constraints.audio as MediaTrackConstraints).deviceId = {
        exact: deviceId,
      };
    }

    const stream = await navigator.mediaDevices.getUserMedia(constraints);

    // 2. Get the audio track from the stream.
    const audioTrack = stream.getAudioTracks()[0];
    if (!audioTrack) {
      console.error("No audio track found in the stream.");
      return null;
    }

    // 3. Get the settings of the track.
    const settings = audioTrack.getSettings();
    const maxChannelCount = settings.channelCount || 2;

    console.log(
      `The device supports a maximum of ${maxChannelCount} channels.`,
    );

    // 4. Important: Stop the track to release the microphone/device.
    audioTrack.stop();

    return maxChannelCount;
  } catch (error) {
    console.error("Error accessing the audio device:", error);
    if (error instanceof Error && error.name === "OverconstrainedError") {
      console.error("The requested channel count is not supported.");
    }
    return null;
  }
}

/**
 * Detect comprehensive device capabilities including input and output channels
 */
export async function detectDeviceCapabilities(
  deviceId: string,
): Promise<DeviceChannelInfo | null> {
  try {
    // Get device info to determine channel count
    const devices = await navigator.mediaDevices.enumerateDevices();
    const selectedDevice = devices.find((d) => d.deviceId === deviceId);

    if (!selectedDevice) {
      console.warn("Selected device not found");
      return null;
    }

    console.log("=== Audio Device Detection ===");
    console.log("Device ID:", deviceId);
    console.log("Device Label:", selectedDevice.label);

    // Try to get maximum channel count using the high ideal method
    const maxChannels = await getDeviceMaxChannelCount(deviceId);

    if (maxChannels === null) {
      console.warn(
        "Could not detect device capabilities, using defaults: 1 input, 2 output",
      );
      return {
        inputChannels: 1,
        outputChannels: 2,
        deviceLabel: selectedDevice.label || "Unknown Device",
        deviceId: deviceId,
      };
    }

    // Request access again to get detailed info
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        deviceId: deviceId === "default" ? undefined : { exact: deviceId },
        channelCount: { ideal: maxChannels },
      },
    });

    const audioTrack = stream.getAudioTracks()[0];
    const settings = audioTrack.getSettings();
    const capabilities = audioTrack.getCapabilities
      ? audioTrack.getCapabilities()
      : null;

    console.log("Track Label:", audioTrack.label);
    console.log("Track Settings:", settings);
    console.log("Track Capabilities:", capabilities);
    console.log("Settings.channelCount:", settings.channelCount);
    if (capabilities) {
      console.log(
        "Capabilities.channelCount:",
        (capabilities as any).channelCount,
      );
    }
    console.log("==============================");

    // Get input channel count
    let inputChannelCount = maxChannels;

    if (settings.channelCount) {
      inputChannelCount = settings.channelCount;
      console.log("✓ Using settings.channelCount:", inputChannelCount);
    } else if (capabilities && (capabilities as any).channelCount) {
      const chCount = (capabilities as any).channelCount;
      inputChannelCount =
        typeof chCount === "number" ? chCount : chCount.max || maxChannels;
      console.log(
        "✓ Using capabilities.channelCount:",
        chCount,
        "-> resolved to:",
        inputChannelCount,
      );
    } else {
      console.log("✓ Using max channel count from probe:", inputChannelCount);
    }

    // For audio interfaces, output channels often match input channels
    // This is a reasonable assumption for most audio interfaces
    const outputChannelCount = inputChannelCount;

    // Clean up stream
    stream.getTracks().forEach((track) => track.stop());

    console.log(
      `Device - Input: ${inputChannelCount} ch, Output: ${outputChannelCount} ch`,
    );

    return {
      inputChannels: inputChannelCount,
      outputChannels: outputChannelCount,
      deviceLabel: selectedDevice.label || audioTrack.label || "Unknown Device",
      deviceId: deviceId,
    };
  } catch (error) {
    console.error("Error detecting device capabilities:", error);
    return null;
  }
}

/**
 * Check if a device can be accessed for recording
 */
export async function checkDeviceAccess(deviceId: string): Promise<boolean> {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        deviceId: deviceId === "default" ? undefined : { exact: deviceId },
      },
    });

    stream.getTracks().forEach((track) => track.stop());
    return true;
  } catch (error) {
    console.error("Cannot access device:", error);
    return false;
  }
}
