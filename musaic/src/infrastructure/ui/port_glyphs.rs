//! Port compass glyphs on board tiles (3D billboards) and in the inspector panel.

use bevy::{math::primitives::Cuboid, prelude::*};
use tessera::prelude::{NodeId, SpatialSide};

use crate::{
    adapter::load_up::UiSpriteAssets,
    application::command::EditorCommand,
    application::editor::interaction::{BoardPickEvent, BoardPickHit, BoardPickTargetKind},
    application::editor::{
        ConnectionEndpointView, CursorInteraction, EditorSession, PortSlotState,
        blocks_board_picks_with_cursor,
    },
    domain::board::BoardSurfaceId,
    infrastructure::ui::{
        InspectorButtonAction, board::BoardPickTarget, render_layers::SCENE_NODE_VISIBILITY,
        ui_sprites::port_slot_image,
    },
};

const PORT_RING_SCALE: f32 = 0.52;
const PORT_GLYPH_WORLD: f32 = 0.18;

/// Spawn N/E/S/W port glyphs around a flat board tile.
pub fn spawn_board_port_compass(
    tile: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    node: &NodeId,
    footprint: f32,
    view: &ConnectionEndpointView,
    _images: &Assets<Image>,
    sprites: &UiSpriteAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let ring = footprint * PORT_RING_SCALE;
    let sides = [
        (SpatialSide::North, Vec3::new(0.0, 0.0, -ring), view.north),
        (SpatialSide::East, Vec3::new(ring, 0.0, 0.0), view.east),
        (SpatialSide::South, Vec3::new(0.0, 0.0, ring), view.south),
        (SpatialSide::West, Vec3::new(-ring, 0.0, 0.0), view.west),
    ];

    for (side, offset, state) in sides {
        spawn_port_glyph_3d(
            tile,
            surface,
            node.clone(),
            side,
            state,
            offset,
            sprites,
            meshes,
            materials,
        );
    }
}

fn spawn_port_glyph_3d(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    tile_id: NodeId,
    side: SpatialSide,
    state: PortSlotState,
    offset: Vec3,
    sprites: &UiSpriteAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let handle = port_slot_image(sprites, state).clone();
    let world = PORT_GLYPH_WORLD;
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(handle),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    let mesh = meshes.add(Mesh::from(Cuboid::new(world, 0.02, world)));

    parent
        .spawn((
            BoardPickTarget {
                surface_id: surface,
                kind: BoardPickTargetKind::PortSide { tile_id, side },
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            SCENE_NODE_VISIBILITY,
            Transform::from_translation(offset + Vec3::new(0.0, 0.08, 0.0)),
            Pickable::default(),
        ))
        .observe(on_port_side_clicked);
}

pub fn on_port_side_clicked(
    mut event: On<'_, '_, Pointer<Click>>,
    session: Res<'_, EditorSession>,
    cursor: Res<'_, CursorInteraction>,
    targets: Query<'_, '_, &BoardPickTarget>,
    mut pick_events: MessageWriter<'_, BoardPickEvent>,
) {
    if blocks_board_picks_with_cursor(&session, cursor.phase()) {
        event.propagate(false);
        return;
    }
    let entity = event.original_event_target();
    let Ok(target) = targets.get(entity) else {
        return;
    };
    let Some(position) = event.hit.position else {
        return;
    };

    pick_events.write(BoardPickEvent::Hit(BoardPickHit {
        surface_id: target.surface_id,
        kind: target.kind.clone(),
        world_position: position,
        distance: 0.0,
    }));
    event.propagate(false);
}

pub fn spawn_inspector_port_compass(
    parent: &mut ChildSpawnerCommands<'_>,
    _images: &Assets<Image>,
    _sprites: Option<&UiSpriteAssets>,
    node: &NodeId,
    view: &ConnectionEndpointView,
    glyph: &str,
) {
    let theme = super::theme::MusaicUiTheme::default();
    parent
        .spawn((
            Node {
                width: percent(100),
                padding: UiRect::all(px(12)),
                margin: UiRect::top(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(theme.chrome.panel_inset),
            BorderColor::all(theme.chrome.border),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new("Connections"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            card.spawn(Node {
                width: percent(100),
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                grid_template_rows: RepeatedGridTrack::px(3, 48.0),
                column_gap: px(5),
                row_gap: px(5),
                ..default()
            })
            .with_children(|grid| {
                for (row, col, side, state) in [
                    (1, 2, SpatialSide::North, view.north),
                    (2, 3, SpatialSide::East, view.east),
                    (3, 2, SpatialSide::South, view.south),
                    (2, 1, SpatialSide::West, view.west),
                ] {
                    let symbol = match state {
                        PortSlotState::None => "·",
                        PortSlotState::Input => "[ ]",
                        PortSlotState::Output => match side {
                            SpatialSide::North => "↑",
                            SpatialSide::East => "→",
                            SpatialSide::South => "↓",
                            _ => "←",
                        },
                    };
                    let label = format!(
                        "{}: {}. Set output on this side",
                        side_label(side),
                        port_state_label(state)
                    );
                    grid.spawn(super::widgets::musaic_button(
                        Node {
                            grid_row: GridPlacement::start(row),
                            grid_column: GridPlacement::start(col),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            flex_direction: FlexDirection::Column,
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(4)),
                            ..default()
                        },
                        (
                            InspectorButtonAction(EditorCommand::BindOutputSide {
                                node: node.clone(),
                                side,
                            }),
                            super::widgets::ButtonAccessibilityLabel(label),
                        ),
                        symbol,
                    ))
                    .remove::<bevy_feathers::theme::ThemeFontColor>()
                    .insert((
                        TextFont {
                            font_size: 23.0,
                            ..default()
                        },
                        TextColor(if state == PortSlotState::Input {
                            theme.semantic.atom_scalar
                        } else {
                            theme.semantic.atom_note
                        }),
                        BackgroundColor(if state == PortSlotState::None {
                            theme.chrome.panel_bg
                        } else {
                            theme.chrome.crumb_selected_bg
                        }),
                        BorderColor::all(theme.chrome.border),
                    ))
                    .observe(super::on_inspector_button_activated);
                }
                grid.spawn((
                    Node {
                        grid_row: GridPlacement::start(2),
                        grid_column: GridPlacement::start(2),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(theme.chrome.panel_bg),
                    BorderColor::all(theme.semantic.atom_scalar),
                ))
                .with_children(|tile| {
                    tile.spawn((
                        crate::infrastructure::ui::inspector::panels::tile_inspect::selected_tile_glyph(node),
                                    Text::new(glyph),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(theme.semantic.atom_scalar),
                    ));
                });
            });
            card.spawn((
                Text::new("[ ] Input    → Output    · Unused"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(theme.chrome.text_dim),
            ));
        });
}

fn side_label(side: SpatialSide) -> &'static str {
    match side {
        SpatialSide::North => "North",
        SpatialSide::East => "East",
        SpatialSide::South => "South",
        SpatialSide::West => "West",
        SpatialSide::Off => "Off",
    }
}
fn port_state_label(state: PortSlotState) -> &'static str {
    match state {
        PortSlotState::None => "Unused",
        PortSlotState::Input => "Input",
        PortSlotState::Output => "Output",
    }
}
