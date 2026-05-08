use std::collections::BTreeMap;

use serde_json::Value;
use tessera::{
    graph::{Edge, Graph, Node},
    types::{EdgeId, GridPos, TileSide, adjacent_in_direction},
};

use crate::{
    adapter::tessera::{
        graph_defaults::normalize_graph_piece_sides, host_adapter::runtime_engine,
        lowering::lower_target_graph, piece_registry::default_cadence_registry,
    },
    domain::common::GridPos as CadenceGridPos,
    domain::project::{CadenceGraphTarget, CadenceProjectDocument},
};

fn simple_project() -> CadenceProjectDocument {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "cadence.sound".into(),
            inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
            pattern_source: None,
            input_sides: BTreeMap::new(),
            output_side: Some(TileSide::RIGHT),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "cadence.output".into(),
            inline_params: BTreeMap::new(),
            input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
            output_side: None,
            label: None,
            node_state: None,
            pattern_source: None,
        },
    );

    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".into(),
    };

    CadenceProjectDocument::new(
        "demo".to_string(),
        Graph {
            nodes,
            edges: BTreeMap::from([(edge.id.clone(), edge)]),
            name: "demo".to_string(),
            cols: 12,
            rows: 8,
        },
    )
}

fn combinator_project(piece_id: &str) -> CadenceProjectDocument {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "cadence.sound".into(),
            inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
            pattern_source: None,
            input_sides: BTreeMap::new(),
            output_side: Some(TileSide::RIGHT),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: piece_id.into(),
            inline_params: BTreeMap::new(),
            pattern_source: None,
            input_sides: BTreeMap::from([("in_w".into(), TileSide::LEFT)]),
            output_side: Some(TileSide::RIGHT),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "cadence.output".into(),
            inline_params: BTreeMap::new(),
            input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
            output_side: None,
            label: None,
            node_state: None,
            pattern_source: None,
        },
    );

    let edge_1 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "in_w".into(),
    };
    let edge_2 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".into(),
    };

    CadenceProjectDocument::new(
        piece_id.to_string(),
        Graph {
            nodes,
            edges: BTreeMap::from([(edge_1.id.clone(), edge_1), (edge_2.id.clone(), edge_2)]),
            name: piece_id.to_string(),
            cols: 12,
            rows: 8,
        },
    )
}

#[test]
fn tile_side_faces_matrix_matches_expected_pairs() {
    let sides = [
        TileSide::TOP,
        TileSide::BOTTOM,
        TileSide::RIGHT,
        TileSide::LEFT,
    ];

    for left in sides {
        for right in sides {
            let expected = matches!(
                (left, right),
                (TileSide::RIGHT, TileSide::LEFT)
                    | (TileSide::LEFT, TileSide::RIGHT)
                    | (TileSide::TOP, TileSide::BOTTOM)
                    | (TileSide::BOTTOM, TileSide::TOP)
            );
            assert_eq!(left.faces(right), expected);
        }
    }
}

#[test]
fn adjacent_in_direction_covers_all_directions_and_negative_coords() {
    let origin = GridPos { col: -3, row: 5 };
    assert_eq!(
        adjacent_in_direction(&origin, Some(TileSide::TOP)),
        GridPos { col: -3, row: 4 }
    );
    assert_eq!(
        adjacent_in_direction(&origin, Some(TileSide::BOTTOM)),
        GridPos { col: -3, row: 6 }
    );
    assert_eq!(
        adjacent_in_direction(&origin, Some(TileSide::LEFT)),
        GridPos { col: -4, row: 5 }
    );
    assert_eq!(
        adjacent_in_direction(&origin, Some(TileSide::RIGHT)),
        GridPos { col: -2, row: 5 }
    );
}

#[test]
fn registry_contains_expected_runtime_pieces() {
    let registry = default_cadence_registry();
    assert!(registry.get("cadence.sound").is_some());
    assert!(registry.get("cadence.output").is_some());
    assert!(registry.get("cadence.fast").is_some());
}

#[test]
fn normalized_simple_graph_is_analyzable() {
    let project = simple_project();
    let mut graph = project.runtime_graph().clone();
    let registry = default_cadence_registry();

    assert!(normalize_graph_piece_sides(&mut graph, &registry));

    let analyzed = runtime_engine(&project).analyze(&graph);
    assert!(
        analyzed.diagnostics.is_empty(),
        "{:?}",
        analyzed.diagnostics
    );
    assert_eq!(analyzed.outputs, vec![GridPos { col: 1, row: 0 }]);
}

#[test]
fn lowering_simple_project_produces_one_output_pattern_and_preview_event() {
    let project = simple_project();
    let analyzed = runtime_engine(&project).analyze(project.runtime_graph());
    let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

    assert_eq!(lowered.outputs, vec![CadenceGridPos { col: 1, row: 0 }]);
    assert_eq!(lowered.program.outputs.len(), 1);
    assert_eq!(lowered.output_scores.len(), 1);
    assert_eq!(lowered.output_debug.len(), 1);
    assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
    assert_eq!(lowered.preview.preview_events.len(), 1);
    assert_eq!(
        lowered.preview.preview_events[0].site,
        Some(GridPos { col: 0, row: 0 }.into())
    );
    assert_eq!(
        lowered.preview.preview_events[0].output_lane,
        GridPos { col: 1, row: 0 }.into()
    );
}

#[test]
fn combinator_projects_are_analyzable_with_runtime_registry() {
    for piece_id in ["cadence.overlay", "cadence.arrange"] {
        let project = combinator_project(piece_id);
        let analyzed = runtime_engine(&project).analyze(project.runtime_graph());
        assert!(
            analyzed.diagnostics.is_empty(),
            "{piece_id}: {:?}",
            analyzed.diagnostics
        );
        assert_eq!(
            analyzed.outputs,
            vec![GridPos { col: 2, row: 0 }],
            "{piece_id}"
        );
    }
}
