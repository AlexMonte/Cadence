//! Gesture-level orchestration that maps pointer input into drag/marquee behaviors.

use bevy::prelude::*;

pub mod drag;
pub mod marquee;
pub mod pointer;

pub use pointer::PointerState;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((pointer::plugin, drag::plugin, marquee::plugin));
}
