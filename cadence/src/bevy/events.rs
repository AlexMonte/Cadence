use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

/// Emitted when active scores are replaced from the host lowering path.
#[derive(Message, Debug, Clone, Reflect)]
pub struct ScoreReplaced {
    /// Number of output scores now active.
    pub output_count: usize,
}

/// Emitted when playback transport state changes.
#[derive(Message, Debug, Clone, Copy, Reflect, PartialEq, Eq)]
pub struct PlaybackStatusChanged {
    /// Whether transport is actively playing.
    pub playing: bool,
}
