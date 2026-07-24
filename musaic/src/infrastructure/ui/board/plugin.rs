use bevy::{prelude::*, state::condition::in_state};

use crate::application::editor::MusaicEditorSet;
use crate::infrastructure::app::{AppState, MusaicSet};
use crate::infrastructure::ui::theme::MusaicUiTheme;

use super::grid::sync_board_grid_anchor;
use super::materials::{Board3dMaterials, Board3dRenderCache};
use super::picking::{sample_board_placement_pointer, sync_board_cursor_hover};
use super::placement_ghost::{
    animate_placement_pulse, sync_connection_preview, sync_drag_preview,
    sync_placement_slot_highlight,
};
use super::scene_reconcile::{setup_board_3d_scene, sync_board_3d_scene, teardown_board_3d_scene};

pub struct Board3dPlugin;

impl Plugin for Board3dPlugin {
    fn build(&self, app: &mut App) {
        // Board chrome reads MusaicUiTheme (clear color, materials). Ensure the
        // resource exists even when Board3dPlugin is mounted without the full
        // MusaicPlugin theme insert; full-app insert_musaic_ui_theme overwrites
        // with the same default_dark + Feathers mapping.
        app.init_resource::<MusaicUiTheme>()
            .init_resource::<Board3dMaterials>()
            .init_resource::<Board3dRenderCache>()
            .add_systems(OnEnter(AppState::Editor), setup_board_3d_scene)
            .add_systems(OnExit(AppState::Editor), teardown_board_3d_scene)
            .add_systems(
                Update,
                sync_board_3d_scene
                    .after(MusaicSet::SceneSync)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                sample_board_placement_pointer
                    .before(MusaicEditorSet::MutateState)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    sync_board_cursor_hover,
                    // Grid backdrop follows the smoothed camera (read-only) so
                    // the slab always covers the view; runs after the rig's
                    // single transform writer.
                    sync_board_grid_anchor
                        .after(crate::infrastructure::ui::camera_rig::CameraRigSet::Smooth),
                    sync_drag_preview,
                    sync_connection_preview,
                    sync_placement_slot_highlight,
                    animate_placement_pulse,
                )
                    .after(MusaicEditorSet::MutateState)
                    .after(MusaicSet::SceneSync)
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

