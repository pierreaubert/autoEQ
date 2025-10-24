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
  type FilterParam,
} from "@audio-player/audio-player";
export { LayoutManager } from "./layout-manager";

// Rust audio backend
export {
  AudioManagerRust,
  audioManagerRust,
  type FilterParams,
  type AudioStreamState,
  type AudioStateChangedEvent,
  type AudioPositionUpdateEvent,
  type AudioErrorEvent,
  type AudioSignalPeakEvent,
  AudioState,
} from "./audio-manager-rust";
