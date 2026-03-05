use std::collections::BTreeMap;

use serde_json::{Number, Value};

use super::code_expr::CodeExpr;
use super::compiler::compile_graph;
use super::diagnostics::DiagnosticKind;
use super::graph::{Edge, Graph, Node, ProjectDocument};
use super::piece::ParamSchema;
use super::piece_registry::PieceRegistry;
use super::semantic::semantic_pass;
use super::types::{EdgeId, GridPos, PortType, TileSide, adjacent_in_direction};

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
    assert!(number.accepts(&PortType::Number));
    assert!(!number.accepts(&PortType::Text));
    assert!(number.accepts(&PortType::Any));
    assert!(text.accepts(&PortType::Any));
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
    let rhythm = ParamSchema::Rhythm {
        default: "bd sd".to_string(),
        can_inline: true,
    };
    let pattern = ParamSchema::Pattern { can_inline: false };
    assert!(number.default_expr().is_some());
    assert!(text.default_expr().is_some());
    assert!(rhythm.default_expr().is_some());
    assert!(pattern.default_expr().is_none());
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
},
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.output".into(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
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
    }
}

#[test]
fn semantic_pass_accepts_valid_graph() {
    let graph = simple_graph();
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(sem.errors.is_empty());
    assert_eq!(sem.terminal, Some(GridPos { col: 1, row: 0 }));
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
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::from([("factor".to_string(), Value::Number(Number::from(2)))]),
            input_sides: BTreeMap::from([("pattern".to_string(), TileSide::South)]),
            output_side: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
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
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors.is_empty(),
        "expected no semantic errors with side overrides, got {:?}",
        sem.errors
    );

    let expr = compile_graph(&graph, &registry, &sem).expect("compile");
    assert_eq!(expr.render(), "note(\"c3\").fast(2)");
}

#[test]
fn compile_graph_is_deterministic() {
    let graph = simple_graph();
    let registry = PieceRegistry::default_strudel();
    let first_sem = semantic_pass(&graph, &registry);
    let first = compile_graph(&graph, &registry, &first_sem)
        .expect("compile")
        .render();
    let second_sem = semantic_pass(&graph, &registry);
    let second = compile_graph(&graph, &registry, &second_sem)
        .expect("compile")
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

    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
},
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.slow".into(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".into(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
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
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);

    assert!(
        sem.errors
            .iter()
            .any(|diag| matches!(diag.kind, DiagnosticKind::Cycle { .. }))
    );
}

#[test]
fn semantic_reports_no_terminal_node() {
    let mut graph = simple_graph();
    graph.nodes.remove(&GridPos { col: 1, row: 0 });
    graph.edges.clear();
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
},
    );
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
},
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };
    graph.edges.insert(edge.id.clone(), edge);
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
        },
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 2, row: 0 },
        to_param: "pattern".to_string(),
    };
    graph.edges.insert(edge.id.clone(), edge);
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
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
        edges: BTreeMap::from([
            (edge_a.id.clone(), edge_a),
            (edge_b.id.clone(), edge_b),
        ]),
        name: "type_mismatch".to_string(),
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
},
    );
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.stack".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );
    nodes.insert(
        GridPos { col: 2, row: 1 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );

    let edge_a = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 0 },
        to_node: GridPos { col: 1, row: 1 },
        to_param: "b".to_string(),
    };
    let edge_b = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 1, row: 1 },
        to_node: GridPos { col: 2, row: 1 },
        to_param: "pattern".to_string(),
    };

    let graph = Graph {
        nodes,
        edges: BTreeMap::from([
            (edge_a.id.clone(), edge_a),
            (edge_b.id.clone(), edge_b),
        ]),
        name: "side_mismatch".to_string(),
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
},
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
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
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
},
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::from([(
                "pattern".to_string(),
                Value::String("mini(\"bd\")".to_string()),
            )]),
            input_sides: Default::default(),
            output_side: None,
        },
    );
    nodes.insert(
        GridPos { col: 2, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
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
        edges: BTreeMap::from([
            (edge_a.id.clone(), edge_a),
            (edge_b.id.clone(), edge_b),
        ]),
        name: "inline_not_allowed".to_string(),
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
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
},
    );
    nodes.insert(
        GridPos { col: 0, row: 2 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );
    nodes.insert(
        GridPos { col: 1, row: 1 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );
    nodes.insert(
        GridPos { col: 2, row: 1 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );
    nodes.insert(
        GridPos { col: 8, row: 8 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
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
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    assert!(
        sem.errors
            .iter()
            .any(|diag| { matches!(diag.kind, DiagnosticKind::DuplicateConnection { .. }) })
    );
    assert!(
        sem.errors
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
},
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.fast".to_string(),
            inline_params: BTreeMap::from([("factor".to_string(), Value::Number(Number::from(2)))]),
        input_sides: Default::default(),
        output_side: None,
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
        },
    );
    nodes.insert(
        GridPos { col: 3, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
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
    };
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph, &registry);
    let code = compile_graph(&graph, &registry, &sem)
        .expect("compile chain")
        .render();
    assert_eq!(code, "s(\"bd\").fast(2).gain(0.5)");
}
