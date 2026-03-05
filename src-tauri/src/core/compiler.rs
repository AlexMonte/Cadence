use std::collections::BTreeMap;

use crate::core::code_expr::CodeExpr;
use crate::core::diagnostics::{Diagnostic, DiagnosticKind, SemanticResult};
use crate::core::graph::Graph;
use crate::core::piece_registry::PieceRegistry;
use crate::core::semantic::incoming_edge_for_param;
use crate::core::types::{EdgeId, GridPos};

fn diagnostic(kind: DiagnosticKind, site: Option<GridPos>, edge_id: Option<EdgeId>) -> Diagnostic {
    Diagnostic {
        kind,
        site,
        edge_id,
    }
}

pub fn compile_graph(
    graph: &Graph,
    registry: &PieceRegistry,
    sem: &SemanticResult,
) -> Result<CodeExpr, Vec<Diagnostic>> {
    if !sem.is_valid() {
        return Err(sem.errors.clone());
    }

    let mut compiled = BTreeMap::<GridPos, CodeExpr>::new();
    let mut compile_errors = Vec::<Diagnostic>::new();

    for pos in &sem.eval_order {
        let Some(node) = graph.nodes.get(pos) else {
            compile_errors.push(diagnostic(
                DiagnosticKind::UnknownNode { pos: pos.clone() },
                Some(pos.clone()),
                None,
            ));
            continue;
        };

        let Some(piece) = registry.get(node.piece_id.as_str()) else {
            compile_errors.push(diagnostic(
                DiagnosticKind::UnknownPiece {
                    piece_id: node.piece_id.clone(),
                },
                Some(pos.clone()),
                None,
            ));
            continue;
        };

        let mut inputs = BTreeMap::<String, CodeExpr>::new();

        for param in &piece.def().params {
            let connected = incoming_edge_for_param(graph, pos, param.id.as_str())
                .and_then(|edge| compiled.get(&edge.from).cloned());

            if let Some(expr) = connected {
                inputs.insert(param.id.clone(), expr);
                continue;
            }

            if let Some(value) = node.inline_params.get(param.id.as_str()) {
                if !param.schema.can_inline() {
                    compile_errors.push(diagnostic(
                        DiagnosticKind::InlineNotAllowed {
                            param: param.id.clone(),
                        },
                        Some(pos.clone()),
                        None,
                    ));
                    continue;
                }
                let Some(expr) = param.schema.inline_expr(value) else {
                    compile_errors.push(diagnostic(
                        DiagnosticKind::InlineTypeMismatch {
                            param: param.id.clone(),
                            expected: param.schema.expected_port_type(),
                            got_value: value.clone(),
                        },
                        Some(pos.clone()),
                        None,
                    ));
                    continue;
                };
                inputs.insert(param.id.clone(), expr);
                continue;
            }

            if let Some(default_expr) = param.schema.default_expr() {
                inputs.insert(param.id.clone(), default_expr);
                continue;
            }

            if param.required {
                compile_errors.push(diagnostic(
                    DiagnosticKind::MissingRequiredParam {
                        param: param.id.clone(),
                    },
                    Some(pos.clone()),
                    None,
                ));
            }
        }

        if !compile_errors.is_empty() {
            continue;
        }

        let expr = piece.compile(&inputs, &node.inline_params);
        compiled.insert(pos.clone(), expr);
    }

    if !compile_errors.is_empty() {
        return Err(compile_errors);
    }

    let Some(terminal) = sem.terminal.clone() else {
        return Err(vec![diagnostic(DiagnosticKind::NoTerminalNode, None, None)]);
    };

    compiled.remove(&terminal).ok_or_else(|| {
        vec![diagnostic(
            DiagnosticKind::UnknownNode {
                pos: terminal.clone(),
            },
            Some(terminal),
            None,
        )]
    })
}
