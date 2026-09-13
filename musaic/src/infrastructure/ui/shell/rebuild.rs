//! Rebuild driver: consumes [`UiDirty`] from the post-SceneSync projection.

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    application::editor::{InspectorLayout, MinimapPanelState, TimelinePanelState},
    application::pipeline::scene_sync::VisibleBoardState,
    application::pipeline::ui_projection::{
        EditorUiProjection, UiDirty, inspector_layout_identity,
    },
};

use super::breadcrumbs::spawn_shell_footer;
use super::layout::{MusaicUiRoot, spawn_main_row};
use super::menu::spawn_top_menu;
use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::board::Board3dCamera;
use crate::infrastructure::ui::controls::UiTilePaletteDisplay;
use crate::infrastructure::ui::theme::{
    InspectorPanelHost, InspectorTransitionQueue, MusaicUiTheme, PendingInspectorTransition,
};
use crate::infrastructure::ui::{minimap_view, timeline_view};

#[derive(SystemParam)]
pub(crate) struct RebuildUiInputs<'w> {
    theme: Res<'w, MusaicUiTheme>,
    preferences: Res<'w, crate::application::editor::preferences::EditorPreferences>,
    minimap_panel: Res<'w, MinimapPanelState>,
    timeline_panel: Res<'w, TimelinePanelState>,
    visible: Res<'w, VisibleBoardState>,
    projection: Res<'w, EditorUiProjection>,
    dirty: Res<'w, UiDirty>,
    host: ResMut<'w, InspectorPanelHost>,
    transition_queue: ResMut<'w, InspectorTransitionQueue>,
    minimap_cache: ResMut<'w, minimap_view::MinimapContentCache>,
    timeline_cache: ResMut<'w, timeline_view::TimelineContentCache>,
    ui_sprites: Option<Res<'w, UiSpriteAssets>>,
    images: Res<'w, Assets<Image>>,
}

pub(crate) fn rebuild_ui(
    mut commands: Commands,
    existing_root: Query<'_, '_, Entity, With<MusaicUiRoot>>,
    board_camera: Query<'_, '_, Entity, With<Board3dCamera>>,
    mut palette_display: ResMut<'_, UiTilePaletteDisplay>,
    mut inputs: RebuildUiInputs,
) {
    let has_root = existing_root.iter().next().is_some();
    let dirty = *inputs.dirty;
    let inspector_layout = inputs.projection.inspector.clone();

    if let Some(library_context) = inspector_layout.drawer_context() {
        palette_display.context = library_context;
    }

    // No shell work when nothing structural/inspector-related changed and root exists.
    if has_root && !dirty.shell_structure && !dirty.shell_layout && !dirty.inspector {
        return;
    }

    // Panel dimension-only updates are handled by sync_shell_panel_sizes.
    if has_root && dirty.shell_layout && !dirty.shell_structure && !dirty.inspector {
        return;
    }

    // In-place inspector refresh (keep shell).
    if has_root && dirty.inspector && !dirty.shell_structure {
        let prev_identity = inputs
            .host
            .displayed_layout
            .as_ref()
            .map(|layout| inspector_layout_identity(Some(layout)))
            .unwrap_or_else(|| "empty".into());
        let animate = !inputs.preferences.reduced_motion
            && prev_identity != inputs.projection.inspector_identity;
        inputs.transition_queue.pending = Some(PendingInspectorTransition {
            layout: inspector_layout,
            animate,
        });
        return;
    }

    // Full shell respawn (first frame, surface/layout/sprites, or no root).
    if !has_root || dirty.shell_structure {
        full_shell_respawn(
            &mut commands,
            &existing_root,
            board_camera.iter().next(),
            &mut inputs,
            inspector_layout,
        );
    }
}

fn full_shell_respawn(
    commands: &mut Commands,
    existing_root: &Query<'_, '_, Entity, With<MusaicUiRoot>>,
    board_camera: Option<Entity>,
    inputs: &mut RebuildUiInputs<'_>,
    inspector_layout: InspectorLayout,
) {
    inputs.transition_queue.pending = None;
    *inputs.host = InspectorPanelHost::default();
    *inputs.minimap_cache = minimap_view::MinimapContentCache { force_next: true };
    *inputs.timeline_cache = timeline_view::TimelineContentCache { force_next: true };

    for entity in existing_root {
        commands.entity(entity).despawn();
    }

    commands
        .spawn((
            MusaicUiRoot,
            bevy::input_focus::tab_navigation::TabGroup::new(0),
            TextColor(inputs.theme.chrome.text_main),
            bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
            Node {
                width: percent(100),
                height: percent(100),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_top_menu(
                root,
                &inputs.theme,
                &inputs.images,
                inputs.ui_sprites.as_deref(),
            );
            crate::infrastructure::ui::first_loop::spawn(root, &inputs.theme);
            spawn_main_row(
                root,
                &inputs.theme,
                &inputs.visible,
                &inputs.projection,
                &inspector_layout,
                board_camera,
                inputs.minimap_panel.visible_width(),
                inputs.timeline_panel.visible_height(),
                &mut inputs.host,
                inputs.ui_sprites.as_deref(),
                &inputs.images,
            );
            spawn_shell_footer(root, &inputs.theme);
        });

    inputs.host.displayed_layout = Some(inspector_layout);
}
