use bevy::prelude::*;

pub mod resources;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(resources::plugin);
}
