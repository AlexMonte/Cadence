//! Port compass glyphs on board tiles (3D billboards) and in the inspector panel.

use bevy::{math::primitives::Cuboid, prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;
use tessera::prelude::{NodeId, SpatialSide};

use crate::{
    adapter::load_up::{UiSpriteAssets, pixel_ui_display_size},
    application::command::EditorCommand,
    application::editor::interaction::{BoardPickEvent, BoardPickHit, BoardPickTargetKind},
    application::editor::{
        ConnectionEndpointView, CursorInteraction, EditorSession, PortSlotState,
        blocks_board_picks_with_cursor,
    },
    domain::board::BoardSurfaceId,
    infrastructure::ui::{
        InspectorButtonAction,
        board::BoardPickTarget,
        render_layers::SCENE_NODE_VISIBILITY,
        ui_sprites::{self, port_slot_image},
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
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
    node: &NodeId,
    view: &ConnectionEndpointView,
) {
    parent
        .spawn(Node {
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
            grid_template_rows: RepeatedGridTrack::flex(3, 1.0),
            row_gap: px(4.0),
            column_gap: px(4.0),
            margin: UiRect::top(px(8.0)),
            ..default()
        })
        .with_children(|grid| {
            grid.spawn(Node::default());
            if let Some(sprites) = sprites {
                spawn_inspector_port_button(
                    grid,
                    images,
                    Some(sprites),
                    node,
                    SpatialSide::North,
                    view.north,
                );
            }
            grid.spawn(Node::default());

            if let Some(sprites) = sprites {
                spawn_inspector_port_button(
                    grid,
                    images,
                    Some(sprites),
                    node,
                    SpatialSide::West,
                    view.west,
                );
            }
            grid.spawn(Node::default());
            if let Some(sprites) = sprites {
                spawn_inspector_port_button(
                    grid,
                    images,
                    Some(sprites),
                    node,
                    SpatialSide::East,
                    view.east,
                );
            }

            grid.spawn(Node::default());
            if let Some(sprites) = sprites {
                spawn_inspector_port_button(
                    grid,
                    images,
                    Some(sprites),
                    node,
                    SpatialSide::South,
                    view.south,
                );
            }
            grid.spawn(Node::default());
        });
}

fn spawn_inspector_port_button(
    parent: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
    node: &NodeId,
    side: SpatialSide,
    state: PortSlotState,
) {
    let label = side_label(side);
    parent
        .spawn((super::controls::musaic_clickable(
            Node {
                min_width: px(72.0),
                min_height: px(36.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(2.0),
                padding: UiRect::all(px(4.0)),
                ..default()
            },
            InspectorButtonAction(EditorCommand::BindOutputSide {
                node: node.clone(),
                side,
            }),
            format!("{label} port"),
        ),))
        .observe(super::on_inspector_button_activated)
        .with_children(|btn| {
            if let Some(sprites) = sprites {
                let handle = port_slot_image(sprites, state);
                if let Some(size) = pixel_ui_display_size(images, handle) {
                    btn.spawn((
                        ui_sprites::image_node(handle),
                        Node {
                            width: px(size.x),
                            height: px(size.y),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                }
            }
            btn.spawn((
                UiText::new(format!("{label} · {}", port_state_label(state))),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                ThemedText,
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
        PortSlotState::None => "None",
        PortSlotState::Input => "Input",
        PortSlotState::Output => "Output",
    }
}
