//! Scene projection: canonical document + attention + selection → visible board read model.

pub mod logic;
mod projection;
pub mod surface_content;
pub mod types;
pub mod view_model;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::state::condition::in_state;

use crate::infrastructure::app::MusaicSet;

pub use logic::{
    focused_node_from_attention, needs_visible_board_rebuild, render_focus_from_attention,
};
pub use types::{
    RenderBoardFocus, TileSurfaceContent, VisibleAtomCompound, VisibleBoardConnection,
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

#[derive(SystemParam)]
struct UiProjectionSystemParams<'w> {
    project: Res<'w, crate::application::session::MusaicProject>,
    attention: Res<'w, crate::application::editor::EditorAttention>,
    selection: Res<'w, crate::application::editor::SelectionState>,
    session: Res<'w, crate::application::editor::EditorSession>,
    cursor: Res<'w, crate::application::editor::CursorInteraction>,
    drawer_panel: Res<'w, crate::application::editor::DrawerPanelState>,
    minimap_panel: Res<'w, crate::application::editor::MinimapPanelState>,
    timeline_panel: Res<'w, crate::application::editor::TimelinePanelState>,
    visible: Res<'w, VisibleBoardState>,
    preview: Res<'w, crate::application::pipeline::runtime::RuntimePreviewSnapshot>,
    view_settings: Res<'w, crate::application::board_view_settings::BoardViewSettings>,
    diagnostics: Res<'w, crate::infrastructure::diagnostics::DiagnosticStore>,
    tile_assets: Option<Res<'w, crate::infrastructure::ui::transform_tile::TransformTileAssets>>,
    ui_sprites: Option<Res<'w, crate::adapter::load_up::UiSpriteAssets>>,
    projection: ResMut<'w, crate::application::pipeline::ui_projection::EditorUiProjection>,
    last: ResMut<'w, crate::application::pipeline::ui_projection::LastEditorUiProjection>,
    dirty: ResMut<'w, crate::application::pipeline::ui_projection::UiDirty>,
}

fn compute_ui_projection_system(mut params: UiProjectionSystemParams<'_>) {
    use crate::application::pipeline::ui_projection::{
        UiProjectionInputs, compute_editor_ui_projection, diff_ui_regions,
    };

    let next = compute_editor_ui_projection(&UiProjectionInputs {
        project: &params.project,
        attention: &params.attention,
        selection: &params.selection,
        session: &params.session,
        cursor: &params.cursor,
        drawer_panel: &params.drawer_panel,
        minimap_panel: &params.minimap_panel,
        timeline_panel: &params.timeline_panel,
        visible: &params.visible,
        preview: &params.preview,
        view_settings: *params.view_settings,
        diagnostics: &params.diagnostics,
        transform_tiles_ready: params.tile_assets.as_ref().is_some_and(|a| a.ready),
        ui_sprites_ready: params.ui_sprites.is_some(),
    });
    // First frame after reset: last is default — force full dirty via all-true when
    // last matches default and next is non-default, or always diff (default→real is dirty).
    *params.dirty = diff_ui_regions(&params.last.0, &next);
    params.last.0 = next.clone();
    *params.projection = next;
}

fn mark_scene_clean(mut runtime: ResMut<'_, crate::application::pipeline::runtime::RuntimeState>) {
    if runtime.needs_scene() {
        runtime.revisions.rendered_presentation = runtime.revisions.presentation;
    }
}
