//! Output adapter layer.
//!
//! Keep engine/runtime ownership in `application`. Future modules here should
//! translate scheduled intents into concrete outputs such as audio,
//! MIDI, OSC, or UI messages.

/// Concrete audio adapter pieces.
pub mod audio;
/// In-memory decoded sample bank used by the active runtime path.
pub mod sample_bank;
