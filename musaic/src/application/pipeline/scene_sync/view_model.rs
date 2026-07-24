use bevy::prelude::*;
use tessera::prelude::NodeId;

use super::{
    logic::needs_visible_board_rebuild,
    project_board_scene,
    surface_content::surface_content_for_node,
    types::{
        RenderBoardFocus, VisibleAtomCompound, VisibleBoardConnection, VisibleBoardNode,
        VisibleNodeKind,
    },
};
use crate::{
    application::editor::{
        AtomCompoundSemantic, AtomCompoundView, EditorAttention, PickHit, SelectionState,
    },
    application::session::MusaicProject,
    domain::board::{BoardSlot, BoardSurfaceId, SurfaceLayoutKind},
    domain::document::{DocumentQueries, PlacementAddress, StackIndex},
};

#[derive(Resource, Debug, Clone, Default)]
pub struct VisibleBoardState {
    pub active_surface: Option<BoardSurfaceId>,
    pub layout: SurfaceLayoutKind,
    pub nodes: Vec<VisibleBoardNode>,
    pub atom_compounds: Vec<VisibleAtomCompound>,
    pub stack_inserts: Vec<StackIndex>,
    pub stack_locked_slots: Vec<StackIndex>,
    pub connections: Vec<VisibleBoardConnection>,
    pub focus: Option<RenderBoardFocus>,
}

impl VisibleBoardState {
    pub fn pick_at(&self, slot: BoardSlot) -> Option<PickHit> {
        if let Some(compound) = self
            .atom_compounds
            .iter()
            .find(|compound| compound.slot == slot)
        {
            let primary_node = compound.compound.members.first()?.clone();
            return Some(PickHit::AtomCompound {
                primary_node,
                members: compound.compound.members.clone(),
            });
        }

        if let Some(node) = self.nodes.iter().find(|node| {
            let anchor = visual_slot_from_address(node.address);
            node.tessera_footprint
                .is_some_and(|footprint| footprint.occupies(anchor, slot))
                || node.tessera_footprint.is_none() && anchor == slot
        }) {
            return Some(PickHit::Tile {
                node: node.node.clone(),
            });
        }

        self.active_surface.map(|surface| match self.layout {
            SurfaceLayoutKind::Board => PickHit::EmptySlot { surface, slot },
            SurfaceLayoutKind::Stack => PickHit::StackInsert {
                surface,
                index: self
                    .stack_inserts
                    .iter()
                    .find(|insert| insert.0 == slot.x.max(0) as usize)
                    .copied()
                    .unwrap_or(StackIndex(slot.x.max(0) as usize)),
            },
        })
    }
}

fn visual_slot_from_address(address: PlacementAddress) -> BoardSlot {
    match address {
        PlacementAddress::BoardSlot(slot) => slot,
        PlacementAddress::StackIndex(index) => BoardSlot::new(index.0 as i32, 0),
    }
}

pub(super) fn rebuild_visible_board_state(
    project: Res<'_, MusaicProject>,
    runtime: Res<'_, crate::application::pipeline::runtime::RuntimeState>,
    attention: Res<'_, EditorAttention>,
    selection: Res<'_, SelectionState>,
    view_settings: Res<'_, crate::application::board_view_settings::BoardViewSettings>,
    mut visible: ResMut<'_, VisibleBoardState>,
) {
    if !needs_visible_board_rebuild(
        runtime.dirty.scene,
        attention.is_changed(),
        selection.is_changed(),
        visible.active_surface,
        attention.active_board(),
    ) && !view_settings.is_changed()
    {
        return;
    }

    let queries = DocumentQueries::new(&project.document);
    let scene = project_board_scene(&queries, &attention, &selection, *view_settings);

    let icon_for_node = |node_id: &NodeId| -> Option<crate::adapter::tile_icons::TileIconId> {
        match queries.node_kind(node_id)? {
            crate::domain::document::DocumentNodeKind::TrickInstance(trick) => Some(
                crate::adapter::tile_icons::icon_for_trick_prototype(trick.prototype),
            ),
            _ => None,
        }
    };
    visible.active_surface = Some(scene.surface);
    visible.layout = scene.layout;

    let mut compound_by_member = std::collections::BTreeMap::new();
    for compound in &scene.compounds {
        if let Some(anchor) = compound.members.first() {
            compound_by_member.insert(anchor.clone(), compound.display.clone());
        }
    }
    let compound_members: std::collections::BTreeSet<_> = scene
        .compounds
        .iter()
        .flat_map(|compound| compound.members.iter().cloned())
        .collect();

    visible.nodes = scene
        .tiles
        .into_iter()
        .map(|tile| VisibleBoardNode {
            node: tile.node.clone(),
            address: tile.address,
            tessera_footprint: tile.tessera_footprint,
            kind: match tile.visual_kind {
                super::TileVisualKind::Container(_) => VisibleNodeKind::Container,
                super::TileVisualKind::Atom => VisibleNodeKind::Atom,
                super::TileVisualKind::Output => VisibleNodeKind::Output,
                super::TileVisualKind::Trick => VisibleNodeKind::TrickInstance,
                super::TileVisualKind::Generic => VisibleNodeKind::Tile,
            },
            selected: tile.selected,
            focused: tile.focused,
            icon: icon_for_node(&tile.node),
            surface_content: surface_content_for_node(
                &queries,
                &tile.node,
                &compound_by_member,
                &compound_members,
            ),
        })
        .collect();
    visible.atom_compounds = scene
        .compounds
        .into_iter()
        .map(|compound| VisibleAtomCompound {
            slot: compound.anchor_slot,
            compound: AtomCompoundView {
                members: compound.members,
                display: compound.display,
                semantic: AtomCompoundSemantic::Single,
            },
        })
        .collect();
    visible.stack_inserts = scene.stack_inserts;
    visible.stack_locked_slots = scene.stack_locked_slots;
    visible.connections = scene
        .connections
        .into_iter()
        .map(|connection| VisibleBoardConnection {
            from: connection.from,
            to: connection.to,
            from_slot: connection.from_slot,
            to_slot: connection.to_slot,
            kind: connection.kind,
        })
        .collect();
    visible.focus = scene.focus;
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::application::pipeline::scene_sync::TileSurfaceContent;
    use crate::{
        application::command::{EditorCommand, execute_command},
        application::editor::{EditorAttention, SelectionState, transaction::PlacementTarget},
        application::pipeline::PlaybackPlugin,
        application::pipeline::runtime::TimelineProvenanceStore,
        application::session::MusaicProject,
        domain::board::{BoardSlot, BoardSurfaceId},
        domain::document::{AtomValue, ContainerKind, NoteName, TileSpawnKind},
        infrastructure::app::{AppState, TransportMode},
    };
    use tessera::bevy::{TesseraBoard, TesseraPlugin};

    #[test]
    fn visible_board_compounds_atom_row_into_a2() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_state::<TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((
                TesseraPlugin,
                crate::application::editor::EditorPlugin,
                PlaybackPlugin,
            ));
        app.insert_state(AppState::Editor);
        app.world_mut()
            .resource_mut::<crate::application::board_view_settings::BoardViewSettings>()
            .container_interior =
            crate::application::board_view_settings::AtomDisplayMode::CompoundTile;

        let provenance = TimelineProvenanceStore::default();
        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;
        app.world_mut()
            .resource_scope(|world, mut project: Mut<'_, MusaicProject>| {
                world.resource_scope(|world, mut board: Mut<'_, TesseraBoard>| {
                    world.resource_scope(|world, mut attention: Mut<'_, EditorAttention>| {
                        world.resource_scope(|_world, mut selection: Mut<'_, SelectionState>| {
                            let _ = execute_command(
                                &mut project.document,
                                &mut board,
                                &mut attention,
                                &mut selection,
                                &provenance,
                                &EditorCommand::PlaceTile {
                                    target: PlacementTarget::BoardSlot {
                                        surface: root_surface,
                                        slot: BoardSlot::new(0, 0),
                                    },
                                    tile: TileSpawnKind::Container {
                                        kind: ContainerKind::Sequence,
                                    },
                                },
                            );
                            let container = selection.nodes.iter().next().unwrap().clone();
                            let local_surface = project
                                .document
                                .graph
                                .container_surface(&container)
                                .unwrap();

                            let _ = execute_command(
                                &mut project.document,
                                &mut board,
                                &mut attention,
                                &mut selection,
                                &provenance,
                                &EditorCommand::EnterContainer { container },
                            );

                            let _ = execute_command(
                                &mut project.document,
                                &mut board,
                                &mut attention,
                                &mut selection,
                                &provenance,
                                &EditorCommand::PlaceTile {
                                    target: PlacementTarget::BoardSlot {
                                        surface: local_surface,
                                        slot: BoardSlot::new(0, 0),
                                    },
                                    tile: TileSpawnKind::Atom {
                                        atom: AtomValue::NoteName(NoteName::A),
                                    },
                                },
                            );

                            let _ = execute_command(
                                &mut project.document,
                                &mut board,
                                &mut attention,
                                &mut selection,
                                &provenance,
                                &EditorCommand::PlaceTile {
                                    target: PlacementTarget::BoardSlot {
                                        surface: local_surface,
                                        slot: BoardSlot::new(1, 0),
                                    },
                                    tile: TileSpawnKind::Atom {
                                        atom: AtomValue::Octave(2),
                                    },
                                },
                            );
                        });
                    });
                });
            });
        app.world_mut()
            .resource_mut::<crate::application::pipeline::runtime::RuntimeState>()
            .dirty
            .scene = true;

        app.update();

        let visible = app.world().resource::<VisibleBoardState>();
        assert_eq!(visible.active_surface, Some(BoardSurfaceId(1)));
        assert_eq!(visible.atom_compounds.len(), 1);
        assert_eq!(visible.atom_compounds[0].compound.display, "A2");
    }

    #[test]
    fn pick_at_prefers_atom_compound_over_underlying_atom_tile() {
        let visible = VisibleBoardState {
            active_surface: Some(BoardSurfaceId(7)),
            layout: SurfaceLayoutKind::Board,
            nodes: vec![VisibleBoardNode {
                node: NodeId::new("atom_a"),
                address: PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                tessera_footprint: None,
                kind: VisibleNodeKind::Atom,
                selected: false,
                focused: false,
                icon: None,
                surface_content: TileSurfaceContent::Empty,
            }],
            atom_compounds: vec![VisibleAtomCompound {
                slot: BoardSlot::new(0, 0),
                compound: AtomCompoundView {
                    members: vec![NodeId::new("atom_a"), NodeId::new("atom_oct")],
                    display: "A2".into(),
                    semantic: AtomCompoundSemantic::Pitch,
                },
            }],
            stack_inserts: Vec::new(),
            stack_locked_slots: Vec::new(),
            connections: Vec::new(),
            focus: None,
        };

        assert_eq!(
            visible.pick_at(BoardSlot::new(0, 0)),
            Some(PickHit::AtomCompound {
                primary_node: NodeId::new("atom_a"),
                members: vec![NodeId::new("atom_a"), NodeId::new("atom_oct")],
            })
        );
    }

    #[test]
    fn pick_at_hits_secondary_cell_of_footprinted_tile() {
        let container = NodeId::new("container");
        let visible = VisibleBoardState {
            active_surface: Some(BoardSurfaceId(7)),
            layout: SurfaceLayoutKind::Board,
            nodes: vec![VisibleBoardNode {
                node: container.clone(),
                address: PlacementAddress::BoardSlot(BoardSlot::new(3, 4)),
                tessera_footprint: Some(tessera::prelude::TileFootprint::new(2, 2)),
                kind: VisibleNodeKind::Container,
                selected: false,
                focused: false,
                icon: None,
                surface_content: TileSurfaceContent::Empty,
            }],
            ..Default::default()
        };

        assert_eq!(
            visible.pick_at(BoardSlot::new(4, 5)),
            Some(PickHit::Tile { node: container })
        );
    }
}
