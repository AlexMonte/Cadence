//! Concrete audio adapter implementation details.

/// Trigger-resolution bridge from application audio queues to render commands.
pub mod engine;
/// Stereo frame type and interpolation helpers.
pub mod frame;
/// Voice mixer and effect buses.
pub mod mixer;
/// Position values inside a decoded sample.
pub mod playback_position;
/// Playback-region helpers.
pub mod region;
/// Host-owned renderer and render-command queue.
pub mod renderer;
/// Small interpolation buffer for resampling.
pub mod resampler;
/// Fully decoded in-memory sample storage.
pub mod sample_buffer;
/// Spatial rendering from lattice position to channel output.
pub mod spatializer;
/// Forward/reverse playback transport over a sample region.
pub mod transport;
/// Active sample and synth voice renderers.
pub mod voice;

pub use crate::adapter::sample_bank::LoadedSampleTrigger;
pub use engine::AudioTriggerResolver;
pub use frame::{Frame, interpolate_frame};
pub use mixer::{AudioMixer, SampleMixer};
pub use playback_position::PlaybackPosition;
pub use region::{EndPosition, Region};
pub use renderer::{
    AudioControl, AudioRenderer, AudioRendererError, AudioRendererSettings, RenderCommand,
    RenderCommandError, RenderCommandSender,
};
pub use sample_buffer::SampleBuffer;
pub use spatializer::{ChannelLayout, Spatializer, StereoVectorPanner};
pub use transport::Transport;
pub use voice::{SampleVoice, SynthVoice};
