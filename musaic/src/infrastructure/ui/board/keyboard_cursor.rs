//! A persistent outline makes empty keyboard insertion positions visible.
use crate::{
    application::{
        editor::{EditorAttention, FocusTarget},
        pipeline::scene_sync::VisibleBoardState,
    },
    domain::{board::SLOT_SIZE, document::PlacementAddress},
    infrastructure::{
        app::{AppState, MusaicSet},
        ui::{board_geometry::tessera_slot_center, render_layers::BOARD_VIEW},
    },
};
use bevy::prelude::*;
#[derive(Component)]
struct EmptyCellCursor;

pub(super) fn register(app: &mut App) {
    app.add_systems(OnEnter(AppState::Editor), spawn)
        .add_systems(
            Update,
            sync.after(MusaicSet::SceneSync)
                .run_if(in_state(AppState::Editor)),
        );
}
fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<super::materials::Board3dMaterials>,
) {
    let mesh = meshes.add(Cuboid::new(1.0, 0.02, 1.0));
    let size = SLOT_SIZE * 0.92;
    commands
        .spawn((
            EmptyCellCursor,
            DespawnOnExit(AppState::Editor),
            Transform::default(),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            for (x, z, sx, sz) in [
                (0., -size / 2., size, 0.025),
                (0., size / 2., size, 0.025),
                (-size / 2., 0., 0.025, size),
                (size / 2., 0., 0.025, size),
            ] {
                root.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(materials.focused.clone()),
                    BOARD_VIEW,
                    Transform::from_xyz(x, 0., z).with_scale(Vec3::new(sx, 1., sz)),
                    Pickable::IGNORE,
                ));
            }
        });
}
fn sync(
    attention: Res<EditorAttention>,
    visible: Res<VisibleBoardState>,
    focus: Option<Res<bevy::input_focus::InputFocus>>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
    modals: Query<
        (),
        With<crate::application::editor::interaction::keyboard_navigation::KeyboardModal>,
    >,
    mut cursor: Query<(&mut Transform, &mut Visibility), With<EmptyCellCursor>>,
) {
    let Ok((mut transform, mut visibility)) = cursor.single_mut() else {
        return;
    };
    let position = match &attention.focus {
        FocusTarget::EmptySlot { slot, .. } => Some(tessera_slot_center(
            *slot,
            tessera::prelude::TileFootprint::new(1, 1),
            0.14,
        )),
        FocusTarget::StackInsert { index, .. } => {
            match visible.display_address(PlacementAddress::StackIndex(*index)) {
                PlacementAddress::StackIndex(index) => {
                    Some(super::helpers::stack_insert_marker_center(index))
                }
                _ => None,
            }
        }
        _ => None,
    };
    let available = modals.is_empty()
        && focus
            .as_ref()
            .is_none_or(|focus| focus.0.is_none_or(|entity| windows.contains(entity)));
    if let Some(position) = position.filter(|_| available) {
        transform.translation = position;
        *visibility = Visibility::Visible;
    } else {
        *visibility = Visibility::Hidden;
    }
}
