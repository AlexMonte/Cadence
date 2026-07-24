//! Dynamic text labels on tile surfaces (scalars, compounds, transform aux values).

use std::collections::BTreeMap;

use bevy::prelude::*;
use tessera::prelude::NodeId;

use crate::application::pipeline::scene_sync::VisibleBoardState;
use crate::infrastructure::ui::{
    board_3d::Board3dTile, musaic_tile::TILE_WORLD_HEIGHT, render_layers::SCENE_NODE_VISIBILITY,
};

const LABEL_LIFT: f32 = 0.06;
const LABEL_FONT_SIZE: f32 = 0.14;

#[derive(Component)]
pub struct TileSurfaceLabel {
    pub node: NodeId,
}

pub struct TileSurfacePlugin;

impl Plugin for TileSurfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_tile_surface_labels.run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
    }
}

fn sync_tile_surface_labels(
    mut commands: Commands,
    visible: Res<VisibleBoardState>,
    tiles: Query<(
        Entity,
        &Board3dTile,
        Option<&TileSurfaceLabel>,
        Option<&Children>,
    )>,
    labels: Query<(Entity, &TileSurfaceLabel)>,
    mut cache: Local<BTreeMap<NodeId, String>>,
) {
    let mut desired = BTreeMap::<NodeId, String>::new();
    for node in &visible.nodes {
        if let Some(display) = node.surface_content.display() {
            desired.insert(node.node.clone(), display);
        }
    }

    if *cache == desired {
        return;
    }
    *cache = desired.clone();

    for (entity, label) in labels.iter() {
        if !desired.contains_key(&label.node) {
            commands.entity(entity).despawn();
        }
    }

    for (tile_entity, tile, existing_label, children) in tiles.iter() {
        let Some(display) = desired.get(&tile.node) else {
            if let Some(children) = children {
                for child in children.iter() {
                    if labels.get(child).is_ok() {
                        commands.entity(child).despawn();
                    }
                }
            }
            continue;
        };

        if let Some(label) = existing_label {
            if let Some(children) = children {
                for child in children.iter() {
                    if let Ok((label_entity, _)) = labels.get(child) {
                        commands
                            .entity(label_entity)
                            .insert(Text2d::new(display.clone()));
                    }
                }
            }
            let _ = label;
            continue;
        }

        commands.entity(tile_entity).with_children(|parent| {
            parent.spawn((
                TileSurfaceLabel {
                    node: tile.node.clone(),
                },
                Text2d::new(display.clone()),
                TextFont {
                    font_size: LABEL_FONT_SIZE,
                    ..default()
                },
                TextColor(Color::WHITE),
                SCENE_NODE_VISIBILITY,
                Transform::from_translation(Vec3::new(
                    0.0,
                    TILE_WORLD_HEIGHT * 0.5 + LABEL_LIFT,
                    0.0,
                )),
            ));
        });
    }
}
