//! Board 3D viewport: VisibleBoardState → entities.
//!
//! Modules:
//! - [`scene_reconcile`] — keyed NodeId sync
//! - [`picking`] — MeshPicking → BoardPickEvent via pick_at
//! - [`placement_ghost`] — drag preview + slot highlight
//! - [`grid`] — backdrop / anchor
//! - [`plugin`] — system order

mod components;
mod grid;
mod helpers;
mod materials;
mod picking;
mod placement_ghost;
mod plugin;
mod scene_reconcile;

pub use components::{
    Board3dCamera, Board3dRoot, Board3dTile, BoardGridAnchor, BoardGridPickSlab, BoardPickTarget,
    BoardTileId,
};
pub use plugin::Board3dPlugin;

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use tessera::prelude::NodeId;

    use super::components::BoardDragSurface;
    use super::*;
    use crate::application::command::{EditorCommand, EditorCommandBus};
    use crate::application::editor::{SelectionMode, SelectionState, transaction::PlacementTarget};
    use crate::application::pipeline::PlaybackPlugin;
    use crate::application::pipeline::scene_sync::VisibleBoardState;
    use crate::application::session::MusaicProject;
    use crate::domain::board::{BoardSlot, BoardSurfaceId};
    use crate::domain::document::{ContainerKind, PlacementAddress, TileSpawnKind};
    use crate::infrastructure::app::{AppState, TransportMode};
    use crate::infrastructure::ui::tile_icons::TileIconAssets;
    use crate::infrastructure::ui::transform_tile::TransformTileAssets;
    use tessera::bevy::TesseraPlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_state::<TransportMode>()
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .init_resource::<TransformTileAssets>()
            .init_resource::<TileIconAssets>()
            .init_resource::<super::super::board_camera_nav::BoardCameraPointerState>()
            .init_resource::<super::super::camera_rig::BoardCameraRig>()
            .add_plugins((
                TesseraPlugin,
                crate::application::editor::EditorPlugin,
                PlaybackPlugin,
            ))
            .add_plugins(Board3dPlugin);
        app.insert_state(AppState::Editor);
        app
    }

    fn place_container(app: &mut App, surface: BoardSurfaceId, slot: BoardSlot) {
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot { surface, slot },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            }));
        app.update();
        app.update();
    }

    fn tile_entity(world: &mut World, node: &NodeId) -> Entity {
        world
            .query::<(Entity, &Board3dTile)>()
            .iter(world)
            .find(|(_, tile)| &tile.node == node)
            .map(|(entity, _)| entity)
            .expect("board tile entity for node")
    }

    fn board_root(world: &mut World) -> Entity {
        world
            .query_filtered::<Entity, With<Board3dRoot>>()
            .iter(world)
            .next()
            .expect("Board3dRoot")
    }

    #[test]
    fn entering_editor_with_empty_project_spawns_board_grid_backdrop() {
        let mut app = test_app();
        app.update();
        app.update();

        let visible = app.world().resource::<VisibleBoardState>();
        assert!(
            visible.active_surface.is_some(),
            "scene sync should activate the root surface for an empty project"
        );
        let active_surface = visible.active_surface.unwrap();

        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Board3dRoot>>()
            .iter(world)
            .next()
            .expect("entering Editor should spawn a Board3dRoot");
        world
            .query_filtered::<Entity, With<BoardGridAnchor>>()
            .iter(world)
            .next()
            .expect("entering Editor should spawn the persistent grid backdrop");
        let slab = world
            .query_filtered::<(&Mesh3d, &BoardDragSurface, &Pickable), With<BoardGridPickSlab>>()
            .iter(world)
            .next()
            .expect("grid backdrop should carry a pickable slab");
        assert_eq!(
            slab.1.surface, active_surface,
            "pick slab should be bound to the active root surface"
        );
    }

    #[test]
    fn selection_toggle_keeps_unrelated_tile_entity_stable() {
        let mut app = test_app();
        app.update();
        app.update();

        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;

        place_container(&mut app, root_surface, BoardSlot::new(0, 0));
        let first = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();

        place_container(&mut app, root_surface, BoardSlot::new(2, 0));
        let second = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        assert_ne!(first, second);

        let first_entity_before = tile_entity(app.world_mut(), &first);
        let second_entity_before = tile_entity(app.world_mut(), &second);
        let root_before = board_root(app.world_mut());

        // Select the second tile; first tile's visual key is unchanged.
        app.world_mut()
            .resource_mut::<SelectionState>()
            .select(second.clone(), SelectionMode::Replace);
        app.update();
        app.update();

        assert_eq!(board_root(app.world_mut()), root_before);
        assert_eq!(tile_entity(app.world_mut(), &first), first_entity_before);
        // Selected tile may respawn for highlight materials — that is fine.
        let _ = second_entity_before;
        assert!(
            app.world()
                .resource::<VisibleBoardState>()
                .nodes
                .iter()
                .any(|n| n.node == second && n.selected),
            "selection should project onto visible board"
        );
    }

    #[test]
    fn tile_address_change_keeps_board_root_stable() {
        let mut app = test_app();
        app.update();
        app.update();

        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;
        place_container(&mut app, root_surface, BoardSlot::new(0, 0));
        let node = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        let root_before = board_root(app.world_mut());

        {
            let mut visible = app.world_mut().resource_mut::<VisibleBoardState>();
            if let Some(entry) = visible.nodes.iter_mut().find(|n| n.node == node) {
                entry.address = PlacementAddress::BoardSlot(BoardSlot::new(2, 1));
            }
        }
        app.update();
        app.update();

        assert_eq!(board_root(app.world_mut()), root_before);
        let tile = app
            .world_mut()
            .query::<&Board3dTile>()
            .iter(app.world())
            .find(|t| t.node == node)
            .expect("moved tile still present");
        assert_eq!(
            tile.address,
            PlacementAddress::BoardSlot(BoardSlot::new(2, 1))
        );
    }
}
