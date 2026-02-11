//! Pointer and cursor input domain for editor interactions.

use bevy::prelude::*;

pub mod cursor;

pub use cursor::{
    Cursor, CursorAssets, Modifiers, PointerDownTarget, PointerModel, PointerPosition,
    PointerTarget,
};

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(cursor::plugin);
}
