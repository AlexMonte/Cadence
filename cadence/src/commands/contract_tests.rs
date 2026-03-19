//! Contract tests validating that backend DTO serialisation shapes match the
//! expectations encoded in the frontend bridge (`cadence_frontend/src/bridge/tauri.rs`).
//!
//! These tests catch the most dangerous class of Tauri seam bugs: a backend type
//! serialising to a JSON shape the frontend cannot parse.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::export::ExportSongResultDto;
use super::graph_commands::{GraphApplyResultDto, GraphCompilePreviewDto};
use super::project_commands::ProjectViewDto;
use super::runtime_commands::RuntimeCommitDto;
use crate::model::{
    CadenceSampleLoad, CompileMetaDto, GraphEdgeDto, GraphSnapshotDto, ProjectCompilePreviewDto,
    SemanticSnapshotDto,
};
use tessera::diagnostics::{Diagnostic, DiagnosticKind, SemanticResult};
use tessera::graph::{Edge, Graph, GraphOp, Node};
use tessera::ops::EdgeConnectProbeReason;
use tessera::types::{DomainBridgeKind, EdgeId, GridPos, PortType, TileSide};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_has_field(v: &Value, field: &str) {
    assert!(v.get(field).is_some(), "missing field `{field}` in {v}",);
}

fn assert_is_object(v: &Value) {
    assert!(v.is_object(), "expected object, got {v}");
}

fn assert_is_array(v: &Value) {
    assert!(v.is_array(), "expected array, got {v}");
}

fn sample_graph() -> Graph {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.sound".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("bd".to_string()))]),
            input_sides: Default::default(),
            output_side: Some(TileSide::RIGHT),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: BTreeMap::from([("pattern".to_string(), TileSide::LEFT)]),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".to_string(),
    };
    Graph {
        nodes,
        edges: BTreeMap::from([(edge.id.clone(), edge)]),
        name: "runtime".to_string(),
        cols: 12,
        rows: 8,
    }
}

fn sample_semantic() -> SemanticResult {
    SemanticResult {
        diagnostics: Vec::new(),
        eval_order: vec![GridPos { col: 0, row: 0 }, GridPos { col: 1, row: 0 }],
        terminals: vec![GridPos { col: 1, row: 0 }],
        output_types: BTreeMap::new(),
        domain_bridges: BTreeMap::new(),
        delay_edges: BTreeSet::new(),
    }
}

// ---------------------------------------------------------------------------
// Group A: Response DTO shapes
// ---------------------------------------------------------------------------

#[test]
fn contract_project_view_dto_shape() {
    let dto = ProjectViewDto {
        name: "Demo".to_string(),
        schema_version: 3,
        node_count: 2,
        edge_count: 1,
        dirty: false,
        path: Some("/tmp/demo.cadence.json".to_string()),
    };

    let v = serde_json::to_value(&dto).expect("serialize");
    assert_is_object(&v);
    assert_eq!(v["name"], json!("Demo"));
    assert_eq!(v["schema_version"], json!(3));
    assert_eq!(v["node_count"], json!(2));
    assert_eq!(v["edge_count"], json!(1));
    assert_eq!(v["dirty"], json!(false));
    assert_eq!(v["path"], json!("/tmp/demo.cadence.json"));

    // path must be nullable
    let dto_no_path = ProjectViewDto { path: None, ..dto };
    let v2 = serde_json::to_value(&dto_no_path).expect("serialize");
    assert_eq!(v2["path"], json!(null));
}

#[test]
fn contract_graph_compile_preview_dto_shape() {
    let dto = GraphCompilePreviewDto {
        can_compile: true,
        code: Some("s(\"bd\")".to_string()),
        exprs: vec!["s(\"bd\")".to_string()],
        diagnostics: Vec::new(),
        eval_order: vec![GridPos { col: 0, row: 0 }],
        terminals: vec![GridPos { col: 1, row: 0 }],
        compile_meta: CompileMetaDto::default(),
    };

    let v = serde_json::to_value(&dto).expect("serialize");
    assert_is_object(&v);
    assert_has_field(&v, "can_compile");
    assert_has_field(&v, "code");
    assert_has_field(&v, "exprs");
    assert_has_field(&v, "diagnostics");
    assert_has_field(&v, "eval_order");
    assert_has_field(&v, "terminals");
    assert_has_field(&v, "compile_meta");

    // exprs must be plain strings, not nested objects
    assert_is_array(&v["exprs"]);
    assert_eq!(v["exprs"][0], json!("s(\"bd\")"));

    // eval_order / terminals must be {col, row} objects
    assert_eq!(v["eval_order"][0]["col"], json!(0));
    assert_eq!(v["eval_order"][0]["row"], json!(0));

    // compile_meta must have expected sub-fields
    let meta = &v["compile_meta"];
    assert_has_field(meta, "delay_slots");
    assert_has_field(meta, "domain_bridges");
    assert_has_field(meta, "activity_events");
}

#[test]
fn contract_graph_apply_result_dto_shape() {
    let graph = sample_graph();
    let edge = graph.edges.values().next().unwrap().clone();
    let dto = GraphApplyResultDto {
        graph: GraphSnapshotDto::from(&graph),
        semantic: SemanticSnapshotDto::from(&sample_semantic()),
        preview: GraphCompilePreviewDto {
            can_compile: true,
            code: Some("s(\"bd\")".to_string()),
            exprs: vec![],
            diagnostics: Vec::new(),
            eval_order: Vec::new(),
            terminals: Vec::new(),
            compile_meta: CompileMetaDto::default(),
        },
        removed_edges: vec![GraphEdgeDto::from(&edge)],
    };

    let v = serde_json::to_value(&dto).expect("serialize");
    assert_is_object(&v);

    // graph must have nodes, edges, name, cols, rows
    let g = &v["graph"];
    assert_is_object(g);
    assert_has_field(g, "nodes");
    assert_has_field(g, "edges");
    assert_has_field(g, "name");
    assert_has_field(g, "cols");
    assert_has_field(g, "rows");

    assert_is_array(&g["nodes"]);
    let first_node = &g["nodes"][0];
    assert_has_field(first_node, "position");
    assert_has_field(first_node, "piece_id");

    assert_is_array(&g["edges"]);
    let first_edge = &g["edges"][0];
    assert_has_field(first_edge, "id");
    assert_has_field(first_edge, "from");
    assert_has_field(first_edge, "to_node");
    assert_has_field(first_edge, "to_param");

    // semantic must be an object
    assert_is_object(&v["semantic"]);
    assert_is_array(&v["semantic"]["diagnostics"]);
    assert_is_array(&v["semantic"]["eval_order"]);
    assert_is_array(&v["semantic"]["terminals"]);
    assert_is_array(&v["semantic"]["output_types"]);
    assert_is_array(&v["semantic"]["domain_bridges"]);
    assert_is_array(&v["semantic"]["delay_edges"]);

    // removed_edges must be an array of edge objects
    assert_is_array(&v["removed_edges"]);
    let re = &v["removed_edges"][0];
    assert_has_field(re, "id");
    assert_has_field(re, "from");
    assert_has_field(re, "to_node");
    assert_has_field(re, "to_param");
}

#[test]
fn contract_runtime_commit_dto_shape() {
    let dto = RuntimeCommitDto {
        success: true,
        rev: 1,
        changed: true,
        playing: true,
        code: Some("s(\"bd\")".to_string()),
        cps_expr: None,
        sample_loads: vec![CadenceSampleLoad {
            id: "bd".to_string(),
            source: "github:tidalcycles/dirt-samples".to_string(),
            aliases: BTreeMap::from([("kick".to_string(), "bd".to_string())]),
        }],
        declaration_code: vec!["let pattern = s(\"bd\")".to_string()],
        runtime_code: Some("$: pattern".to_string()),
        voice_count: 1,
        play_elapsed_ms: 42,
        request_id: Some(7),
        diagnostics: Vec::new(),
        error: None,
    };

    let v = serde_json::to_value(&dto).expect("serialize");
    assert_is_object(&v);
    for field in [
        "success",
        "rev",
        "changed",
        "playing",
        "code",
        "cps_expr",
        "sample_loads",
        "declaration_code",
        "runtime_code",
        "voice_count",
        "play_elapsed_ms",
        "request_id",
        "diagnostics",
        "error",
    ] {
        assert_has_field(&v, field);
    }

    // sample_loads items have id, source, aliases
    let sl = &v["sample_loads"][0];
    assert_eq!(sl["id"], json!("bd"));
    assert_eq!(sl["source"], json!("github:tidalcycles/dirt-samples"));
    assert_is_object(&sl["aliases"]);
    assert_eq!(sl["aliases"]["kick"], json!("bd"));
}

#[test]
fn contract_export_song_result_dto_shape() {
    let dto = ExportSongResultDto {
        exported: true,
        message: "exported song to /tmp/out.js".to_string(),
        path: Some("/tmp/out.js".to_string()),
        diagnostics: Vec::new(),
    };

    let v = serde_json::to_value(&dto).expect("serialize");
    assert_is_object(&v);
    assert_eq!(v["exported"], json!(true));
    assert_has_field(&v, "message");
    assert_has_field(&v, "path");
    assert_has_field(&v, "diagnostics");
}

#[test]
fn contract_project_compile_preview_dto_shape() {
    let dto = ProjectCompilePreviewDto {
        can_render: true,
        can_play: true,
        code: Some("s(\"bd\")".to_string()),
        diagnostics: Vec::new(),
        compile_meta: CompileMetaDto::default(),
    };

    let v = serde_json::to_value(&dto).expect("serialize");
    assert_is_object(&v);
    assert_has_field(&v, "can_render");
    assert_has_field(&v, "can_play");
    assert_has_field(&v, "code");
    assert_has_field(&v, "diagnostics");
    assert_has_field(&v, "compile_meta");
}

// ---------------------------------------------------------------------------
// Group B: Enum serialisation
// ---------------------------------------------------------------------------

#[test]
fn contract_diagnostic_kind_all_variants_tagged() {
    let pos = GridPos { col: 0, row: 0 };
    let pt = PortType::number();
    let variants: Vec<DiagnosticKind> = vec![
        DiagnosticKind::UnknownPiece {
            piece_id: "x".into(),
        },
        DiagnosticKind::UnknownNode { pos },
        DiagnosticKind::UnknownParam {
            piece_id: "x".into(),
            param: "p".into(),
        },
        DiagnosticKind::InvalidOperation { reason: "r".into() },
        DiagnosticKind::DuplicateConnection {
            to_node: pos,
            to_param: "p".into(),
        },
        DiagnosticKind::Cycle {
            involved: vec![pos],
        },
        DiagnosticKind::NoTerminalNode,
        DiagnosticKind::MultipleTerminalNodes {
            positions: vec![pos],
        },
        DiagnosticKind::UnreachableNode { position: pos },
        DiagnosticKind::TypeMismatch {
            expected: pt.clone(),
            got: pt.clone(),
            param: "p".into(),
        },
        DiagnosticKind::UnsupportedDomainCrossing {
            expected: pt.clone(),
            got: pt.clone(),
            param: "p".into(),
        },
        DiagnosticKind::DelayTypeMismatch {
            default: pt.clone(),
            feedback: pt.clone(),
        },
        DiagnosticKind::SideMismatch {
            from_pos: pos,
            to_pos: pos,
            expected_side: TileSide::RIGHT,
        },
        DiagnosticKind::NotAdjacent {
            from_pos: pos,
            to_pos: pos,
        },
        DiagnosticKind::OutputFromTerminal { position: pos },
        DiagnosticKind::MissingRequiredParam { param: "p".into() },
        DiagnosticKind::InlineNotAllowed { param: "p".into() },
        DiagnosticKind::InlineTypeMismatch {
            param: "p".into(),
            expected: pt.clone(),
            got_value: json!(42),
        },
    ];

    for variant in &variants {
        let v = serde_json::to_value(variant).expect("serialize DiagnosticKind");
        assert!(
            v.get("kind").is_some(),
            "DiagnosticKind variant missing `kind` tag: {v}"
        );
        let kind = v["kind"].as_str().expect("kind should be a string");
        // Must be snake_case (no uppercase, no hyphens)
        assert!(
            !kind.chars().any(|c| c.is_uppercase() || c == '-'),
            "kind `{kind}` is not snake_case"
        );
    }
}

#[test]
fn contract_edge_connect_probe_reason_round_trips() {
    let variants = vec![
        EdgeConnectProbeReason::UnknownSourceNode,
        EdgeConnectProbeReason::UnknownTargetNode,
        EdgeConnectProbeReason::UnknownSourcePiece,
        EdgeConnectProbeReason::UnknownTargetPiece,
        EdgeConnectProbeReason::UnknownTargetParam,
        EdgeConnectProbeReason::NotAdjacent,
        EdgeConnectProbeReason::SideMismatch,
        EdgeConnectProbeReason::OutputFromTerminal,
        EdgeConnectProbeReason::NoParamOnTargetSide,
        EdgeConnectProbeReason::TargetParamOccupied,
        EdgeConnectProbeReason::TypeMismatch,
        EdgeConnectProbeReason::UnsupportedDomain,
        EdgeConnectProbeReason::NoCompatibleParam,
    ];

    for variant in &variants {
        let v = serde_json::to_value(variant).expect("serialize probe reason");
        let s = v.as_str().expect("probe reason should serialize as string");
        assert!(
            !s.chars().any(|c| c.is_uppercase() || c == '-'),
            "probe reason `{s}` is not snake_case"
        );

        // Round-trip
        let back: EdgeConnectProbeReason =
            serde_json::from_value(v.clone()).expect("deserialize probe reason");
        assert_eq!(
            serde_json::to_value(&back).unwrap(),
            v,
            "round-trip failed for {s}"
        );
    }
}

#[test]
fn contract_domain_bridge_kind_round_trips() {
    let expected = vec![
        (DomainBridgeKind::ControlToAudio, "control_to_audio"),
        (DomainBridgeKind::AudioToControl, "audio_to_control"),
        (DomainBridgeKind::EventToControl, "event_to_control"),
    ];

    for (variant, expected_str) in &expected {
        let v = serde_json::to_value(variant).expect("serialize");
        assert_eq!(v.as_str().unwrap(), *expected_str);

        let back: DomainBridgeKind = serde_json::from_value(v).expect("deserialize");
        assert_eq!(&back, variant);
    }
}

// ---------------------------------------------------------------------------
// Group C: Tricky serde attributes
// ---------------------------------------------------------------------------

#[test]
fn contract_port_type_untagged_serialization() {
    // Plain variant serialises to a bare string
    let plain = PortType::new("pattern");
    let v = serde_json::to_value(&plain).expect("serialize plain PortType");
    // PortType serialises as either a string or a {kind, domain} object depending on variant
    // The frontend handles both forms via #[serde(untagged)]
    assert!(
        v.is_string() || v.is_object(),
        "PortType should be string or object, got {v}"
    );

    // Ensure round-trip works for both forms
    let back: PortType = serde_json::from_value(v.clone()).expect("deserialize PortType");
    assert_eq!(back.as_str(), "pattern");
}

#[test]
fn contract_graph_op_tagged_dispatch() {
    let ops: Vec<GraphOp> = vec![
        GraphOp::NodePlace {
            position: GridPos { col: 0, row: 0 },
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::new(),
        },
        GraphOp::NodeMove {
            from: GridPos { col: 0, row: 0 },
            to: GridPos { col: 1, row: 0 },
        },
        GraphOp::NodeSwap {
            a: GridPos { col: 0, row: 0 },
            b: GridPos { col: 1, row: 0 },
        },
        GraphOp::NodeRemove {
            position: GridPos { col: 0, row: 0 },
        },
        GraphOp::EdgeConnect {
            edge_id: None,
            from: GridPos { col: 0, row: 0 },
            to_node: GridPos { col: 1, row: 0 },
            to_param: "pattern".to_string(),
        },
        GraphOp::EdgeDisconnect {
            edge_id: EdgeId::new(),
        },
        GraphOp::ParamSetInline {
            position: GridPos { col: 0, row: 0 },
            param_id: "value".to_string(),
            value: json!("c3"),
        },
        GraphOp::ParamClearInline {
            position: GridPos { col: 0, row: 0 },
            param_id: "value".to_string(),
        },
        GraphOp::ParamSetSide {
            position: GridPos { col: 0, row: 0 },
            param_id: "value".to_string(),
            side: TileSide::LEFT,
        },
        GraphOp::ParamClearSide {
            position: GridPos { col: 0, row: 0 },
            param_id: "value".to_string(),
        },
        GraphOp::OutputSetSide {
            position: GridPos { col: 0, row: 0 },
            side: TileSide::RIGHT,
        },
        GraphOp::OutputClearSide {
            position: GridPos { col: 0, row: 0 },
        },
        GraphOp::NodeAutoWire {
            position: GridPos { col: 0, row: 0 },
        },
        GraphOp::NodeSetLabel {
            position: GridPos { col: 0, row: 0 },
            label: Some("my tile".to_string()),
        },
        GraphOp::NodeSetState {
            position: GridPos { col: 0, row: 0 },
            state: None,
        },
        GraphOp::ResizeGrid { cols: 14, rows: 6 },
    ];

    for op in &ops {
        let v = serde_json::to_value(op).expect("serialize GraphOp");
        assert_is_object(&v);
        let tag = v
            .get("op")
            .and_then(Value::as_str)
            .expect("GraphOp missing `op` tag")
            .to_string();
        assert!(
            !tag.chars().any(|c| c.is_uppercase() || c == '-'),
            "op tag `{tag}` is not snake_case"
        );

        // Round-trip
        let back: GraphOp = serde_json::from_value(v).expect("deserialize GraphOp");
        assert_eq!(
            serde_json::to_value(&back).unwrap(),
            serde_json::to_value(op).unwrap(),
            "round-trip failed for op `{tag}`"
        );
    }
}

// ---------------------------------------------------------------------------
// Group D: Error return shape
// ---------------------------------------------------------------------------

#[test]
fn contract_graph_apply_ops_error_is_diagnostic_vec() {
    let diagnostics = vec![
        Diagnostic::error(
            DiagnosticKind::NoTerminalNode,
            Some(GridPos { col: 0, row: 0 }),
        ),
        Diagnostic::warning(
            DiagnosticKind::UnreachableNode {
                position: GridPos { col: 2, row: 0 },
            },
            Some(GridPos { col: 2, row: 0 }),
        ),
    ];

    // graph_apply_ops returns Result<_, Vec<Diagnostic>>. The error variant
    // must serialize as a JSON array of diagnostic objects.
    let v = serde_json::to_value(&diagnostics).expect("serialize diagnostic vec");
    assert_is_array(&v);
    assert_eq!(v.as_array().unwrap().len(), 2);

    for entry in v.as_array().unwrap() {
        assert_is_object(entry);
        assert_has_field(entry, "kind");
        assert_has_field(entry, "site");
        assert_has_field(entry, "severity");
        // edge_id may be null but the field must exist
        assert_has_field(entry, "edge_id");

        // severity must be a known string
        let sev = entry["severity"].as_str().expect("severity is string");
        assert!(
            ["error", "warning", "info"].contains(&sev),
            "unknown severity: {sev}"
        );
    }
}
