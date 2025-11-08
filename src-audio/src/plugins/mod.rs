// ============================================================================
// Audio Plugin System
// ============================================================================
//
// This module provides a flexible plugin system for audio processing.
// Plugins can be chained together in a host, with each plugin processing
// N input channels and producing P output channels.
//
// Architecture:
// - Plugin trait: Defines the interface for audio processing plugins
// - PluginHost: Chains multiple plugins together
// - Parameter system: Allows dynamic parameter changes
//
// Example usage:
// ```
// let mut host = PluginHost::new(2, 44100); // 2 channels, 44.1kHz
// let gain_plugin = GainPlugin::new(2, -6.0); // -6dB gain
// host.add_plugin(Box::new(gain_plugin));
// host.process(&mut audio_buffer);
// ```

mod compressor;
mod gain;
mod gate;
mod host;
mod limiter;
mod parameters;
mod plugin;
mod upmixer;

pub use compressor::CompressorPlugin;
pub use gain::GainPlugin;
pub use gate::GatePlugin;
pub use host::{PluginHost, SharedPluginHost};
pub use limiter::LimiterPlugin;
pub use parameters::{Parameter, ParameterId, ParameterValue};
pub use plugin::{InPlacePlugin, InPlacePluginAdapter, Plugin, PluginInfo, ProcessContext};
pub use upmixer::UpmixerPlugin;
