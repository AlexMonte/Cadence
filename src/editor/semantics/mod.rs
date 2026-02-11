//! Editor semantic reducers that turn interaction intents into deterministic state changes.

use bevy::prelude::*;

pub mod drag;
pub mod selection;

pub use drag::{DragIntent, DragSession};
pub use selection::{SelectionIntent, SelectionState};

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((selection::plugin, drag::plugin));
}
