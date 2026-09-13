use bevy::prelude::*;
use tessera::prelude::NodeId;

use super::{
    logic::needs_visible_board_rebuild,
    projection::project_visible_board,
    types::{
        RenderBoardFocus, VisibleAtomCompound, VisibleBoardConnection, VisibleBoardNode,
        VisibleNodeKind,
    },
};
use crate::{
    application::editor::{EditorAttention, PickHit, SelectionState},
    application::session::MusaicProject,
    domain::board::{BoardSlot, BoardSurfaceId, SurfaceLayoutKind},
    domain::document::{PlacementAddress, StackIndex},
};

#[derive(Resource, Debug, Clone, Default)]
pub struct VisibleBoardState {
    pub active_surface: Option<BoardSurfaceId>,
    pub layout: SurfaceLayoutKind,
    pub nodes: Vec<VisibleBoardNode>,
    pub atom_compounds: Vec<VisibleAtomCompound>,
    pub stack_inserts: Vec<StackIndex>,
    pub stack_locked_slots: Vec<StackIndex>,
    pub stack_display: super::types::StackDisplayMap,
    pub connections: Vec<VisibleBoardConnection>,
    pub focus: Option<RenderBoardFocus>,
}

impl VisibleBoardState {
    /// Translate only presentation coordinates; authored document addresses stay intact.
    pub fn display_address(&self, address: PlacementAddress) -> PlacementAddress {
        self.stack_display.display_address(address)
    }

    pub fn display_address_of(&self, node: &NodeId) -> Option<PlacementAddress> {
        self.address_of(node)
            .map(|address| self.display_address(address))
    }

    /// Placement address for a projected node, if it is on the visible surface.
    pub fn address_of(&self, node: &NodeId) -> Option<PlacementAddress> {
        self.nodes
            .iter()
            .find(|n| &n.node == node)
            .map(|n| n.address)
    }

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
            if node.kind == VisibleNodeKind::Atom
                && matches!(
                    node.surface_content,
                    super::types::TileSurfaceContent::Empty
                )
            {
                return false;
            }
            let anchor = visual_slot_from_address(self.display_address(node.address));
            node.tessera_footprint
                .is_some_and(|footprint| footprint.occupies(anchor, slot))
                || node.tessera_footprint.is_none() && anchor == slot
        }) {
            return Some(PickHit::Tile {
                node: node.node.clone(),
            });
        }

        let surface = self.active_surface?;
        match self.layout {
            SurfaceLayoutKind::Board => Some(PickHit::EmptySlot { surface, slot }),
            SurfaceLayoutKind::Stack if slot.x >= 0 && slot.y == 0 => {
                let index = self
                    .stack_display
                    .authored_index(StackIndex(slot.x as usize));
                // The append marker is a hint, not the only usable cell.
                // Empty cells (including wrapped rows and authored gaps) are
                // legitimate drop targets, just like the visible grid suggests.
                Some(PickHit::StackInsert { surface, index })
            }
            SurfaceLayoutKind::Stack => None,
        }
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
        runtime.needs_scene(),
        attention.is_changed(),
        selection.is_changed(),
        visible.active_surface,
        attention.active_board(),
    ) && !view_settings.is_changed()
    {
        return;
    }

    *visible = project_visible_board(&project, &attention, &selection, *view_settings);
}

#[cfg(test)]
#[path = "compact_stack_tests.rs"]
mod compact_stack_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::board_view_settings::{AtomDisplayMode, BoardViewSettings},
        application::command::{EditorCommand, execute_command},
        application::editor::{
            AtomCompoundSemantic, AtomCompoundView, EditorAttention, SelectionState,
            transaction::PlacementTarget,
        },
        application::pipeline::{
            runtime::TimelineProvenanceStore,
            scene_sync::{TileSurfaceContent, projection::project_visible_board},
        },
        application::session::MusaicProject,
        domain::board::{BoardSlot, BoardSurfaceId},
        domain::document::{AtomValue, ContainerKind, NoteName, TileSpawnKind},
    };

    #[test]
    fn visible_board_compounds_atom_row_into_a2() {
        let mut project = MusaicProject::new_empty();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = project.document.root_surface;
        let mut attention = EditorAttention::new(root_surface);
        let mut selection = SelectionState::default();
        execute_command(
            &mut project.document,
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
        )
        .unwrap();
        let container = selection.nodes.iter().next().unwrap().clone();
        let local_surface = project
            .document
            .graph
            .container_surface(&container)
            .unwrap();
        execute_command(
            &mut project.document,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::EnterContainer { container },
        )
        .unwrap();
        for (x, atom) in [
            (0, AtomValue::NoteName(NoteName::A)),
            (1, AtomValue::Octave(2)),
        ] {
            execute_command(
                &mut project.document,
                &mut attention,
                &mut selection,
                &provenance,
                &EditorCommand::PlaceTile {
                    target: PlacementTarget::BoardSlot {
                        surface: local_surface,
                        slot: BoardSlot::new(x, 0),
                    },
                    tile: TileSpawnKind::Atom { atom },
                },
            )
            .unwrap();
        }

        let visible = project_visible_board(
            &project,
            &attention,
            &selection,
            BoardViewSettings {
                container_interior: AtomDisplayMode::CompoundTile,
                ..Default::default()
            },
        );
        assert_eq!(visible.active_surface, Some(BoardSurfaceId(1)));
        assert_eq!(visible.atom_compounds.len(), 1);
        assert_eq!(visible.atom_compounds[0].compound.display, "A2");
    }

    #[test]
    fn visible_board_projects_port_chrome_for_root_board_tiles() {
        let mut project = MusaicProject::new_empty();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = project.document.root_surface;
        let mut attention = EditorAttention::new(root_surface);
        let mut selection = SelectionState::default();
        execute_command(
            &mut project.document,
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
        )
        .unwrap();

        let visible = project_visible_board(
            &project,
            &attention,
            &selection,
            BoardViewSettings::default(),
        );
        let node = visible
            .nodes
            .iter()
            .find(|n| n.kind == VisibleNodeKind::Container)
            .expect("container tile on visible board");
        assert!(node.atom.is_none());
        assert!(
            node.ports.is_some(),
            "root-board tiles must project port compass view data"
        );
    }

    #[test]
    fn address_of_returns_projected_node_address() {
        let node = NodeId::new("tile");
        let visible = VisibleBoardState {
            active_surface: Some(BoardSurfaceId(1)),
            layout: SurfaceLayoutKind::Board,
            nodes: vec![VisibleBoardNode {
                node: node.clone(),
                address: PlacementAddress::BoardSlot(BoardSlot::new(3, 4)),
                tessera_footprint: None,
                kind: VisibleNodeKind::Output,
                selected: false,
                focused: false,
                icon: None,
                atom: None,
                ports: None,
                surface_content: TileSurfaceContent::Empty,
            }],
            ..Default::default()
        };
        assert_eq!(
            visible.address_of(&node),
            Some(PlacementAddress::BoardSlot(BoardSlot::new(3, 4)))
        );
        assert_eq!(visible.address_of(&NodeId::new("missing")), None);
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
                atom: Some(AtomValue::NoteName(NoteName::A)),
                ports: None,
                surface_content: TileSurfaceContent::Empty,
            }],
            atom_compounds: vec![VisibleAtomCompound {
                slot: BoardSlot::new(0, 0),
                compound: AtomCompoundView {
                    members: vec![NodeId::new("atom_a"), NodeId::new("atom_oct")],
                    display: "A2".into(),
                    semantic: AtomCompoundSemantic::Pitch,
                },
                primary_atom: Some(AtomValue::NoteName(NoteName::A)),
            }],
            stack_inserts: Vec::new(),
            stack_locked_slots: Vec::new(),
            stack_display: Default::default(),
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
                atom: None,
                ports: None,
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
