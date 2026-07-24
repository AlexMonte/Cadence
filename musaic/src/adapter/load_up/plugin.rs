use bevy::prelude::*;
use load_up::prelude::*;

use crate::adapter::load_up::sampler::{NearestSamplerApplied, ensure_nearest_samplers};
use crate::adapter::load_up::tile_assets::{
    AtomTileAssets, BoardTileIconAssets, OrthoTileAssets, TileMeshSourceAssets,
};
use crate::adapter::load_up::ui_assets::UiSpriteAssets;
use crate::infrastructure::app::AppState;

pub struct LoadUpMusaicPlugin;

impl Plugin for LoadUpMusaicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NearestSamplerApplied>()
            .add_plugins(LoadUpPlugin::<AppState>::default())
            .require_resource_for_state::<UiSpriteAssets>(AppState::Editor)
            .require_resource_for_state::<OrthoTileAssets>(AppState::Editor)
            .require_resource_for_state::<BoardTileIconAssets>(AppState::Editor)
            .require_resource_for_state::<AtomTileAssets>(AppState::Editor)
            .require_resource_for_state::<TileMeshSourceAssets>(AppState::Editor)
            .add_systems(
                Update,
                (apply_nearest_ui_samplers, apply_nearest_tile_samplers)
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

fn apply_nearest_ui_samplers(
    ui: Option<Res<UiSpriteAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut applied: ResMut<NearestSamplerApplied>,
) {
    let Some(ui) = ui else {
        return;
    };
    ensure_nearest_samplers(&mut images, &mut applied, ui.all_handles());
}

fn apply_nearest_tile_samplers(
    ortho: Option<Res<OrthoTileAssets>>,
    icons: Option<Res<BoardTileIconAssets>>,
    atoms: Option<Res<AtomTileAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut applied: ResMut<NearestSamplerApplied>,
) {
    let (Some(ortho), Some(icons), Some(atoms)) = (ortho, icons, atoms) else {
        return;
    };
    let mut handles = vec![
        ortho.transform_ortho.clone(),
        ortho.container_ortho.clone(),
        ortho.output_ortho.clone(),
        ortho.atom_note_ortho.clone(),
        ortho.atom_scalar_ortho.clone(),
        ortho.atom_accidental_ortho.clone(),
        ortho.atom_operator_ortho.clone(),
        icons.container_sequence.clone(),
        icons.container_parallel.clone(),
        icons.container_alternate.clone(),
        icons.output_tile.clone(),
        icons.transform_fast.clone(),
        icons.transform_slow.clone(),
        icons.transform_legato.clone(),
        icons.transform_gain.clone(),
    ];
    handles.extend(atoms.all_handles());
    ensure_nearest_samplers(&mut images, &mut applied, handles);
}
