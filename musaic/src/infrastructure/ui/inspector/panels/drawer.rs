//! Tile drawer panel: sole interactive library is the 3D palette viewport.

use bevy::{
    picking::prelude::{Pointer, Press, Release},
    prelude::*,
    ui::widget::Text as UiText,
};
use bevy_feathers::theme::ThemedText;

use crate::application::editor::{
    EditorSession, TileLibraryContextKind, atom_tile_drawer_rows, labeled_tile_drawer_items,
    placement_target_label, transaction::PlacementTarget,
};
use crate::domain::document::TileSpawnKind;

use crate::infrastructure::ui::controls::tile_palette_viewport;

/// Marks a pressable tile source on a 3D palette entry.
#[derive(Component, Clone)]
pub(crate) struct DrawerTileSource {
    pub tile: TileSpawnKind,
}

/// Sole tile library: 3D palette viewport with on-tile glyphs.
///
/// Catalog identity comes from the application layer ([`TileDrawerItem`] via
/// [`basic_tile_options`](crate::application::editor::basic_tile_options)).
/// There is no duplicate label-chip grid.
pub(crate) fn spawn_tile_drawer(
    panel: &mut ChildSpawnerCommands<'_>,
    session: &EditorSession,
    target: Option<&PlacementTarget>,
    _context: TileLibraryContextKind,
    palette_camera: Option<Entity>,
) {
    if let Some(target) = target {
        panel.spawn((UiText::new(placement_target_label(target)), ThemedText));
    }
    if let Some(armed) = session.armed_tile() {
        panel.spawn((
            UiText::new(format!("Armed: {}", drawer_item_short_label(armed))),
            ThemedText,
        ));
    }

    spawn_palette_viewport(panel, palette_camera);
}

fn spawn_palette_viewport(panel: &mut ChildSpawnerCommands<'_>, palette_camera: Option<Entity>) {
    if let Some(camera) = palette_camera {
        panel.spawn(tile_palette_viewport(camera));
    }
}

fn drawer_item_short_label(spawn: &TileSpawnKind) -> String {
    labeled_tile_drawer_items()
        .into_iter()
        .find(|item| &item.spawn == spawn)
        .map(|item| item.label)
        .or_else(|| {
            atom_tile_drawer_rows()
                .into_iter()
                .flat_map(|(_, row)| row)
                .find(|item| &item.spawn == spawn)
                .map(|item| item.label)
        })
        .unwrap_or_else(|| format!("{spawn:?}"))
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
