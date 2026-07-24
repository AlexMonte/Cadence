use std::collections::BTreeMap;

use tessera::prelude::{
    AtomOperatorToken, AtomTile, Board, BoardError, Container, ContainerAxis, ContainerId,
    ContainerKind, ContainerSurfaceTile, NodeId, NoteAtom, SequenceStack,
};

use crate::domain::board::{BoardSlot, BoardSurfaceId};

use super::MusaicDocument;
use super::footprint::{RootBoardTileKind, root_board_tile_footprint};
use super::graph::{
    AtomNode, AtomValue, ContainerKind as DocumentContainerKind, DocumentGraph, DocumentNode,
    DocumentNodeKind, NoteName, OperatorValue, PlacementAddress, StackIndex, TileSpawnKind,
};
use crate::domain::transform::transform_kind_from_prototype;

pub struct DocumentBoardExport {
    pub board: Board,
    pub nested_containers: BTreeMap<ContainerId, Container>,
}

pub fn export_document_to_board(
    document: &MusaicDocument,
) -> Result<DocumentBoardExport, BoardError> {
    let mut board = Board::new();
    let mut nested_containers = BTreeMap::new();
    let root = document.root_surface;

    let mut root_placements = document
        .graph
        .nodes_on_surface(root)
        .into_iter()
        .filter_map(|(location, node)| match location.address {
            PlacementAddress::BoardSlot(slot) => Some((slot, node)),
            _ => None,
        })
        .collect::<Vec<_>>();
    root_placements.sort_by_key(|(slot, _)| (slot.y, slot.x));

    for (slot, node) in root_placements {
        place_root_node(
            &mut board,
            &document.graph,
            slot,
            node,
            &mut nested_containers,
        )?;
    }

    Ok(DocumentBoardExport {
        board,
        nested_containers,
    })
}

pub fn finish_board_export(
    export: DocumentBoardExport,
) -> tessera::prelude::AuthoredTesseraProgram {
    let mut program = export.board.finish();
    program.containers.extend(export.nested_containers);
    program
}

fn place_root_node(
    board: &mut Board,
    graph: &DocumentGraph,
    slot: BoardSlot,
    node: &DocumentNode,
    nested: &mut BTreeMap<ContainerId, Container>,
) -> Result<(), BoardError> {
    let name = node.id.0.clone();
    let footprint = root_board_tile_footprint(match &node.kind {
        DocumentNodeKind::Container(_) => RootBoardTileKind::Container,
        DocumentNodeKind::Output(_) => RootBoardTileKind::Output,
        _ => RootBoardTileKind::Other,
    });
    match &node.kind {
        DocumentNodeKind::Container(container) => {
            let stack = export_container_stack(graph, node, container.local_surface, nested);
            let slot_builder = board.at(slot.x, slot.y).named(name).footprint(footprint);
            match map_container_kind_for_document(container.kind) {
                ContainerKind::Sequence => {
                    let _ = slot_builder.sequence(stack)?;
                }
                ContainerKind::Alternate => {
                    let _ = slot_builder.alternate(stack)?;
                }
                ContainerKind::Layer => {
                    let _ = slot_builder.layer_container(stack)?;
                }
            }
        }
        DocumentNodeKind::Output(_) => {
            board
                .at(slot.x, slot.y)
                .named(name)
                .footprint(footprint)
                .output()?;
        }
        DocumentNodeKind::TrickInstance(trick) => {
            let Some(kind) = transform_kind_from_prototype(trick.prototype) else {
                return Err(BoardError::UnknownTile);
            };
            board
                .at(slot.x, slot.y)
                .named(name)
                .footprint(footprint)
                .transform(kind)?;
        }
        DocumentNodeKind::Tile(_)
        | DocumentNodeKind::Atom(_)
        | DocumentNodeKind::Arrangement(_) => {}
    }
    Ok(())
}

/// Builds an empty tessera stack for a newly placed container.
pub fn empty_container_stack() -> Vec<ContainerSurfaceTile> {
    SequenceStack::new().build()
}

pub fn export_container_stack(
    graph: &DocumentGraph,
    _node: &DocumentNode,
    surface: BoardSurfaceId,
    nested: &mut BTreeMap<ContainerId, Container>,
) -> Vec<ContainerSurfaceTile> {
    let stack_nodes = stack_nodes_on_surface(graph, surface);
    let refs = stack_nodes
        .iter()
        .map(|(index, node)| (*index, node))
        .collect::<Vec<_>>();
    export_stack_from_nodes(graph, _node, &refs, nested)
}

pub fn stack_nodes_on_surface(
    graph: &DocumentGraph,
    surface: BoardSurfaceId,
) -> Vec<(StackIndex, DocumentNode)> {
    let mut stack_nodes = graph
        .nodes_on_surface(surface)
        .into_iter()
        .map(|(location, node)| match location.address {
            PlacementAddress::StackIndex(index) => (index, node.clone()),
            PlacementAddress::BoardSlot(slot) => (StackIndex(slot.x as usize), node.clone()),
        })
        .collect::<Vec<_>>();
    stack_nodes.sort_by_key(|(index, _)| index.0);
    stack_nodes
}

pub fn export_container_stack_with_insert(
    graph: &mut DocumentGraph,
    container_node: &DocumentNode,
    surface: BoardSurfaceId,
    at: StackIndex,
    tile: &TileSpawnKind,
    nested: &mut BTreeMap<ContainerId, Container>,
) -> Result<
    (
        Vec<ContainerSurfaceTile>,
        NodeId,
        Vec<(StackIndex, DocumentNode)>,
    ),
    BoardError,
> {
    if !graph.is_stack_index_empty(surface, at) {
        return Err(BoardError::SlotOccupied);
    }
    let mut stack_nodes = stack_nodes_on_surface(graph, surface);
    let new_id = NodeId::new(format!("{}_{}", container_node.id.0, at.0));
    graph.reserve_node_id(&new_id);
    let new_node = document_node_from_spawn(new_id.clone(), tile).ok_or(BoardError::UnknownTile)?;
    stack_nodes.push((at, new_node));
    stack_nodes.sort_by_key(|(index, _)| index.0);
    let stack = export_stack_from_nodes(
        graph,
        container_node,
        &stack_nodes
            .iter()
            .map(|(index, node)| (*index, node))
            .collect::<Vec<_>>(),
        nested,
    );
    Ok((stack, new_id, stack_nodes))
}

pub fn export_container_stack_excluding(
    graph: &DocumentGraph,
    container_node: &DocumentNode,
    surface: BoardSurfaceId,
    exclude: &NodeId,
    nested: &mut BTreeMap<ContainerId, Container>,
) -> Vec<ContainerSurfaceTile> {
    let stack_nodes = stack_nodes_on_surface(graph, surface)
        .into_iter()
        .filter(|(_, node)| node.id != *exclude)
        .collect::<Vec<_>>();
    export_stack_from_nodes(
        graph,
        container_node,
        &stack_nodes
            .iter()
            .map(|(index, node)| (*index, node))
            .collect::<Vec<_>>(),
        nested,
    )
}

fn export_stack_from_nodes(
    graph: &DocumentGraph,
    _container_node: &DocumentNode,
    stack_nodes: &[(StackIndex, &DocumentNode)],
    nested: &mut BTreeMap<ContainerId, Container>,
) -> Vec<ContainerSurfaceTile> {
    let mut stack = SequenceStack::new();
    let mut next_stack_index = 0usize;
    let mut index = 0;
    while index < stack_nodes.len() {
        let (stack_index, node) = &stack_nodes[index];
        while next_stack_index < stack_index.0 {
            stack = stack.push(ContainerSurfaceTile::Atom(AtomTile::Rest));
            next_stack_index += 1;
        }
        match &node.kind {
            DocumentNodeKind::Atom(atom_node) => {
                if let Some((consumed, tiles)) =
                    atom_tiles_from_stack(&stack_nodes[index..], &atom_node.atom)
                {
                    for tile in tiles {
                        stack = stack.push(tile);
                    }
                    index += consumed;
                    next_stack_index = stack_index.0 + consumed;
                    continue;
                }
            }
            DocumentNodeKind::Container(container) => {
                let container_id = ContainerId::new(node.id.0.clone());
                let inner_stack =
                    export_container_stack(graph, node, container.local_surface, nested);
                nested.insert(
                    container_id.clone(),
                    Container {
                        kind: map_container_kind_for_document(container.kind),
                        axis: ContainerAxis::Time,
                        stack: inner_stack,
                    },
                );
                stack = stack.push(ContainerSurfaceTile::NestedContainer(container_id));
                next_stack_index = stack_index.0 + 1;
            }
            _ => {}
        }
        index += 1;
    }

    stack.build()
}

fn document_node_from_spawn(id: NodeId, tile: &TileSpawnKind) -> Option<DocumentNode> {
    let kind = match tile {
        TileSpawnKind::Atom { atom } => DocumentNodeKind::Atom(AtomNode { atom: atom.clone() }),
        TileSpawnKind::Container { kind: _ } => return None,
        _ => return None,
    };
    Some(DocumentNode { id, kind })
}

fn atom_tiles_from_stack(
    nodes: &[(StackIndex, &DocumentNode)],
    atom: &AtomValue,
) -> Option<(usize, Vec<ContainerSurfaceTile>)> {
    match atom {
        AtomValue::NoteName(note) => {
            let (consumed, note_atom) = compound_note_from_stack(nodes, *note);
            Some((
                consumed,
                vec![ContainerSurfaceTile::Atom(AtomTile::Note(note_atom))],
            ))
        }
        AtomValue::Rest => Some((1, vec![ContainerSurfaceTile::Atom(AtomTile::Rest)])),
        AtomValue::Number(value) => Some((
            1,
            vec![ContainerSurfaceTile::Atom(AtomTile::Scalar(
                tessera::prelude::ScalarAtom::integer(*value as i64),
            ))],
        )),
        AtomValue::Operator(operator) => operator_tiles_from_stack(nodes, *operator),
        AtomValue::Octave(_) | AtomValue::Accidental(_) => None,
    }
}

fn compound_note_from_stack(
    nodes: &[(StackIndex, &DocumentNode)],
    note: NoteName,
) -> (usize, NoteAtom) {
    let mut note_atom = NoteAtom::new(note_letter(note));
    let mut consumed = 1;

    if let Some((
        _,
        DocumentNode {
            kind: DocumentNodeKind::Atom(atom),
            ..
        },
    )) = nodes.get(consumed)
    {
        if let AtomValue::Accidental(_accidental) = atom.atom {
            consumed += 1;
        }
    }

    if let Some((
        _,
        DocumentNode {
            kind: DocumentNodeKind::Atom(atom),
            ..
        },
    )) = nodes.get(consumed)
    {
        if let AtomValue::Octave(octave) = atom.atom {
            note_atom = note_atom.with_octave(octave as i64);
            consumed += 1;
        }
    }

    (consumed, note_atom)
}

fn operator_tiles_from_stack(
    nodes: &[(StackIndex, &DocumentNode)],
    operator: OperatorValue,
) -> Option<(usize, Vec<ContainerSurfaceTile>)> {
    let token = match operator {
        OperatorValue::Power => AtomOperatorToken::Replicate,
        OperatorValue::At => AtomOperatorToken::Elongate,
        OperatorValue::Multiply => AtomOperatorToken::Fast,
        OperatorValue::Divide => AtomOperatorToken::Slow,
    };

    let mut tiles = vec![ContainerSurfaceTile::Atom(AtomTile::Operator(token))];
    let mut consumed = 1;

    if operator_takes_scalar_operand(operator) {
        if let Some((
            _,
            DocumentNode {
                kind: DocumentNodeKind::Atom(atom),
                ..
            },
        )) = nodes.get(consumed)
        {
            if let AtomValue::Number(value) = atom.atom {
                tiles.push(ContainerSurfaceTile::Atom(AtomTile::Scalar(
                    tessera::prelude::ScalarAtom::integer(value as i64),
                )));
                consumed += 1;
            }
        }
    }

    Some((consumed, tiles))
}

fn operator_takes_scalar_operand(operator: OperatorValue) -> bool {
    matches!(
        operator,
        OperatorValue::Power | OperatorValue::At | OperatorValue::Multiply | OperatorValue::Divide
    )
}

pub fn map_container_kind_for_document(kind: DocumentContainerKind) -> ContainerKind {
    match kind {
        DocumentContainerKind::Sequence | DocumentContainerKind::Subdivision => {
            ContainerKind::Sequence
        }
        DocumentContainerKind::Alternating => ContainerKind::Alternate,
        DocumentContainerKind::Parallel => ContainerKind::Layer,
    }
}

pub fn map_tessera_container_kind_to_document(kind: ContainerKind) -> DocumentContainerKind {
    match kind {
        ContainerKind::Sequence => DocumentContainerKind::Sequence,
        ContainerKind::Alternate => DocumentContainerKind::Alternating,
        ContainerKind::Layer => DocumentContainerKind::Parallel,
    }
}

fn note_letter(note: NoteName) -> &'static str {
    match note {
        NoteName::A => "a",
        NoteName::B => "b",
        NoteName::C => "c",
        NoteName::D => "d",
        NoteName::E => "e",
        NoteName::F => "f",
        NoteName::G => "g",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{ContainerKind, MusaicDocument, TileSpawnKind};
    use tessera::prelude::{ContainerId, TesseraCompiler};

    #[test]
    fn export_sequence_and_output_compiles() {
        let mut document = MusaicDocument::new_empty();
        let root = document.root_surface;

        let sequence = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            )
            .unwrap();
        let container_surface = document.graph.container_surface(&sequence).unwrap();

        for (index, note) in [(0, NoteName::A), (1, NoteName::B), (2, NoteName::C)] {
            document
                .graph
                .insert_tile(
                    &mut document.surfaces,
                    container_surface,
                    PlacementAddress::StackIndex(StackIndex(index)),
                    TileSpawnKind::Atom {
                        atom: AtomValue::NoteName(note),
                    },
                )
                .unwrap();
        }

        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(2, 0)),
                TileSpawnKind::Output {
                    name: "main".into(),
                },
            )
            .unwrap();

        let export = export_document_to_board(&document).expect("export should succeed");
        let program = finish_board_export(export);
        let (_, _, report) = TesseraCompiler::new()
            .compile_authored_pipeline(&program)
            .expect("exported board should compile");
        let ir = report.ir;
        assert_eq!(ir.outputs.len(), 1);
    }

    #[test]
    fn export_fast_and_slow_operators_with_scalar() {
        let mut document = MusaicDocument::new_empty();
        let root = document.root_surface;

        let sequence = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            )
            .unwrap();
        let container_surface = document.graph.container_surface(&sequence).unwrap();

        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                container_surface,
                PlacementAddress::StackIndex(StackIndex(0)),
                TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::C),
                },
            )
            .unwrap();
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                container_surface,
                PlacementAddress::StackIndex(StackIndex(1)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Operator(OperatorValue::Multiply),
                },
            )
            .unwrap();
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                container_surface,
                PlacementAddress::StackIndex(StackIndex(2)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Number(2),
                },
            )
            .unwrap();
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                container_surface,
                PlacementAddress::StackIndex(StackIndex(3)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Operator(OperatorValue::Divide),
                },
            )
            .unwrap();
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                container_surface,
                PlacementAddress::StackIndex(StackIndex(4)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Number(3),
                },
            )
            .unwrap();

        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(2, 0)),
                TileSpawnKind::Output {
                    name: "main".into(),
                },
            )
            .unwrap();

        let export = export_document_to_board(&document).expect("export should succeed");
        let program = finish_board_export(export);
        let sequence_stack = &program
            .containers
            .get(&ContainerId::new(sequence.0.clone()))
            .expect("sequence container")
            .stack;
        assert_eq!(
            sequence_stack[1..5],
            [
                ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Fast)),
                ContainerSurfaceTile::Atom(AtomTile::Scalar(
                    tessera::prelude::ScalarAtom::integer(2)
                )),
                ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Slow)),
                ContainerSurfaceTile::Atom(AtomTile::Scalar(
                    tessera::prelude::ScalarAtom::integer(3)
                )),
            ]
        );
        TesseraCompiler::new()
            .compile_authored_pipeline(&program)
            .expect("fast/slow operators should compile");
    }
}
