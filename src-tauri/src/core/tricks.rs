use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::core::pieces::tricks::default_expr_for_input;
use crate::core::pieces::{
    GeneratedTrickPiece, TRICK_INPUT_1_ID, TRICK_INPUT_2_ID, TRICK_INPUT_3_ID, TRICK_OUTPUT_ID,
};
use crate::core::strudel_schema::pattern_port;
use crate::model::{
    CadenceProjectDocument, CadenceTrickDef, CadenceTrickInput, CadenceTrickSignature,
};
use tile_graph::code_expr::CodeExpr;
use tile_graph::compiler::{CompileMode, compile_node_expr};
use tile_graph::diagnostics::{Diagnostic, DiagnosticKind};
use tile_graph::piece_registry::PieceRegistry;
use tile_graph::semantic::semantic_pass;
use tile_graph::types::{GridPos, PortType};

#[derive(Debug, Clone)]
pub struct CompiledTrick {
    pub trick_id: String,
    pub display_name: String,
    pub binding_name: String,
    pub signature: CadenceTrickSignature,
    pub body: CodeExpr,
}

impl CompiledTrick {
    pub fn to_piece(&self) -> GeneratedTrickPiece {
        GeneratedTrickPiece::new(
            self.trick_id.as_str(),
            self.display_name.as_str(),
            self.binding_name.as_str(),
            self.signature.inputs.as_slice(),
        )
    }
}

pub fn analyze_trick(
    graph: &tile_graph::graph::Graph,
    registry: &PieceRegistry,
) -> Result<CadenceTrickSignature, Vec<Diagnostic>> {
    let mut inputs = Vec::<CadenceTrickInput>::new();
    let mut output_positions = Vec::<GridPos>::new();
    let mut diagnostics = Vec::<Diagnostic>::new();

    for (pos, node) in &graph.nodes {
        match node.piece_id.as_str() {
            TRICK_INPUT_1_ID | TRICK_INPUT_2_ID | TRICK_INPUT_3_ID => {
                let slot = trick_slot_from_piece_id(node.piece_id.as_str()).unwrap_or(1);
                let label = node
                    .inline_params
                    .get("label")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| format!("input {slot}"));
                let port_type = node
                    .inline_params
                    .get("port_type")
                    .and_then(Value::as_str)
                    .map(PortType::from)
                    .unwrap_or_else(pattern_port);
                let required = node
                    .inline_params
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let is_receiver = node
                    .inline_params
                    .get("is_receiver")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let default_value = node.inline_params.get("default_value").cloned();

                inputs.push(CadenceTrickInput {
                    slot,
                    pos: pos.clone(),
                    label,
                    port_type,
                    required,
                    is_receiver,
                    default_value,
                });
            }
            TRICK_OUTPUT_ID => {
                output_positions.push(pos.clone());
            }
            _ => {}
        }
    }

    if inputs.len() > 3 {
        diagnostics.push(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: "trick may declare at most 3 inputs".into(),
            },
            inputs.get(3).map(|input| input.pos.clone()),
        ));
    }

    let mut seen_slots = BTreeSet::new();
    for input in &inputs {
        if !seen_slots.insert(input.slot) {
            diagnostics.push(Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: format!("duplicate trick input slot {}", input.slot),
                },
                Some(input.pos.clone()),
            ));
        }
    }

    if output_positions.is_empty() {
        diagnostics.push(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: "trick requires exactly one output".into(),
            },
            None,
        ));
    } else if output_positions.len() > 1 {
        diagnostics.push(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: "trick requires exactly one output".into(),
            },
            output_positions.get(1).cloned(),
        ));
    }

    let receiver_count = inputs.iter().filter(|input| input.is_receiver).count();
    if receiver_count > 1 {
        diagnostics.push(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: "trick may declare at most one receiver input".into(),
            },
            inputs
                .iter()
                .find(|input| input.is_receiver)
                .map(|input| input.pos.clone()),
        ));
    }

    for input in &inputs {
        if input.is_receiver && input.port_type != pattern_port() {
            diagnostics.push(Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: "receiver input must use port type pattern".into(),
                },
                Some(input.pos.clone()),
            ));
        }
    }

    for input in &inputs {
        for edge in graph.edges.values().filter(|edge| edge.from == input.pos) {
            let Some(target_node) = graph.nodes.get(&edge.to_node) else {
                continue;
            };
            let Some(target_piece) = registry.get(target_node.piece_id.as_str()) else {
                continue;
            };
            let Some(param_def) = target_piece
                .def()
                .params
                .iter()
                .find(|param| param.id == edge.to_param)
            else {
                continue;
            };
            if !param_def.schema.accepts(&input.port_type) {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticKind::TypeMismatch {
                            expected: param_def.schema.expected_port_type(),
                            got: input.port_type.clone(),
                            param: edge.to_param.clone(),
                        },
                        Some(edge.to_node.clone()),
                    )
                    .with_edge(edge.id.clone()),
                );
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    inputs.sort_by_key(|input| input.slot);
    Ok(CadenceTrickSignature {
        inputs,
        output_pos: output_positions
            .into_iter()
            .next()
            .expect("checked output existence"),
    })
}

pub fn compile_tricks(
    project: &CadenceProjectDocument,
    trick_registry: &PieceRegistry,
) -> (Vec<CompiledTrick>, Vec<Diagnostic>) {
    let mut compiled = Vec::<CompiledTrick>::new();
    let mut diagnostics = Vec::<Diagnostic>::new();
    let mut used_names = BTreeSet::<String>::new();

    for trick in &project.init_stage.tricks {
        match compile_trick(trick, trick_registry, &mut used_names) {
            Ok(result) => compiled.push(result),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    (compiled, diagnostics)
}

fn compile_trick(
    trick: &CadenceTrickDef,
    trick_registry: &PieceRegistry,
    used_names: &mut BTreeSet<String>,
) -> Result<CompiledTrick, Vec<Diagnostic>> {
    let sem = semantic_pass(&trick.graph, trick_registry);
    let mut diagnostics = Vec::<Diagnostic>::new();
    let signature = match analyze_trick(&trick.graph, trick_registry) {
        Ok(signature) => signature,
        Err(mut errors) => {
            diagnostics.append(&mut errors);
            return Err(diagnostics);
        }
    };

    if !sem.diagnostics.is_empty() {
        let mut sem_errors = sem.diagnostics.clone();
        diagnostics.append(&mut sem_errors);
    }

    let mut overrides = BTreeMap::new();
    for input in &signature.inputs {
        overrides.insert(input.pos.clone(), CodeExpr::Ident(input.param_name()));
    }

    let body = match compile_node_expr(
        &trick.graph,
        trick_registry,
        &sem,
        CompileMode::Preview,
        &signature.output_pos,
        &overrides,
    ) {
        Ok((expr, _)) => expr,
        Err(mut errors) => {
            diagnostics.append(&mut errors);
            return Err(diagnostics);
        }
    };

    let binding_name = unique_binding_name(trick.name.as_str(), trick.id.as_str(), used_names);

    Ok(CompiledTrick {
        trick_id: trick.id.clone(),
        display_name: trick.name.clone(),
        binding_name,
        signature,
        body,
    })
}

pub fn runtime_trick_pieces(compiled: &[CompiledTrick]) -> Vec<GeneratedTrickPiece> {
    compiled.iter().map(CompiledTrick::to_piece).collect()
}

pub fn trick_param_defaults(signature: &CadenceTrickSignature) -> Vec<Option<CodeExpr>> {
    signature
        .inputs
        .iter()
        .map(default_expr_for_input)
        .collect()
}

fn trick_slot_from_piece_id(piece_id: &str) -> Option<u8> {
    piece_id
        .rsplit('_')
        .next()
        .and_then(|value| value.parse::<u8>().ok())
}

fn unique_binding_name(name: &str, trick_id: &str, used_names: &mut BTreeSet<String>) -> String {
    let mut base = sanitize_js_identifier(name);
    if base.is_empty() {
        let suffix = trick_id
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .take(8)
            .collect::<String>();
        base = if suffix.is_empty() {
            "trick_generated".into()
        } else {
            format!("trick_{suffix}")
        };
    }

    let mut candidate = base.clone();
    let mut index = 2usize;
    while used_names.contains(&candidate) {
        candidate = format!("{base}_{index}");
        index += 1;
    }
    used_names.insert(candidate.clone());
    candidate
}

fn sanitize_js_identifier(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for (index, ch) in trimmed.chars().enumerate() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            if index == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else if (ch == ' ' || ch == '-' || ch == '.') && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}
