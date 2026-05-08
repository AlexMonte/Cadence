use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::adapter::catalog::classify_piece::classify_piece_id;
use crate::adapter::board_scene::{BoardCellState, BoardSceneState, TileVisualKind};
use crate::application::board::{
    BoardCommand, BoardFocus, BoardPointer, BoardSpaceVec2, BoardStore, InteractionStore,
    runtime::BoardCommandEvent,
};
use crate::infrastructure::theme::CadenceTheme;

#[derive(Component)]
struct BoardCamera;

#[derive(Component)]
struct BoardTile;

#[derive(Component)]
struct BoardGhost;

pub struct BoardRenderPlugin;

impl Plugin for BoardRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_board_camera).add_systems(
            Update,
            (
                sync_board_camera,
                sync_board_tiles,
                draw_board_gizmos,
                route_pointer_input,
            ),
        );
    }
}

fn spawn_board_camera(mut commands: Commands) {
    commands.spawn((Camera2d, BoardCamera));
}

fn sync_board_camera(
    focus: Res<BoardFocus>,
    mut cameras: Query<&mut Transform, With<BoardCamera>>,
) {
    if !focus.is_changed() {
        return;
    }

    let viewport = focus.active_viewport();
    for mut transform in &mut cameras {
        transform.translation.x = viewport.pan.x;
        transform.translation.y = viewport.pan.y;
        transform.scale = Vec3::splat(1.0 / viewport.zoom.max(0.1));
    }
}

fn sync_board_tiles(
    mut commands: Commands,
    scene: Res<BoardSceneState>,
    theme: Res<CadenceTheme>,
    existing_tiles: Query<Entity, With<BoardTile>>,
    existing_ghosts: Query<Entity, With<BoardGhost>>,
) {
    if !scene.is_changed() {
        return;
    }

    for entity in &existing_tiles {
        commands.entity(entity).despawn();
    }
    for entity in &existing_ghosts {
        commands.entity(entity).despawn();
    }

    for tile in &scene.scene.tiles {
        let (fill, size) = match tile.visual_kind {
            TileVisualKind::LargeContainer => (theme.tile_fill, Vec2::new(132.0, 86.0)),
            TileVisualKind::AtomChip => (Color::srgb(0.28, 0.42, 0.30), Vec2::new(76.0, 44.0)),
            TileVisualKind::OutputTile => (Color::srgb(0.42, 0.34, 0.20), Vec2::new(116.0, 72.0)),
            _ => (theme.tile_fill, Vec2::new(116.0, 72.0)),
        };
        commands.spawn((
            Sprite::from_color(
                if tile.selected {
                    theme.tile_selected_fill
                } else {
                    fill
                },
                size,
            ),
            Transform::from_xyz(tile.logical_rect.center.x, tile.logical_rect.center.y, 2.0),
            BoardTile,
        ));
    }

    if let Some(ghost) = &scene.scene.ghost {
        let ghost_color = match ghost.validity {
            crate::domain::board::PlacementVerdict::Valid => Color::srgba(0.56, 0.78, 0.98, 0.32),
            crate::domain::board::PlacementVerdict::Invalid { .. } => {
                Color::srgba(0.98, 0.38, 0.38, 0.28)
            }
        };
        commands.spawn((
            Sprite::from_color(
                ghost_color,
                Vec2::new(ghost.logical_rect.size.x, ghost.logical_rect.size.y),
            ),
            Transform::from_xyz(
                ghost.logical_rect.center.x,
                ghost.logical_rect.center.y,
                3.0,
            ),
            BoardGhost,
        ));
    }
}

fn draw_board_gizmos(
    mut gizmos: Gizmos,
    scene: Res<BoardSceneState>,
    theme: Res<CadenceTheme>,
) {
    for cell in &scene.scene.cells {
        let color = match cell.state {
            BoardCellState::Idle => theme.board_grid_minor,
            BoardCellState::Selected => theme.selection,
            BoardCellState::GhostValid => Color::srgba(0.56, 0.78, 0.98, 0.85),
            BoardCellState::GhostInvalid => Color::srgba(0.98, 0.38, 0.38, 0.85),
        };
        gizmos.rect_2d(
            Vec2::new(cell.logical_rect.center.x, cell.logical_rect.center.y),
            Vec2::new(cell.logical_rect.size.x, cell.logical_rect.size.y),
            color,
        );
    }
}

fn route_pointer_input(
    window: Single<&Window, With<PrimaryWindow>>,
    cameras: Single<(&Camera, &GlobalTransform), With<BoardCamera>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    board_focus: Res<BoardFocus>,
    board_store: Res<BoardStore>,
    interactions: Res<InteractionStore>,
    mut last_middle_cursor: Local<Option<Vec2>>,
    mut wheel_events: MessageReader<MouseWheel>,
    mut command_writer: MessageWriter<BoardCommandEvent>,
) {
    for event in wheel_events.read() {
        let factor = if event.y > 0.0 { 1.1 } else { 0.9 };
        command_writer.write(BoardCommandEvent(BoardCommand::ZoomViewport { factor }));
    }

    let cursor = window.cursor_position();
    let Some(cursor) = cursor else {
        return;
    };

    let (camera, camera_transform) = *cameras;
    let Ok(world_cursor) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };

    let pointer = BoardPointer {
        board_id: board_focus.active_board.clone(),
        board_position: BoardSpaceVec2 {
            x: world_cursor.x,
            y: world_cursor.y,
        },
    };
    let slot = crate::application::board::slot_resolution::resolve_slot(
        board_focus.active_layout(),
        pointer.board_position,
    );

    if buttons.just_released(MouseButton::Middle) {
        *last_middle_cursor = None;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if matches!(
            interactions.interaction,
            crate::application::board::BoardInteraction::Dragging(_)
        ) {
            command_writer.write(BoardCommandEvent(BoardCommand::CancelDrag));
        } else if board_focus.focus_path.len() > 1 {
            command_writer.write(BoardCommandEvent(BoardCommand::CloseContainer));
        }
    }

    if keys.just_pressed(KeyCode::Digit1) {
        command_writer.write(BoardCommandEvent(BoardCommand::StartPaletteDrag {
            piece_id: "cadence.container.basic".to_string(),
            class: classify_piece_id("cadence.container.basic"),
        }));
        command_writer.write(BoardCommandEvent(BoardCommand::UpdateDrag {
            pointer: pointer.clone(),
        }));
    }
    if keys.just_pressed(KeyCode::Digit2) {
        command_writer.write(BoardCommandEvent(BoardCommand::StartPaletteDrag {
            piece_id: "cadence.fast".to_string(),
            class: classify_piece_id("cadence.fast"),
        }));
        command_writer.write(BoardCommandEvent(BoardCommand::UpdateDrag {
            pointer: pointer.clone(),
        }));
    }
    if keys.just_pressed(KeyCode::Digit3) {
        command_writer.write(BoardCommandEvent(BoardCommand::StartPaletteDrag {
            piece_id: "cadence.atom.note".to_string(),
            class: classify_piece_id("cadence.atom.note"),
        }));
        command_writer.write(BoardCommandEvent(BoardCommand::UpdateDrag {
            pointer: pointer.clone(),
        }));
    }

    if buttons.just_pressed(MouseButton::Left) {
        if matches!(
            interactions.interaction,
            crate::application::board::BoardInteraction::Dragging(_)
        ) {
            command_writer.write(BoardCommandEvent(BoardCommand::CommitDrag));
            return;
        }

        if let Some(surface) = board_store.surface(&board_focus.active_board)
            && let Some(tile_id) = surface.occupancy.get(slot)
            && let Some(tile) = board_store.tile(tile_id)
            && tile.class == crate::domain::board::TileClass::Container
        {
            command_writer.write(BoardCommandEvent(BoardCommand::OpenContainer {
                tile_id: tile.id.clone(),
            }));
        } else {
            command_writer.write(BoardCommandEvent(BoardCommand::SelectSlot {
                board_id: board_focus.active_board.clone(),
                slot: Some(slot),
            }));
        }
    }

    if buttons.just_pressed(MouseButton::Right) {
        if matches!(
            interactions.interaction,
            crate::application::board::BoardInteraction::Dragging(_)
        ) {
            command_writer.write(BoardCommandEvent(BoardCommand::CancelDrag));
            return;
        }

        if let Some(surface) = board_store.surface(&board_focus.active_board)
            && let Some(tile_id) = surface.occupancy.get(slot)
        {
            command_writer.write(BoardCommandEvent(BoardCommand::StartTileDrag {
                tile_id: tile_id.clone(),
            }));
            command_writer.write(BoardCommandEvent(BoardCommand::UpdateDrag {
                pointer: pointer.clone(),
            }));
        }
    }

    if buttons.pressed(MouseButton::Middle) {
        if let Some(previous) = last_middle_cursor.replace(cursor) {
            let delta = cursor - previous;
            command_writer.write(BoardCommandEvent(BoardCommand::PanViewport {
                delta: BoardSpaceVec2 {
                    x: delta.x,
                    y: delta.y,
                },
            }));
        }
    }

    if matches!(
        interactions.interaction,
        crate::application::board::BoardInteraction::Dragging(_)
    ) {
        command_writer.write(BoardCommandEvent(BoardCommand::UpdateDrag {
            pointer: pointer.clone(),
        }));
    }
}
