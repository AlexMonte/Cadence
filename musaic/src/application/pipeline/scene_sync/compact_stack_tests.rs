use super::*;
use crate::application::{
    board_view_settings::BoardViewSettings,
    pipeline::{
        runtime::RuntimeState,
        scene_sync::surface_content::{owned_compound_for_node, surface_content_for_node},
    },
};
use crate::domain::document::{AtomValue, DocumentNodeKind, DocumentQueries};
use std::collections::BTreeMap;

fn fixture() -> (App, NodeId, BoardSurfaceId, Vec<NodeId>) {
    let project = MusaicProject::demo();
    let container = project
        .document
        .graph
        .nodes_on_surface(project.document.root_surface)
        .into_iter()
        .find(|(_, node)| matches!(node.kind, DocumentNodeKind::Container(_)))
        .unwrap()
        .1
        .id
        .clone();
    let surface = project
        .document
        .graph
        .container_surface(&container)
        .unwrap();
    let mut authored = project.document.graph.nodes_on_surface(surface);
    authored.sort_by_key(|(location, _)| match location.address {
        PlacementAddress::StackIndex(index) => index.0,
        _ => unreachable!("container children have stack addresses"),
    });
    let nodes: Vec<_> = authored
        .into_iter()
        .map(|(_, node)| node.id.clone())
        .collect();
    let mut app = App::new();
    app.insert_resource(project)
        .insert_resource(EditorAttention::new(surface))
        .init_resource::<SelectionState>()
        .init_resource::<BoardViewSettings>()
        .init_resource::<RuntimeState>()
        .init_resource::<VisibleBoardState>()
        .add_systems(Update, rebuild_visible_board_state);
    app.update();
    (app, container, surface, nodes)
}

#[test]
fn compact_faces_pick_whole_expressions_without_changing_authored_addresses() {
    let (mut app, container, surface, nodes) = fixture();
    let graph = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .clone();
    let visible = app.world().resource::<VisibleBoardState>();
    assert_eq!(
        visible
            .atom_compounds
            .iter()
            .map(|c| (
                c.slot.x,
                c.compound.display.as_str(),
                c.compound.members.len()
            ))
            .collect::<Vec<_>>(),
        [(0, "C♯4", 7), (1, "E4", 2)]
    );
    for (index, node) in nodes.iter().enumerate() {
        assert_eq!(
            visible.address_of(node),
            Some(PlacementAddress::StackIndex(StackIndex(index)))
        );
        let expected = if index < 7 {
            0
        } else if index < 9 {
            1
        } else {
            2
        };
        assert_eq!(
            visible.display_address_of(node),
            Some(PlacementAddress::StackIndex(StackIndex(expected)))
        );
    }
    for (cell, range) in [(0, 0..7), (1, 7..9)] {
        assert_eq!(
            visible.pick_at(BoardSlot::new(cell, 0)),
            Some(PickHit::AtomCompound {
                primary_node: nodes[range.start].clone(),
                members: nodes[range].to_vec(),
            })
        );
    }
    assert_eq!(
        visible.pick_at(BoardSlot::new(2, 0)),
        Some(PickHit::Tile {
            node: nodes[9].clone()
        })
    );
    assert_eq!(
        visible.pick_at(BoardSlot::new(3, 0)),
        Some(PickHit::Tile {
            node: nodes[9].clone()
        })
    );
    assert_eq!(
        visible.pick_at(BoardSlot::new(4, 0)),
        Some(PickHit::StackInsert {
            surface,
            index: StackIndex(10)
        })
    );
    assert_eq!(
        visible.pick_at(BoardSlot::new(5, 0)),
        Some(PickHit::StackInsert {
            surface,
            index: StackIndex(11)
        })
    );

    // Selecting a hidden operand highlights the visible note and exposes its owner.
    app.world_mut()
        .resource_mut::<SelectionState>()
        .nodes
        .insert(nodes[4].clone());
    app.update();
    let visible = app.world().resource::<VisibleBoardState>();
    assert!(
        visible
            .nodes
            .iter()
            .find(|node| node.node == nodes[0])
            .unwrap()
            .selected
    );
    let project = app.world().resource::<MusaicProject>();
    let queries = DocumentQueries::new(&project.document);
    let owned = owned_compound_for_node(&queries, &nodes[4]).unwrap();
    assert_eq!(owned.groups[owned.selected_group].members, nodes[3..5]);
    assert_eq!(
        owned.groups[owned.selected_group]
            .owned_value
            .as_ref()
            .unwrap()
            .atom,
        AtomValue::Number(2)
    );
    assert_eq!(project.document.graph, graph);

    // The closed container uses exactly the same faces as the opened surface.
    let content =
        surface_content_for_node(&queries, &container, &BTreeMap::new(), &Default::default());
    let super::super::types::TileSurfaceContent::Container {
        children,
        child_count,
        ..
    } = content
    else {
        panic!("container preview")
    };
    assert_eq!(child_count, 10);
    assert_eq!(children.len(), 3);
    assert_eq!(
        children
            .iter()
            .map(|child| child.slot.x)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        children[0].content,
        super::super::types::TileSurfaceContent::Compound {
            display: "C♯4".into(),
            layers: 7,
            parts: ["C", "#", "4", "@", "2", "×", "3"]
                .map(String::from)
                .to_vec(),
        }
    );
    assert_eq!(
        children[1].content,
        super::super::types::TileSurfaceContent::Compound {
            display: "E4".into(),
            layers: 2,
            parts: vec!["E".into(), "4".into()],
        }
    );
}

#[test]
fn reordered_modifier_keeps_its_operand_and_compact_pitch_summary() {
    let (mut app, _, _, nodes) = fixture();
    let before = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .clone();
    let result = crate::application::editor::transaction::move_modifier_group(
        &mut app.world_mut().resource_mut::<MusaicProject>().document,
        &nodes[3],
        -1,
    );
    assert!(result.is_accepted(), "{:?}", result.diagnostics);
    app.update();
    let visible = app.world().resource::<VisibleBoardState>();
    assert_eq!(visible.atom_compounds[0].compound.display, "C♯4");
    assert_eq!(visible.atom_compounds[0].compound.members.len(), 7);
    let project = app.world().resource::<MusaicProject>();
    let group =
        owned_compound_for_node(&DocumentQueries::new(&project.document), &nodes[4]).unwrap();
    assert_eq!(group.groups[group.selected_group].members, nodes[3..5]);
    for node in &nodes {
        assert_eq!(
            project.document.graph.node(node).unwrap().kind,
            before.node(node).unwrap().kind
        );
    }
}

#[test]
fn compaction_keeps_authored_gaps_and_does_not_move_root_board_addresses() {
    let map = super::super::types::StackDisplayMap::new(
        BTreeMap::from([
            (StackIndex(1), StackIndex(0)),
            (StackIndex(2), StackIndex(0)),
            (StackIndex(5), StackIndex(4)),
        ]),
        BTreeMap::new(),
    );
    // Canonical slot 3 remains an empty display cell between the two expressions.
    for (authored, display) in [(0, 0), (1, 0), (2, 0), (3, 1), (4, 2), (5, 2), (6, 3)] {
        assert_eq!(map.display_index(StackIndex(authored)), StackIndex(display));
    }
    for (display, authored) in [(0, 0), (1, 3), (2, 4), (3, 6)] {
        assert_eq!(
            map.authored_index(StackIndex(display)),
            StackIndex(authored)
        );
    }
    let root = PlacementAddress::BoardSlot(BoardSlot::new(13, 15));
    assert_eq!(map.display_address(root), root);
}

#[test]
fn nested_container_wraps_as_two_cells_and_the_next_slot_keeps_its_musical_address() {
    let map = super::super::types::StackDisplayMap::new(
        BTreeMap::new(),
        BTreeMap::from([(StackIndex(11), 2), (StackIndex(12), 2)]),
    );
    assert_eq!(map.display_index(StackIndex(10)), StackIndex(10));
    assert_eq!(map.display_index(StackIndex(11)), StackIndex(12));
    for cell in [11, 12, 13] {
        assert_eq!(map.authored_index(StackIndex(cell)), StackIndex(11));
    }
    assert_eq!(map.display_index(StackIndex(12)), StackIndex(14));
    for cell in [14, 15] {
        assert_eq!(map.authored_index(StackIndex(cell)), StackIndex(12));
    }
    assert_eq!(map.display_index(StackIndex(13)), StackIndex(16));
    assert_eq!(map.authored_index(StackIndex(16)), StackIndex(13));
}
