//! Tile drawer panel: sole interactive library is the 3D palette viewport.

use bevy::{
    picking::prelude::{Pointer, Press, Release},
    prelude::*,
    ui::widget::Text as UiText,
};
use bevy_feathers::theme::ThemedText;

use crate::domain::document::TileSpawnKind;

use crate::infrastructure::ui::controls::tile_palette_viewport;

/// Marks a pressable tile source on a 3D palette entry.
#[derive(Component, Clone)]
pub(crate) struct DrawerTileSource {
    pub tile: TileSpawnKind,
}

/// Sole tile library: 3D palette viewport with on-tile glyphs.
///
/// Armed / target labels come from [`EditorUiProjection`](crate::application::pipeline::ui_projection::EditorUiProjection)
/// paint DTOs — never live [`EditorSession`](crate::application::editor::EditorSession).
pub(crate) fn spawn_tile_drawer(
    panel: &mut ChildSpawnerCommands<'_>,
    target_label: Option<&str>,
    armed_label: Option<&str>,
    palette_camera: Option<Entity>,
) {
    if let Some(label) = target_label {
        panel.spawn((UiText::new(label.to_string()), ThemedText));
    }
    if let Some(label) = armed_label {
        panel.spawn((UiText::new(format!("Armed: {label}")), ThemedText));
    }

    spawn_palette_viewport(panel, palette_camera);
}

fn spawn_palette_viewport(panel: &mut ChildSpawnerCommands<'_>, palette_camera: Option<Entity>) {
    if let Some(camera) = palette_camera {
        panel.spawn(tile_palette_viewport(camera));
    }
}

pub(crate) fn on_drawer_tile_press(
    press: On<Pointer<Press>>,
    sources: Query<&DrawerTileSource>,
    windows: Query<&Window>,
    mut queue: ResMut<crate::application::editor::DrawerTilePressQueue>,
) {
    let Ok(DrawerTileSource { tile }) = sources.get(press.event_target()) else {
        return;
    };
    let Some(start_screen) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    queue.pending = Some(crate::application::editor::DrawerTilePressed {
        tile: tile.clone(),
        start_screen,
    });
}

pub(crate) fn on_drawer_tile_release(_release: On<Pointer<Release>>) {
    // Press → place flow is owned by the editor interaction state machine.
}
