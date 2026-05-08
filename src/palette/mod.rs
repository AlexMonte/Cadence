use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Default)]
pub struct PaletteState {
    pub is_open: bool,
}

pub struct PalettePlugin;

impl Plugin for PalettePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaletteState>();
    }
}
