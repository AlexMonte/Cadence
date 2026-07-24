//! Scene projection: canonical document + attention + selection → visible board read model.

pub mod board_scene;
pub mod logic;
pub mod surface_content;
pub mod types;
pub mod view_model;

use bevy::prelude::*;
use bevy::state::condition::in_state;

use crate::infrastructure::app::MusaicSet;

pub use board_scene::{
    BoardSceneAtomCompound, BoardSceneConnection, BoardSceneTile, TileVisualKind,
    project_board_scene,
};
pub use logic::{
    focused_node_from_attention, needs_visible_board_rebuild, render_focus_from_attention,
};
pub use types::{
    BoardScene, RenderBoardFocus, TileSurfaceContent, VisibleAtomCompound, VisibleBoardConnection,
    VisibleBoardNode, VisibleNodeKind,
};
pub use view_model::VisibleBoardState;

/// Registers the visible-board projection and UI projection. Called by the editor plugin.
pub fn register_scene_sync(app: &mut App) {
    app.init_resource::<VisibleBoardState>()
        .init_resource::<crate::application::pipeline::ui_projection::EditorUiProjection>()
        .init_resource::<crate::application::pipeline::ui_projection::LastEditorUiProjection>()
        .init_resource::<crate::application::pipeline::ui_projection::UiDirty>()
        .add_systems(
            OnEnter(crate::infrastructure::app::AppState::Editor),
            crate::application::pipeline::ui_projection::reset_ui_projection,
        )
        .add_systems(
            Update,
            (
                view_model::rebuild_visible_board_state,
                compute_ui_projection_system,
                mark_scene_clean,
            )
                .chain()
                .in_set(MusaicSet::SceneSync)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

fn compute_ui_projection_system(
    project: Res<'_, crate::application::session::MusaicProject>,
    attention: Res<'_, crate::application::editor::EditorAttention>,
    selection: Res<'_, crate::application::editor::SelectionState>,
    session: Res<'_, crate::application::editor::EditorSession>,
    cursor: Res<'_, crate::application::editor::CursorInteraction>,
    drawer_panel: Res<'_, crate::application::editor::DrawerPanelState>,
    minimap_panel: Res<'_, crate::application::editor::MinimapPanelState>,
    timeline_panel: Res<'_, crate::application::editor::TimelinePanelState>,
    visible: Res<'_, VisibleBoardState>,
    preview: Res<'_, crate::application::pipeline::runtime::RuntimePreviewSnapshot>,
    diagnostics: Res<'_, crate::infrastructure::diagnostics::DiagnosticStore>,
    tile_assets: Option<Res<'_, crate::infrastructure::ui::transform_tile::TransformTileAssets>>,
    ui_sprites: Option<Res<'_, crate::adapter::load_up::UiSpriteAssets>>,
    mut projection: ResMut<'_, crate::application::pipeline::ui_projection::EditorUiProjection>,
    mut last: ResMut<'_, crate::application::pipeline::ui_projection::LastEditorUiProjection>,
    mut dirty: ResMut<'_, crate::application::pipeline::ui_projection::UiDirty>,
) {
    use crate::application::pipeline::ui_projection::{
        UiProjectionInputs, compute_editor_ui_projection, diff_ui_regions,
    };

    let next = compute_editor_ui_projection(&UiProjectionInputs {
        project: &project,
        attention: &attention,
        selection: &selection,
        session: &session,
        cursor: &cursor,
        drawer_panel: &drawer_panel,
        minimap_panel: &minimap_panel,
        timeline_panel: &timeline_panel,
        visible: &visible,
        preview: &preview,
        diagnostics: &diagnostics,
        transform_tiles_ready: tile_assets.as_ref().is_some_and(|a| a.ready),
        ui_sprites_ready: ui_sprites.is_some(),
    });
    // First frame after reset: last is default — force full dirty via all-true when
    // last matches default and next is non-default, or always diff (default→real is dirty).
    *dirty = diff_ui_regions(&last.0, &next);
    last.0 = next.clone();
    *projection = next;
}

fn mark_scene_clean(mut runtime: ResMut<'_, crate::application::pipeline::runtime::RuntimeState>) {
    if runtime.dirty.scene {
        runtime.dirty.scene = false;
    }
}
