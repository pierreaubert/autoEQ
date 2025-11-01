// Central export file for all modules

export * from "./optimization-constants";
export * from "./templates";
export { UIManager } from "./ui-manager";
export { PlotManager } from "./plot";
export { OptimizationManager } from "./optimization-manager";
export { APIManager } from "./api-manager";
export {
  AudioPlayer,
  type AudioPlayerConfig,
  type AudioPlayerCallbacks,
  type FilterParam as AudioPlayerFilterParam,
} from "@audio-player/audio-player";
export { LayoutManager } from "./layout-manager";

// Streaming audio manager (supports all formats: WAV, FLAC, MP3, etc.)
export {
  StreamingManager,
  type AudioFileInfo,
  type FilterParam,
  type AudioSpec,
  type LoudnessInfo,
  type AudioStreamState,
  type AudioManagerCallbacks,
} from "./audio-manager-streaming";
