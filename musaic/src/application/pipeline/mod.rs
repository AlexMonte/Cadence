pub mod lowering;
pub mod runtime;
pub mod scene_sync;
pub mod ui_projection;

pub use ui_projection::{
    EditorUiProjection, LastEditorUiProjection, PaletteFingerprint, ShellChrome, UiDirty,
    UiProjectionInputs, compute_editor_ui_projection, diff_ui_regions, inspector_layout_identity,
    reset_ui_projection,
};

use bevy::prelude::*;

/// Document → sound: tessera compile, IR lowering, preview projection, and the
/// cadence audio runtime. Everything downstream of an accepted document edit
/// that exists to make the project audible.
pub struct PlaybackPlugin;

impl Plugin for PlaybackPlugin {
    fn build(&self, app: &mut App) {
        runtime::register_runtime(app);
        lowering::register_lowering(app);
        app.add_plugins(crate::adapter::audio::AudioPlugin);
    }
}
