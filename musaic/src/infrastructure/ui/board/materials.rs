use std::collections::BTreeMap;

use bevy::prelude::*;
use tessera::prelude::NodeId;

use crate::domain::board::{BoardSurfaceId, SurfaceLayoutKind};
use crate::domain::document::StackIndex;
use crate::infrastructure::ui::theme::MusaicUiTheme;

use super::components::{ConnectionVisualKey, TileVisualKey};

#[derive(Resource, Default)]
pub(super) struct Board3dRenderCache {
    pub(super) root: Option<Entity>,
    pub(super) active_surface: Option<BoardSurfaceId>,
    pub(super) layout: SurfaceLayoutKind,
    pub(super) stack_columns: usize,
    pub(super) tiles: BTreeMap<NodeId, (Entity, TileVisualKey)>,
    pub(super) connections: BTreeMap<(NodeId, NodeId), (Entity, ConnectionVisualKey)>,
    pub(super) stack_inserts: BTreeMap<StackIndex, Entity>,
    pub(super) stack_locked: BTreeMap<StackIndex, Entity>,
}

#[derive(Resource)]
pub(super) struct Board3dMaterials {
    pub(super) board: Handle<StandardMaterial>,
    pub(super) surface: Handle<StandardMaterial>,
    pub(super) grid_line: Handle<StandardMaterial>,
    pub(super) focused: Handle<StandardMaterial>,
    pub(super) locked_slot: Handle<StandardMaterial>,
    pub(super) connection_scalar: Handle<StandardMaterial>,
    pub(super) connection_control: Handle<StandardMaterial>,
    pub(super) connection_preview: Handle<StandardMaterial>,
    pub(super) drag_preview: Handle<StandardMaterial>,
}

impl FromWorld for Board3dMaterials {
    fn from_world(world: &mut World) -> Self {
        let colors = world
            .get_resource::<MusaicUiTheme>()
            .expect("MusaicUiTheme must be inserted before Board3dMaterials")
            .board;
        let glyphs = world.resource::<AssetServer>().load("ui/tile-glyphs.png");
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();

        Self {
            surface: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(glyphs),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            board: materials.add(StandardMaterial {
                base_color: colors.board,
                unlit: true,
                perceptual_roughness: 1.0,
                ..default()
            }),
            grid_line: materials.add(StandardMaterial {
                base_color: colors.grid_line,
                unlit: true,
                emissive: LinearRgba::rgb(0.005, 0.006, 0.01),
                perceptual_roughness: 0.95,
                ..default()
            }),
            focused: materials.add(StandardMaterial {
                base_color: colors.focused,
                emissive: LinearRgba::rgb(0.02, 0.12, 0.16),
                perceptual_roughness: 0.8,
                ..default()
            }),
            locked_slot: materials.add(StandardMaterial {
                base_color: colors.locked_slot,
                perceptual_roughness: 1.0,
                ..default()
            }),
            connection_scalar: materials.add(StandardMaterial {
                base_color: colors.connection_scalar,
                unlit: true,
                emissive: LinearRgba::rgb(0.04, 0.14, 0.18),
                perceptual_roughness: 0.85,
                ..default()
            }),
            connection_control: materials.add(StandardMaterial {
                base_color: colors.connection_control,
                unlit: true,
                emissive: LinearRgba::rgb(0.16, 0.08, 0.02),
                perceptual_roughness: 0.85,
                ..default()
            }),
            connection_preview: materials.add(StandardMaterial {
                base_color: colors.connection_preview.with_alpha(0.75),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                perceptual_roughness: 0.7,
                ..default()
            }),
            drag_preview: materials.add(StandardMaterial {
                base_color: colors.drag_preview,
                alpha_mode: AlphaMode::Blend,
                emissive: LinearRgba::rgb(0.04, 0.05, 0.08),
                perceptual_roughness: 0.75,
                ..default()
            }),
        }
    }
}
