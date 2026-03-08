use serde_json::Value;

use crate::commands::TerminalStrategy;
use crate::core::cadence_program::{
    CadenceDecl, CadenceProgram, CadenceRuntimeBlock, CadenceSetupStmt,
    declaration_from_compiled_trick,
};
use crate::core::piece_registry::{runtime_registry_from_compiled, trick_editor_registry};
use crate::core::tricks::compile_tricks;
use crate::model::{CadenceProjectDocument, CadenceSampleLoad};
use tile_graph::code_expr::CodeExpr;
use tile_graph::compiler::{CompileMode, NodeStateUpdate, compile_graph};
use tile_graph::diagnostics::Diagnostic;
use tile_graph::semantic::semantic_pass;

#[derive(Debug, Clone)]
pub struct CompiledProject {
    pub program: CadenceProgram,
    pub diagnostics: Vec<Diagnostic>,
    pub full_code: Option<String>,
    pub runtime_code: Option<String>,
    pub state_updates: Vec<NodeStateUpdate>,
    pub can_render: bool,
    pub can_play: bool,
}

pub fn compile_project(
    project: &CadenceProjectDocument,
    strategy: TerminalStrategy,
    mode: CompileMode,
) -> CompiledProject {
    let mut diagnostics = Vec::<Diagnostic>::new();
    let trick_registry = trick_editor_registry();
    let (compiled_tricks, mut trick_diagnostics) = compile_tricks(project, &trick_registry);
    diagnostics.append(&mut trick_diagnostics);

    let runtime_registry = runtime_registry_from_compiled(&compiled_tricks);
    let runtime_sem = semantic_pass(&project.graph, &runtime_registry);
    let mut runtime_errors = runtime_sem.diagnostics.clone();
    diagnostics.append(&mut runtime_errors);

    let (runtime, state_updates) = if runtime_sem.is_valid() {
        match compile_graph(&project.graph, &runtime_registry, &runtime_sem, mode) {
            Ok(program) => (
                Some(CadenceRuntimeBlock {
                    terminals: program.terminals,
                    strategy,
                }),
                program.state_updates,
            ),
            Err(mut errors) => {
                diagnostics.append(&mut errors);
                (None, Vec::new())
            }
        }
    } else {
        (None, Vec::new())
    };

    let setup = build_setup(
        project.init_stage.cps_expr.as_deref(),
        &project.init_stage.sample_loads,
    );
    let declarations = compiled_tricks
        .iter()
        .map(declaration_from_compiled_trick)
        .collect::<Vec<CadenceDecl>>();
    let program = CadenceProgram {
        setup,
        declarations,
        runtime,
    };
    let full_code = program.render();
    let runtime_code = program.runtime.as_ref().map(CadenceRuntimeBlock::render);
    let can_render = full_code.is_some();
    let can_play = runtime_code
        .as_ref()
        .map(|code| !code.trim().is_empty())
        .unwrap_or(false)
        && !diagnostics.iter().any(is_error_diagnostic);

    CompiledProject {
        program,
        diagnostics,
        full_code,
        runtime_code,
        state_updates,
        can_render,
        can_play,
    }
}

fn build_setup(
    cps_expr: Option<&str>,
    sample_loads: &[CadenceSampleLoad],
) -> Vec<CadenceSetupStmt> {
    let mut setup = Vec::<CadenceSetupStmt>::new();

    if let Some(expr) = cps_expr.map(str::trim).filter(|expr| !expr.is_empty()) {
        setup.push(CadenceSetupStmt::Expr {
            expr: CodeExpr::Call {
                func: "setCps".into(),
                args: vec![CodeExpr::Raw(expr.to_string())],
            },
            await_: false,
        });
    }

    for sample in sample_loads {
        setup.push(CadenceSetupStmt::Expr {
            expr: CodeExpr::Call {
                func: "samples".into(),
                args: vec![
                    CodeExpr::Literal(Value::Object(
                        sample
                            .aliases
                            .iter()
                            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                            .collect::<serde_json::Map<String, Value>>(),
                    )),
                    CodeExpr::Literal(Value::String(sample.source.clone())),
                ],
            },
            await_: true,
        });
    }

    setup
}

fn is_error_diagnostic(diag: &Diagnostic) -> bool {
    matches!(
        diag.severity,
        tile_graph::diagnostics::DiagnosticSeverity::Error
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Number, Value};

    use super::compile_project;
    use crate::commands::TerminalStrategy;
    use crate::model::{CadenceInitStage, CadenceProjectDocument, CadenceTrickDef};
    use tile_graph::compiler::CompileMode;
    use tile_graph::graph::{Edge, Graph, Node};
    use tile_graph::types::{EdgeId, GridPos, TileSide};

    fn node(piece_id: &str) -> Node {
        Node {
            piece_id: piece_id.into(),
            inline_params: BTreeMap::new(),
            input_sides: BTreeMap::new(),
            output_side: None,
            label: None,
            node_state: None,
        }
    }

    fn trick_graph() -> Graph {
        let arg = GridPos { col: 0, row: 0 };
        let bank = GridPos { col: 1, row: 0 };
        let clip = GridPos { col: 2, row: 0 };
        let gain = GridPos { col: 3, row: 0 };
        let out = GridPos { col: 4, row: 0 };

        let mut nodes = BTreeMap::new();
        nodes.insert(
            arg.clone(),
            Node {
                piece_id: "cadence.trick_input_1".into(),
                inline_params: BTreeMap::from([
                    ("label".into(), Value::String("x".into())),
                    ("port_type".into(), Value::String("pattern".into())),
                    ("required".into(), Value::Bool(true)),
                    ("is_receiver".into(), Value::Bool(true)),
                ]),
                input_sides: BTreeMap::new(),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            bank.clone(),
            Node {
                inline_params: BTreeMap::from([(
                    "value".into(),
                    Value::String("AlesisHR16".into()),
                )]),
                ..node("strudel.bank")
            },
        );
        nodes.insert(
            clip.clone(),
            Node {
                inline_params: BTreeMap::from([("value".into(), Value::Number(Number::from(1)))]),
                ..node("strudel.clip")
            },
        );
        nodes.insert(
            gain.clone(),
            Node {
                inline_params: BTreeMap::from([(
                    "amount".into(),
                    Value::Number(Number::from_f64(0.08).expect("number")),
                )]),
                ..node("strudel.gain")
            },
        );
        nodes.insert(
            out.clone(),
            Node {
                ..node("cadence.trick_output")
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: arg,
                to_node: bank.clone(),
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: bank,
                to_node: clip.clone(),
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: clip,
                to_node: gain.clone(),
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: gain,
                to_node: out,
                to_param: "pattern".into(),
            },
        ];

        Graph {
            nodes,
            edges: edges
                .into_iter()
                .map(|edge| (edge.id.clone(), edge))
                .collect(),
            name: "ritmo".into(),
            cols: 5,
            rows: 2,
        }
    }

    fn runtime_graph() -> Graph {
        let sound = GridPos { col: 0, row: 0 };
        let apply = GridPos { col: 1, row: 0 };
        let out = GridPos { col: 2, row: 0 };
        let trick = GridPos { col: 1, row: 1 };

        let mut nodes = BTreeMap::new();
        nodes.insert(
            sound.clone(),
            Node {
                inline_params: BTreeMap::from([(
                    "value".into(),
                    Value::String("bd!4,[~ sd]!2,[~ hh!2 hh*2]!2".into()),
                )]),
                ..node("strudel.sound")
            },
        );
        nodes.insert(apply.clone(), node("strudel.apply"));
        nodes.insert(out.clone(), node("strudel.output"));
        nodes.insert(
            trick.clone(),
            Node {
                output_side: Some(TileSide::North),
                ..node("cadence.trick.ritmo")
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: sound,
                to_node: apply.clone(),
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: trick,
                to_node: apply.clone(),
                to_param: "fn_name".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: apply,
                to_node: out,
                to_param: "pattern".into(),
            },
        ];

        Graph {
            nodes,
            edges: edges
                .into_iter()
                .map(|edge| (edge.id.clone(), edge))
                .collect(),
            name: "runtime".into(),
            cols: 4,
            rows: 2,
        }
    }

    #[test]
    fn compile_project_renders_trick_declaration_and_apply_reference() {
        let project = CadenceProjectDocument {
            schema_version: CadenceProjectDocument::SCHEMA_VERSION,
            name: "Untitled".into(),
            graph: runtime_graph(),
            init_stage: CadenceInitStage {
                cps_expr: None,
                sample_loads: Vec::new(),
                tricks: vec![CadenceTrickDef {
                    id: "ritmo".into(),
                    name: "ritmo".into(),
                    graph: trick_graph(),
                }],
            },
        };

        let compiled = compile_project(&project, TerminalStrategy::Stack, CompileMode::Preview);
        assert!(
            compiled.diagnostics.is_empty(),
            "expected no diagnostics, got {:?}",
            compiled.diagnostics
        );
        let code = compiled.full_code.expect("rendered code");
        assert!(code.contains("const ritmo = (x) => x.bank(\"AlesisHR16\").clip(1).gain(0.08)"));
        assert!(code.contains("s(\"bd!4,[~ sd]!2,[~ hh!2 hh*2]!2\").apply(ritmo)"));
    }
}
