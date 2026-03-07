use std::collections::BTreeMap;

use serde_json::{Number, Value};

use super::piece_registry::default_strudel_registry;
use super::strudel_schema::{pattern_port, pattern_schema, rhythm_schema};
use tile_graph::code_expr::CodeExpr;
use tile_graph::compiler::{CompileMode, compile_graph};
use tile_graph::diagnostics::{DiagnosticKind, DiagnosticSeverity};
use tile_graph::graph::{Edge, Graph, Node, ProjectDocument};
use tile_graph::piece::{ParamDef, ParamSchema, Piece, PieceDef, PieceInputs};
use tile_graph::piece_registry::PieceRegistry;
use tile_graph::semantic::semantic_pass;
use tile_graph::types::{
    EdgeId, GridPos, PieceCategory, PortType, TileSide, adjacent_in_direction,
};

#[test]
fn tile_side_faces_matrix_matches_expected_pairs() {
    let sides = [
        TileSide::North,
        TileSide::South,
        TileSide::East,
        TileSide::West,
    ];
    for left in sides {
        for right in sides {
            let expected = matches!(
                (left, right),
                (TileSide::East, TileSide::West)
                    | (TileSide::West, TileSide::East)
                    | (TileSide::North, TileSide::South)
                    | (TileSide::South, TileSide::North)
            );
            assert_eq!(left.faces(right), expected);
        }
    }
}

#[test]
fn adjacent_in_direction_covers_all_directions_and_negative_coords() {
    let origin = GridPos { col: -3, row: 5 };
    assert_eq!(
        adjacent_in_direction(&origin, &TileSide::North),
        GridPos { col: -3, row: 4 }
    );
    assert_eq!(
        adjacent_in_direction(&origin, &TileSide::South),
        GridPos { col: -3, row: 6 }
    );
    assert_eq!(
        adjacent_in_direction(&origin, &TileSide::West),
        GridPos { col: -4, row: 5 }
    );
    assert_eq!(
        adjacent_in_direction(&origin, &TileSide::East),
        GridPos { col: -2, row: 5 }
    );
}

#[test]
fn grid_pos_ordering_is_col_then_row() {
    let mut positions = vec![
        GridPos { col: 2, row: 0 },
        GridPos { col: 1, row: 5 },
        GridPos { col: 1, row: -1 },
        GridPos { col: -1, row: 2 },
    ];
    positions.sort();
    assert_eq!(
        positions,
        vec![
            GridPos { col: -1, row: 2 },
            GridPos { col: 1, row: -1 },
            GridPos { col: 1, row: 5 },
            GridPos { col: 2, row: 0 }
        ]
    );
}

#[test]
fn param_schema_accepts_with_any_wildcard() {
    let number = ParamSchema::Number {
        default: 1.0,
        min: None,
        max: None,
        can_inline: true,
    };
    let text = ParamSchema::Text {
        default: "x".to_string(),
        can_inline: true,
    };
    assert!(number.accepts(&PortType::number()));
    assert!(!number.accepts(&PortType::text()));
    assert!(number.accepts(&PortType::any()));
    assert!(text.accepts(&PortType::any()));
}

#[test]
fn param_schema_default_expr_contract() {
    let number = ParamSchema::Number {
        default: 2.0,
        min: None,
        max: None,
        can_inline: true,
    };
    let text = ParamSchema::Text {
        default: "bd".to_string(),
        can_inline: true,
    };
    let rhythm = rhythm_schema("bd sd", true);
    let pattern = pattern_schema();
    assert!(number.default_expr().is_some());
    assert!(text.default_expr().is_some());
    assert!(rhythm.default_expr().is_some());
    assert!(pattern.default_expr().is_none());
}

#[test]
fn transform_pieces_follow_pattern_chain_contract() {
    let registry = default_strudel_registry();
    let defs = registry.all_defs();
    let transforms = defs
        .into_iter()
        .filter(|def| matches!(def.category, PieceCategory::Transform))
        .collect::<Vec<_>>();
    assert!(
        !transforms.is_empty(),
        "expected at least one transform piece"
    );

    for def in transforms {
        assert_eq!(
            def.output_type,
            Some(pattern_port()),
            "transform '{}' must output Pattern for chainability",
            def.id
        );
        let receiver = def
            .params
            .iter()
            .find(|param| param.id == "pattern")
            .unwrap_or_else(|| {
                panic!(
                    "transform '{}' is missing required 'pattern' receiver param",
                    def.id
                )
            });
        assert_eq!(
            receiver.schema.expected_port_type(),
            pattern_port(),
            "transform '{}' receiver must be Pattern-compatible",
            def.id
        );
        assert!(
            !receiver.schema.can_inline(),
            "transform '{}' receiver must require a connected pattern input",
            def.id
        );
        assert!(
            receiver.required,
            "transform '{}' pattern receiver must be required",
            def.id
        );
    }
}

#[test]
fn code_expr_render_variants() {
    let literal_number = CodeExpr::Literal(Value::Number(Number::from(2)));
    assert_eq!(literal_number.render(), "2");

    let literal_string = CodeExpr::Literal(Value::String("bd sd".to_string()));
    assert_eq!(literal_string.render(), "\"bd sd\"");

    let call = CodeExpr::Call {
        func: "stack".to_string(),
        args: vec![
            CodeExpr::Ident("a".to_string()),
            CodeExpr::Ident("b".to_string()),
        ],
    };
    assert_eq!(call.render(), "stack(a, b)");

    let method = CodeExpr::Method {
        receiver: Box::new(CodeExpr::Call {
            func: "note".to_string(),
            args: vec![CodeExpr::Literal(Value::String("c3".to_string()))],
        }),
        method: "fast".to_string(),
        args: vec![CodeExpr::Literal(Value::Number(Number::from(2)))],
    };
    assert_eq!(method.render(), "note(\"c3\").fast(2)");

    let nested = CodeExpr::Method {
        receiver: Box::new(method),
        method: "gain".to_string(),
        args: vec![CodeExpr::Literal(Value::Number(
            Number::from_f64(0.5).expect("number"),
        ))],
    };
    assert_eq!(nested.render(), "note(\"c3\").fast(2).gain(0.5)");
}

fn simple_graph() -> Graph {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.sound".into(),
            inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.output".into(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".into(),
    };

    Graph {
        nodes,
        edges: BTreeMap::from([(edge.id.clone(), edge)]),
        name: "test".to_string(),
        cols: 9,
        rows: 9,
    }
}

#[test]
fn semantic_pass_accepts_valid_graph() {
    let graph = simple_graph();
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(sem.diagnostics.is_empty());
    assert_eq!(
        sem.terminals.first().cloned(),
        Some(GridPos { col: 1, row: 0 })
    );
}

#[test]
fn semantic_respects_node_side_overrides() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("c3".to_string()))]),
            input_sides: Default::default(),
            output_side: Some(TileSide::North),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::from([("factor".to_string(), Value::Number(Number::from(2)))]),
            input_sides: BTreeMap::from([("pattern".to_string(), TileSide::South)]),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge_a = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 1 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".to_string(),
    };
    let edge_b = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };
    let graph = Graph {
        nodes,
        edges: BTreeMap::from([
            (edge_a.id.clone(), edge_a.clone()),
            (edge_b.id.clone(), edge_b.clone()),
        ]),
        name: "overrides".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics.is_empty(),
        "expected no semantic errors with side overrides, got {:?}",
        sem.diagnostics
    );

    let program = compile_graph(&graph, &registry, &sem, CompileMode::Preview).expect("compile");
    assert_eq!(
        program
            .terminals
            .first()
            .expect("terminal expression")
            .render(),
        "note(\"c3\").fast(2)"
    );
}

#[test]
fn compile_graph_is_deterministic() {
    let graph = simple_graph();
    let registry = default_strudel_registry();
    let first_sem = semantic_pass(&graph, &registry);
    let first = compile_graph(&graph, &registry, &first_sem, CompileMode::Preview)
        .expect("compile")
        .terminals
        .first()
        .expect("terminal expression")
        .render();
    let second_sem = semantic_pass(&graph, &registry);
    let second = compile_graph(&graph, &registry, &second_sem, CompileMode::Preview)
        .expect("compile")
        .terminals
        .first()
        .expect("terminal expression")
        .render();
    assert_eq!(first, second);
}

#[test]
fn semantic_detects_non_adjacent_edge() {
    let mut graph = simple_graph();
    graph.nodes.insert(
        GridPos { col: -4, row: 12 },
        Node {
            piece_id: "strudel.sound".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let first_edge = graph.edges.values().next().cloned().expect("edge");
    graph.edges.clear();
    graph.edges.insert(
        first_edge.id.clone(),
        Edge {
            from: GridPos { col: -4, row: 12 },
            ..first_edge
        },
    );

    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| { matches!(diag.kind, DiagnosticKind::NotAdjacent { .. }) })
    );
}

#[test]
fn semantic_detects_cycle() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.fast".into(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.slow".into(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".into(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let mut edges = BTreeMap::new();
    let e1 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".into(),
    };
    let e2 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 0, row: 0 },
        to_param: "pattern".into(),
    };
    let e3 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".into(),
    };
    edges.insert(e1.id.clone(), e1);
    edges.insert(e2.id.clone(), e2);
    edges.insert(e3.id.clone(), e3);

    let graph = Graph {
        nodes,
        edges,
        name: "test".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);

    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::Cycle { .. }))
    );
}

#[test]
fn semantic_reports_no_terminal_node() {
    let mut graph = simple_graph();
    graph.nodes.remove(&GridPos { col: 1, row: 0 });
    graph.edges.clear();
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::NoTerminalNode))
    );
}

#[test]
fn semantic_reports_multiple_terminals() {
    let mut graph = simple_graph();
    graph.nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| { matches!(diag.kind, DiagnosticKind::MultipleTerminalNodes { .. }) })
    );
}

#[test]
fn semantic_reports_output_from_terminal() {
    let mut graph = simple_graph();
    graph.nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };
    graph.edges.insert(edge.id.clone(), edge);
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| { matches!(diag.kind, DiagnosticKind::OutputFromTerminal { .. }) })
    );
}

#[test]
fn semantic_reports_inline_type_mismatch() {
    let mut graph = simple_graph();
    graph.nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::from([(
                "factor".to_string(),
                Value::String("oops".to_string()),
            )]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };
    graph.edges.insert(edge.id.clone(), edge);
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| { matches!(diag.kind, DiagnosticKind::InlineTypeMismatch { .. }) })
    );
}

#[test]
fn semantic_reports_unknown_piece() {
    let mut graph = simple_graph();
    if let Some(node) = graph.nodes.get_mut(&GridPos { col: 0, row: 0 }) {
        node.piece_id = "strudel.nonexistent".to_string();
    }
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::UnknownPiece { .. }))
    );
}

#[test]
fn semantic_reports_type_mismatch() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.text".to_string(),
            inline_params: BTreeMap::from([(
                "value".to_string(),
                Value::String("not-a-number".to_string()),
            )]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let edge_a = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 1 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "factor".to_string(),
    };
    let edge_b = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };

    let graph = Graph {
        nodes,
        edges: BTreeMap::from([(edge_a.id.clone(), edge_a), (edge_b.id.clone(), edge_b)]),
        name: "type_mismatch".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::TypeMismatch { .. }))
    );
}

#[test]
fn semantic_reports_side_mismatch() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.stack".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 1 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let edge_a = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "in_n".to_string(),
    };
    let edge_b = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 1 },
        to_node: GridPos { col: 2, row: 1 },
        to_param: "pattern".to_string(),
    };

    let graph = Graph {
        nodes,
        edges: BTreeMap::from([(edge_a.id.clone(), edge_a), (edge_b.id.clone(), edge_b)]),
        name: "side_mismatch".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::SideMismatch { .. }))
    );
}

#[test]
fn semantic_reports_missing_required_param() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };

    let graph = Graph {
        nodes,
        edges: BTreeMap::from([(edge.id.clone(), edge)]),
        name: "missing_required".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::MissingRequiredParam { .. }))
    );
}

#[test]
fn semantic_reports_inline_not_allowed() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::from([(
                "pattern".to_string(),
                Value::String("s(\"bd\")".to_string()),
            )]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge_a = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".to_string(),
    };
    let edge_b = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };

    let graph = Graph {
        nodes,
        edges: BTreeMap::from([(edge_a.id.clone(), edge_a), (edge_b.id.clone(), edge_b)]),
        name: "inline_not_allowed".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::InlineNotAllowed { .. }))
    );
}

#[test]
fn semantic_reports_duplicate_connection_and_unreachable() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 0, row: 2 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 1 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 8, row: 8 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let edge1 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "pattern".to_string(),
    };
    let edge2 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 2 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "pattern".to_string(),
    };
    let edge3 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 1 },
        to_node: GridPos { col: 2, row: 1 },
        to_param: "pattern".to_string(),
    };

    let graph = Graph {
        nodes,
        edges: BTreeMap::from([
            (edge1.id.clone(), edge1),
            (edge2.id.clone(), edge2),
            (edge3.id.clone(), edge3),
        ]),
        name: "dup".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| { matches!(diag.kind, DiagnosticKind::DuplicateConnection { .. }) })
    );
    assert!(
        sem.diagnostics
            .iter()
            .any(|diag| { matches!(diag.kind, DiagnosticKind::UnreachableNode { .. }) })
    );
}

#[test]
fn project_document_roundtrip_preserves_grid_keys() {
    let graph = simple_graph();
    let project = ProjectDocument::new("demo".to_string(), graph.clone());
    let payload = serde_json::to_string(&project).expect("serialize");
    let parsed: ProjectDocument = serde_json::from_str(payload.as_str()).expect("deserialize");
    assert_eq!(parsed.graph.nodes.len(), graph.nodes.len());
    assert_eq!(parsed.graph.edges.len(), graph.edges.len());
    assert!(parsed.graph.nodes.contains_key(&GridPos { col: 0, row: 0 }));
    assert!(parsed.graph.nodes.contains_key(&GridPos { col: 1, row: 0 }));
}

#[test]
fn compile_render_preserves_readable_method_chain() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.sound".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("bd".to_string()))]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::from([("factor".to_string(), Value::Number(Number::from(2)))]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.gain".to_string(),
            inline_params: BTreeMap::from([(
                "amount".to_string(),
                Value::Number(Number::from_f64(0.5).expect("number")),
            )]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 3, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let mut edges = BTreeMap::new();
    let edge1 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".to_string(),
    };
    let edge2 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };
    let edge3 = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 2, row: 0 },
        to_node: GridPos { col: 3, row: 0 },
        to_param: "pattern".to_string(),
    };
    edges.insert(edge1.id.clone(), edge1);
    edges.insert(edge2.id.clone(), edge2);
    edges.insert(edge3.id.clone(), edge3);

    let graph = Graph {
        nodes,
        edges,
        name: "test".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    let code = compile_graph(&graph, &registry, &sem, CompileMode::Preview)
        .expect("compile chain")
        .terminals
        .first()
        .expect("terminal expression")
        .render();
    assert_eq!(code, "s(\"bd\").fast(2).gain(0.5)");
}

#[test]
fn compile_graph_returns_all_terminal_expressions() {
    let mut graph = simple_graph();
    graph.nodes.insert(
        GridPos { col: 0, row: 2 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("g3".to_string()))]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    graph.nodes.insert(
        GridPos { col: 1, row: 2 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let extra = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 2 },
        to_node: GridPos { col: 1, row: 2 },
        to_param: "pattern".to_string(),
    };
    graph.edges.insert(extra.id.clone(), extra);

    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(sem.is_valid(), "sem diagnostics: {:?}", sem.diagnostics);
    assert_eq!(sem.terminals.len(), 2);
    assert!(sem.diagnostics.iter().any(|diag| {
        matches!(diag.kind, DiagnosticKind::MultipleTerminalNodes { .. })
            && diag.severity == DiagnosticSeverity::Warning
    }));

    let program = compile_graph(&graph, &registry, &sem, CompileMode::Preview).expect("compile");
    assert_eq!(program.terminals.len(), 2);
}

#[test]
fn stack_piece_compiles_variadic_group_inputs_in_param_order() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 1 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("c3".to_string()))]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("e3".to_string()))]),
            input_sides: Default::default(),
            output_side: Some(TileSide::South),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 2 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("g3".to_string()))]),
            input_sides: Default::default(),
            output_side: Some(TileSide::North),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.stack".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 1 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let edge_w = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 1 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "in_w".to_string(),
    };
    let edge_n = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "in_n".to_string(),
    };
    let edge_s = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 2 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "in_s".to_string(),
    };
    let edge_out = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 1 },
        to_node: GridPos { col: 2, row: 1 },
        to_param: "pattern".to_string(),
    };
    let graph = Graph {
        nodes,
        edges: BTreeMap::from([
            (edge_w.id.clone(), edge_w),
            (edge_n.id.clone(), edge_n),
            (edge_s.id.clone(), edge_s),
            (edge_out.id.clone(), edge_out),
        ]),
        name: "variadic".to_string(),
        cols: 9,
        rows: 9,
    };

    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(sem.is_valid(), "sem diagnostics: {:?}", sem.diagnostics);
    let rendered = compile_graph(&graph, &registry, &sem, CompileMode::Preview)
        .expect("compile")
        .terminals
        .first()
        .expect("terminal expression")
        .render();
    assert_eq!(rendered, "stack(note(\"c3\"), note(\"e3\"), note(\"g3\"))");
}

#[test]
fn compiler_runtime_mode_emits_state_updates_for_stateful_pieces() {
    struct StatefulIncPiece {
        def: PieceDef,
    }

    impl StatefulIncPiece {
        fn new() -> Self {
            Self {
                def: PieceDef {
                    id: "test.stateful_inc".to_string(),
                    label: "stateful_inc".to_string(),
                    category: PieceCategory::Control,
                    params: Vec::new(),
                    output_type: Some(PortType::number()),
                    output_side: Some(TileSide::East),
                    description: Some("test".to_string()),
                },
            }
        }
    }

    impl Piece for StatefulIncPiece {
        fn def(&self) -> &PieceDef {
            &self.def
        }

        fn compile(
            &self,
            _inputs: &PieceInputs,
            _inline_params: &BTreeMap<String, Value>,
        ) -> CodeExpr {
            CodeExpr::Literal(Value::Number(Number::from(0)))
        }

        fn initial_state(&self) -> Option<Value> {
            Some(Value::Number(Number::from(0)))
        }

        fn compile_stateful(
            &self,
            _inputs: &PieceInputs,
            _inline_params: &BTreeMap<String, Value>,
            state: &Value,
        ) -> (CodeExpr, Value) {
            let next = state.as_u64().unwrap_or(0) + 1;
            (
                CodeExpr::Literal(Value::Number(Number::from(next))),
                Value::Number(Number::from(next)),
            )
        }
    }

    struct NumberTerminalPiece {
        def: PieceDef,
    }

    impl NumberTerminalPiece {
        fn new() -> Self {
            Self {
                def: PieceDef {
                    id: "test.number_output".to_string(),
                    label: "number_output".to_string(),
                    category: PieceCategory::Output,
                    params: vec![ParamDef {
                        id: "value".to_string(),
                        label: "value".to_string(),
                        side: TileSide::West,
                        schema: ParamSchema::Number {
                            default: 0.0,
                            min: None,
                            max: None,
                            can_inline: false,
                        },
                        variadic_group: None,
                        required: true,
                    }],
                    output_type: None,
                    output_side: None,
                    description: Some("test".to_string()),
                },
            }
        }
    }

    impl Piece for NumberTerminalPiece {
        fn def(&self) -> &PieceDef {
            &self.def
        }

        fn compile(
            &self,
            inputs: &PieceInputs,
            _inline_params: &BTreeMap<String, Value>,
        ) -> CodeExpr {
            inputs
                .get("value")
                .cloned()
                .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(0))))
        }
    }

    let mut registry = PieceRegistry::new();
    registry.register(StatefulIncPiece::new());
    registry.register(NumberTerminalPiece::new());

    let stateful_pos = GridPos { col: 0, row: 0 };
    let terminal_pos = GridPos { col: 1, row: 0 };
    let mut nodes = BTreeMap::new();
    nodes.insert(
        stateful_pos.clone(),
        Node {
            piece_id: "test.stateful_inc".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        terminal_pos.clone(),
        Node {
            piece_id: "test.number_output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: stateful_pos.clone(),
        to_node: terminal_pos,
        to_param: "value".to_string(),
    };
    let graph = Graph {
        nodes,
        edges: BTreeMap::from([(edge.id.clone(), edge)]),
        name: "stateful".to_string(),
        cols: 9,
        rows: 9,
    };
    let sem = semantic_pass(&graph, &registry);
    assert!(sem.is_valid(), "sem diagnostics: {:?}", sem.diagnostics);

    let preview = compile_graph(&graph, &registry, &sem, CompileMode::Preview).expect("preview");
    assert!(preview.state_updates.is_empty());

    let runtime = compile_graph(&graph, &registry, &sem, CompileMode::Runtime).expect("runtime");
    assert_eq!(runtime.state_updates.len(), 1);
    assert_eq!(runtime.state_updates[0].position, stateful_pos);
    assert_eq!(
        runtime.state_updates[0].state,
        Value::Number(Number::from(1))
    );
}

#[test]
fn strudel_symbol_tiles_compile_and_type_check_in_registry() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 1 },
        Node {
            piece_id: "strudel.sound".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("bd".to_string()))]),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 2 },
        Node {
            piece_id: "strudel.sound".to_string(),
            inline_params: BTreeMap::from([(
                "value".to_string(),
                Value::String("bd ~".to_string()),
            )]),
            input_sides: Default::default(),
            output_side: Some(TileSide::North),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.mask".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 1 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
            label: None,
            node_state: None,
        },
    );

    let edge_pattern = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 1 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "pattern".to_string(),
    };
    let edge_trigger = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 2 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "by".to_string(),
    };
    let edge_out = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 1 },
        to_node: GridPos { col: 2, row: 1 },
        to_param: "pattern".to_string(),
    };
    let graph = Graph {
        nodes,
        edges: BTreeMap::from([
            (edge_pattern.id.clone(), edge_pattern),
            (edge_trigger.id.clone(), edge_trigger),
            (edge_out.id.clone(), edge_out),
        ]),
        name: "symbols".to_string(),
        cols: 9,
        rows: 9,
    };
    let registry = default_strudel_registry();
    let sem = semantic_pass(&graph, &registry);
    assert!(sem.is_valid(), "sem diagnostics: {:?}", sem.diagnostics);
    let rendered = compile_graph(&graph, &registry, &sem, CompileMode::Preview)
        .expect("compile")
        .terminals
        .first()
        .expect("terminal expression")
        .render();
    assert!(rendered.contains(".mask("), "rendered code: {rendered}");
}
