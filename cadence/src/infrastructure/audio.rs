//! Backend-agnostic audio runtime seam.
//!
//! Host crates own device/context startup and call `AudioRenderer` from their
//! dedicated audio thread or worklet. `cadence` owns the render queue,
//! voice mixing, and built-in audio source playback semantics behind that seam.
//!
//! `PlaybackRuntime` remains the main host API for score-first authoring and
//! normalized live input. The mixed trigger seam exported here is for advanced
//! host wiring, direct renderer integration, and other runtime-plumbing cases
//! where hosts need to route already-lowered sample/synth delivery packets.
//!
//! `SampleTrigger` and `SynthTrigger` are runtime delivery packets. They are
//! not a second authoring surface. Cadence should still lower typed meaning
//! into `Score` and `InputEvent` first, then let core project and schedule from
//! that typed source truth.

pub use crate::{
    adapter::audio::{
        AudioControl, AudioRenderer, AudioRendererError, AudioRendererSettings,
        AudioTriggerResolver, Frame, SampleBuffer,
    },
    application::{
        audio::{AudioTrigger, AudioTriggerReceiver, AudioTriggerSender, audio_trigger_channel},
        sample::SampleTrigger,
        synth::SynthTrigger,
    },
};
