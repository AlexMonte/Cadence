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
    pub(super) ortho_tiles_ready: bool,
    pub(super) tile_icons_ready: bool,
    pub(super) tile_meshes_ready: bool,
    pub(super) tiles: BTreeMap<NodeId, (Entity, TileVisualKey)>,
    pub(super) connections: BTreeMap<(NodeId, NodeId), (Entity, ConnectionVisualKey)>,
    pub(super) stack_inserts: BTreeMap<StackIndex, Entity>,
    pub(super) stack_locked: BTreeMap<StackIndex, Entity>,
}

#[derive(Resource)]
pub(super) struct Board3dMaterials {
    pub(super) board: Handle<StandardMaterial>,
    pub(super) grid_line: Handle<StandardMaterial>,
    pub(super) container: Handle<StandardMaterial>,
    pub(super) atom: Handle<StandardMaterial>,
    pub(super) output: Handle<StandardMaterial>,
    pub(super) trick: Handle<StandardMaterial>,
    pub(super) generic: Handle<StandardMaterial>,
    pub(super) selected: Handle<StandardMaterial>,
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
            .map(|t| t.board)
            .unwrap_or_else(|| MusaicUiTheme::default_dark().board);
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();

        Self {
            board: materials.add(StandardMaterial {
                base_color: colors.board,
                perceptual_roughness: 1.0,
                ..default()
            }),
            grid_line: materials.add(StandardMaterial {
                base_color: colors.grid_line,
                emissive: LinearRgba::rgb(0.005, 0.006, 0.01),
                perceptual_roughness: 0.95,
                ..default()
            }),
            container: materials.add(StandardMaterial {
                base_color: colors.container,
                perceptual_roughness: 0.85,
                ..default()
            }),
            atom: materials.add(StandardMaterial {
                base_color: colors.atom,
                perceptual_roughness: 0.85,
                ..default()
            }),
            output: materials.add(StandardMaterial {
                base_color: colors.output,
                perceptual_roughness: 0.85,
                ..default()
            }),
            trick: materials.add(StandardMaterial {
                base_color: colors.trick,
                perceptual_roughness: 0.85,
                ..default()
            }),
            generic: materials.add(StandardMaterial {
                base_color: colors.generic,
                perceptual_roughness: 0.85,
                ..default()
            }),
            selected: materials.add(StandardMaterial {
                base_color: colors.selected,
                emissive: LinearRgba::rgb(0.18, 0.12, 0.02),
                perceptual_roughness: 0.8,
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
                emissive: LinearRgba::rgb(0.04, 0.14, 0.18),
                perceptual_roughness: 0.85,
                ..default()
            }),
            connection_control: materials.add(StandardMaterial {
                base_color: colors.connection_control,
                emissive: LinearRgba::rgb(0.16, 0.08, 0.02),
                perceptual_roughness: 0.85,
                ..default()
            }),
            connection_preview: materials.add(StandardMaterial {
                base_color: colors.connection_preview,
                alpha_mode: AlphaMode::Blend,
                emissive: LinearRgba::rgb(0.05, 0.13, 0.2),
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
