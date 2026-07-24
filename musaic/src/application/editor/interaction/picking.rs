use tessera::prelude::NodeId;

use crate::{
    domain::board::{BoardSlot, BoardSurfaceId},
    domain::document::{PortId, StackIndex, queries::DocumentQueries},
};

use crate::application::command::EditorCommand;
use crate::application::editor::selection::SelectionMode;
use crate::application::editor::workspace::FocusTarget;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PickModifiers {
    pub additive: bool,
    pub toggle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardPick {
    pub hit: PickHit,
    pub modifiers: PickModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickHit {
    EmptySlot {
        surface: BoardSurfaceId,
        slot: BoardSlot,
    },
    StackInsert {
        surface: BoardSurfaceId,
        index: StackIndex,
    },
    Tile {
        node: NodeId,
    },
    AtomCompound {
        primary_node: NodeId,
        members: Vec<NodeId>,
    },
    Port {
        node: NodeId,
        port: PortId,
    },
}

/// Validates a pick against the document and produces the focus/select command.
pub fn command_from_pick_checked(
    queries: &DocumentQueries<'_>,
    pick: BoardPick,
) -> Option<EditorCommand> {
    match pick.hit {
        PickHit::EmptySlot { surface, slot } => {
            if !queries.has_surface(surface) {
                return None;
            }
            Some(EditorCommand::Focus {
                target: FocusTarget::EmptySlot { surface, slot },
            })
        }
        PickHit::StackInsert { surface, index } => {
            if !queries.has_surface(surface) {
                return None;
            }
            Some(EditorCommand::Focus {
                target: FocusTarget::StackInsert { surface, index },
            })
        }
        PickHit::Tile { node } => {
            if !queries.contains_node(&node) {
                return None;
            }
            Some(EditorCommand::SelectNode {
                node,
                mode: selection_mode_from_modifiers(pick.modifiers),
            })
        }
        PickHit::AtomCompound { primary_node, .. } => {
            if !queries.contains_node(&primary_node) {
                return None;
            }
            Some(EditorCommand::Focus {
                target: FocusTarget::Atom { node: primary_node },
            })
        }
        PickHit::Port { node, port } => {
            if !queries.contains_node(&node) {
                return None;
            }
            Some(EditorCommand::Focus {
                target: FocusTarget::Port { node, port },
            })
        }
    }
}

fn selection_mode_from_modifiers(modifiers: PickModifiers) -> SelectionMode {
    if modifiers.toggle {
        SelectionMode::Toggle
    } else if modifiers.additive {
        SelectionMode::Add
    } else {
        SelectionMode::Replace
    }
}

#[cfg(test)]
mod tests {
    use tessera::prelude::NodeId;

    use super::*;
    use crate::domain::document::{
        DocumentQueries, MusaicDocument, PlacementAddress, TileSpawnKind,
    };

    fn queries_with_root_tile() -> (MusaicDocument, NodeId) {
        let mut document = MusaicDocument::new_empty();
        let node = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                document.root_surface,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Output { name: "out".into() },
            )
            .unwrap();
        (document, node)
    }

    #[test]
    fn checked_empty_slot_pick_focuses_empty_slot() {
        let document = MusaicDocument::new_empty();
        let queries = DocumentQueries::new(&document);
        let pick = BoardPick {
            hit: PickHit::EmptySlot {
                surface: document.root_surface,
                slot: BoardSlot::new(1, 2),
            },
            modifiers: PickModifiers::default(),
        };
        let command = command_from_pick_checked(&queries, pick).unwrap();
        assert_eq!(
            command,
            EditorCommand::Focus {
                target: FocusTarget::EmptySlot {
                    surface: document.root_surface,
                    slot: BoardSlot::new(1, 2),
                },
            }
        );
    }

    #[test]
    fn checked_tile_pick_replaces_selection_by_default() {
        let (document, node) = queries_with_root_tile();
        let queries = DocumentQueries::new(&document);
        let pick = BoardPick {
            hit: PickHit::Tile { node: node.clone() },
            modifiers: PickModifiers::default(),
        };
        let command = command_from_pick_checked(&queries, pick).unwrap();
        assert_eq!(
            command,
            EditorCommand::SelectNode {
                node,
                mode: SelectionMode::Replace,
            }
        );
    }

    #[test]
    fn checked_atom_compound_pick_focuses_primary_atom() {
        let (mut document, _) = queries_with_root_tile();
        let atom = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                document.root_surface,
                // East of the 2×2 output at (0,0).
                PlacementAddress::BoardSlot(BoardSlot::new(2, 0)),
                TileSpawnKind::Output {
                    name: "atomish".into(),
                },
            )
            .unwrap();
        let queries = DocumentQueries::new(&document);
        let pick = BoardPick {
            hit: PickHit::AtomCompound {
                primary_node: atom.clone(),
                members: vec![atom.clone(), NodeId::new("other")],
            },
            modifiers: PickModifiers::default(),
        };
        let command = command_from_pick_checked(&queries, pick).unwrap();
        assert_eq!(
            command,
            EditorCommand::Focus {
                target: FocusTarget::Atom { node: atom },
            }
        );
    }

    #[test]
    fn stale_tile_pick_returns_none() {
        let document = MusaicDocument::new_empty();
        let queries = DocumentQueries::new(&document);
        let pick = BoardPick {
            hit: PickHit::Tile {
                node: NodeId::new("missing"),
            },
            modifiers: PickModifiers::default(),
        };
        assert_eq!(command_from_pick_checked(&queries, pick), None);
    }
}
