//! Editor-facing screen state and screen plugin wiring.

use bevy::prelude::*;

pub mod editor;
mod loading;
mod splash;
mod title;

#[derive(States, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub enum Screen {
    #[default]
    Splash,
    Title,
    Loading,
    Editor,
}

pub(super) fn plugin(app: &mut App) {
    app.init_state::<Screen>();
    app.add_plugins((
        splash::plugin,
        loading::plugin,
        title::plugin,
        editor::plugin,
    ));
}
