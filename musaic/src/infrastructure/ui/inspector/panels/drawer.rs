//! Native tile library: named categories, fixed tile faces, and a scrolling list.

use bevy::{
    picking::prelude::{Pointer, Press, Release},
    prelude::*,
    ui::widget::Text as UiText,
};
use bevy_feathers::theme::ThemedText;

use crate::domain::document::TileSpawnKind;

use crate::infrastructure::ui::controls::tile_palette::spawn_tile_library;

/// Marks a pressable library tile source.
#[derive(Component, Clone)]
#[require(
    bevy::input_focus::tab_navigation::TabIndex,
    bevy::a11y::AccessibilityNode(accesskit::Node::new(accesskit::Role::Button))
)]
pub struct DrawerTileSource {
    pub tile: TileSpawnKind,
}

/// Sole tile library: fixed square faces with persistent names.
///
/// Armed / target labels come from [`EditorUiProjection`](crate::application::pipeline::ui_projection::EditorUiProjection)
/// paint DTOs — never live [`EditorSession`](crate::application::editor::EditorSession).
pub(crate) fn spawn_tile_drawer(
    panel: &mut ChildSpawnerCommands<'_>,
    target_label: Option<&str>,
    armed_label: Option<&str>,
) {
    panel
        .spawn((
            crate::infrastructure::ui::keyboard_regions::KeyboardRegion(
                crate::infrastructure::ui::keyboard_regions::RegionKind::Library,
            ),
            bevy::input_focus::tab_navigation::TabIndex(-1),
            Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
        ))
        .with_children(|panel| {
            if let Some(label) = target_label {
                panel.spawn((UiText::new(label.to_string()), ThemedText));
            }
            if let Some(label) = armed_label {
                panel.spawn((UiText::new(format!("Armed: {label}")), ThemedText));
            }

            crate::infrastructure::ui::controls::library_search::spawn(panel);
            crate::infrastructure::ui::controls::library_collections::spawn(panel);
            spawn_tile_library(panel);
        });
}

pub(crate) fn on_drawer_tile_press(
    mut press: On<Pointer<Press>>,
    sources: Query<&DrawerTileSource>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut queue: ResMut<crate::application::editor::DrawerTilePressQueue>,
    labels: Query<&crate::infrastructure::ui::controls::library_search::TileLibraryLabel>,
    mut preferences: ResMut<crate::application::editor::preferences::EditorPreferences>,
) {
    if press.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok(DrawerTileSource { tile }) = sources.get(press.entity) else {
        return;
    };
    let start_screen = press.pointer_location.position;
    if let Ok(label) = labels.get(press.entity) {
        preferences.library.choose(
            crate::application::editor::preferences::library::LibraryTileKey::new(tile, &label.0),
        );
    }
    focus.clear();
    queue.pending = Some(crate::application::editor::DrawerTilePressed {
        tile: tile.clone(),
        start_screen,
    });
    press.propagate(false);
}

pub(crate) fn on_drawer_tile_release(_release: On<Pointer<Release>>) {
    // Press → place flow is owned by the editor interaction state machine.
}

/// Library faces also own a drag gesture, so keyboard activation directly emits
/// the existing arm command without introducing a second pointer-click action.
pub(crate) fn on_drawer_tile_key(
    mut event: On<bevy::input_focus::FocusedInput<bevy::input::keyboard::KeyboardInput>>,
    sources: Query<&DrawerTileSource>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut bus: MessageWriter<crate::application::command::EditorCommandBus>,
    mut positions: Query<(
        Entity,
        &crate::infrastructure::ui::controls::library_search::LibraryPosition,
        &mut bevy::input_focus::tab_navigation::TabIndex,
    )>,
    labels: Query<&crate::infrastructure::ui::controls::library_search::TileLibraryLabel>,
    mut preferences: ResMut<crate::application::editor::preferences::EditorPreferences>,
    mut navigation: ResMut<
        crate::application::editor::interaction::keyboard_navigation::KeyboardNavigation,
    >,
    mut search: ResMut<crate::infrastructure::ui::controls::library_search::TileLibrarySearch>,
) {
    if event.input.state != bevy::input::ButtonState::Pressed {
        return;
    }
    let Ok(source) = sources.get(event.focused_entity) else {
        return;
    };
    let key = event.input.key_code;
    if key == KeyCode::KeyF
        && !event.input.repeat
        && ![
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
        ]
        .iter()
        .any(|k| keys.pressed(*k))
    {
        if let Ok(label) = labels.get(event.focused_entity) {
            crate::infrastructure::ui::controls::library_collections::toggle(
                &mut preferences,
                &mut navigation,
                &mut search,
                crate::application::editor::preferences::library::LibraryTileKey::new(
                    &source.tile,
                    &label.0,
                ),
                &label.0,
            );
        }
        keys.clear_just_pressed(key);
        event.propagate(false);
        return;
    }
    if matches!(
        key,
        KeyCode::ArrowLeft
            | KeyCode::ArrowRight
            | KeyCode::ArrowUp
            | KeyCode::ArrowDown
            | KeyCode::Home
            | KeyCode::End
    ) {
        let Ok((_, at, _)) = positions.get(event.focused_entity) else {
            return;
        };
        let at = *at;
        let mut candidates: Vec<_> = positions
            .iter()
            .filter(|(_, p, _)| p.body == at.body)
            .map(|(entity, position, _)| (entity, *position))
            .collect();
        candidates.retain(|(_, p)| match key {
            KeyCode::ArrowLeft => p.order < at.order,
            KeyCode::ArrowRight => p.order > at.order,
            KeyCode::ArrowUp => p.row < at.row,
            KeyCode::ArrowDown => p.row > at.row,
            _ => true,
        });
        candidates.sort_by_key(|(_, p)| match key {
            KeyCode::ArrowLeft | KeyCode::End => (usize::MAX - p.order, 0, 0),
            KeyCode::ArrowUp => (at.row - p.row, at.column.abs_diff(p.column), p.order),
            KeyCode::ArrowDown => (p.row - at.row, at.column.abs_diff(p.column), p.order),
            _ => (p.order, 0, 0),
        });
        if let Some((next, _)) = candidates.first() {
            if let Ok((_, _, mut index)) = positions.get_mut(event.focused_entity) {
                index.0 = -1;
            }
            if let Ok((_, _, mut index)) = positions.get_mut(*next) {
                index.0 = 0;
            }
            focus.set(*next);
        }
        keys.clear_just_pressed(key);
        event.propagate(false);
        return;
    }
    if event.input.repeat || !matches!(key, KeyCode::Enter | KeyCode::Space) {
        return;
    }
    event.propagate(false);
    keys.clear_just_pressed(event.input.key_code);
    if let Ok(label) = labels.get(event.focused_entity) {
        preferences.library.choose(
            crate::application::editor::preferences::library::LibraryTileKey::new(
                &source.tile,
                &label.0,
            ),
        );
    }
    bus.write(crate::application::command::EditorCommandBus(
        crate::application::command::EditorCommand::ArmPlacementTool {
            tile: source.tile.clone(),
        },
    ));
    focus.clear();
}
