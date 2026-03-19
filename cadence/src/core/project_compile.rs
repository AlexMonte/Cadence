//! Cadence-level compile pipeline that turns a project into runnable Strudel code.

use crate::commands::TerminalStrategy;
use crate::core::cadence_program::{
    CadenceDecl, CadenceProgram, CadenceRuntimeBlock, CadenceSetupStmt,
    declaration_from_compiled_subgraph,
};
use crate::core::compile_support::{has_error_diagnostics, merge_diagnostics};
use crate::core::host_adapter::{
    normalize_project_piece_sides, rendered_output, runtime_engine_from_compiled,
    subgraph_editor_engine,
};
use crate::model::{CadenceProjectDocument, CadenceSampleLoad, CompileMetaDto};
use tessera::compiler::{CompileMode, NodeStateUpdate};
use tessera::diagnostics::{Diagnostic, DiagnosticKind};
use tessera::subgraph::compile_subgraphs;
use tessera::{Expr, parse_ident_path};

#[derive(Debug, Clone)]
/// Result of compiling the full project, including init-stage setup and tricks.
pub struct CompiledProject {
    /// Structured representation of the rendered Cadence program.
    pub program: CadenceProgram,
    /// Collected diagnostics from trick compilation and runtime graph analysis.
    pub diagnostics: Vec<Diagnostic>,
    /// Full render containing setup, declarations, and runtime output.
    pub full_code: Option<String>,
    /// Runtime-only render used directly for playback.
    pub runtime_code: Option<String>,
    /// Stateful node updates produced while compiling in runtime mode.
    pub state_updates: Vec<NodeStateUpdate>,
    /// Compile-time metadata emitted by Tessera for the runtime graph.
    pub compile_meta: CompileMetaDto,
    /// Whether enough structure exists to render a full program.
    pub can_render: bool,
    /// Whether the project is safe to hand to the runtime for playback.
    pub can_play: bool,
}

/// Compile the entire Cadence project into Strudel-ready output.
///
/// This pipeline compiles tricks first, injects them back into the runtime registry,
/// compiles the main graph, and finally renders setup/declarations/runtime sections.
pub fn compile_project(
    project: &CadenceProjectDocument,
    strategy: TerminalStrategy,
    mode: CompileMode,
) -> CompiledProject {
    let mut project = project.clone();
    normalize_project_piece_sides(&mut project);

    let editor_engine = subgraph_editor_engine(TerminalStrategy::Stack);
    let (compiled_subgraphs, subgraph_diagnostics) = compile_subgraphs(
        project.init_stage.tricks.as_slice(),
        editor_engine.registry(),
    );
    let runtime_engine = runtime_engine_from_compiled(compiled_subgraphs.as_slice(), strategy);
    let runtime_sem = runtime_engine.analyze(&project.graph);
    let (runtime, runtime_code, state_updates, runtime_compile_diagnostics, compile_meta) =
        if matches!(mode, CompileMode::Preview) || runtime_sem.is_valid() {
            match runtime_engine.compile(&project.graph, mode) {
                Ok(program) => {
                    let rendered_sections =
                        runtime_engine.render_terminals(program.terminals.as_slice());
                    let runtime_code = rendered_output(rendered_sections.clone());
                    let compile_meta = CompileMetaDto::from(&program);
                    (
                        Some(CadenceRuntimeBlock {
                            rendered_sections,
                            voice_count: program.terminals.len(),
                        }),
                        runtime_code,
                        program.state_updates,
                        program.diagnostics.clone(),
                        compile_meta,
                    )
                }
                Err(errors) => (None, None, Vec::new(), errors, CompileMetaDto::default()),
            }
        } else {
            (
                None,
                None,
                Vec::new(),
                Vec::new(),
                CompileMetaDto::default(),
            )
        };

    let (setup, mut setup_diagnostics) = build_setup(
        project.init_stage.cps_expr.as_deref(),
        &project.init_stage.sample_loads,
    );
    let diagnostics = merge_diagnostics([
        subgraph_diagnostics,
        runtime_sem.diagnostics.clone(),
        runtime_compile_diagnostics,
        std::mem::take(&mut setup_diagnostics),
    ]);
    let declarations = compiled_subgraphs
        .iter()
        .map(declaration_from_compiled_subgraph)
        .collect::<Vec<CadenceDecl>>();
    let program = CadenceProgram {
        setup,
        declarations,
        runtime,
    };
    let full_code = program.render();
    let can_render = full_code.is_some();
    let can_play = runtime_code
        .as_ref()
        .map(|code| !code.trim().is_empty())
        .unwrap_or(false)
        && !has_error_diagnostics(&diagnostics);

    CompiledProject {
        program,
        diagnostics,
        full_code,
        runtime_code,
        state_updates,
        compile_meta,
        can_render,
        can_play,
    }
}

fn build_setup(
    cps_expr: Option<&str>,
    sample_loads: &[CadenceSampleLoad],
) -> (Vec<CadenceSetupStmt>, Vec<Diagnostic>) {
    let mut setup = Vec::<CadenceSetupStmt>::new();
    let mut diagnostics = Vec::<Diagnostic>::new();

    if let Some(expr) = cps_expr.map(str::trim).filter(|expr| !expr.is_empty()) {
        match cps_expr_as_ast(expr) {
            Some(value) => setup.push(CadenceSetupStmt::Expr {
                expr: Expr::call_named("setCps", vec![value]),
                await_: false,
            }),
            None => diagnostics.push(Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: format!(
                        "init-stage cps must be a finite numeric literal or identifier path, got `{expr}`"
                    ),
                },
                None,
            )),
        }
    }

    for sample in sample_loads {
        setup.push(CadenceSetupStmt::Expr {
            expr: Expr::call_named(
                "samples",
                vec![
                    sample_alias_expr(sample),
                    Expr::str_lit(sample.source.clone()),
                ],
            ),
            await_: true,
        });
    }

    (setup, diagnostics)
}

fn cps_expr_as_ast(expr: &str) -> Option<Expr> {
    if let Ok(number) = expr.parse::<f64>()
        && number.is_finite()
    {
        return Some(Expr::float(number));
    }
    parse_ident_path(expr)
}

fn sample_alias_expr(sample: &CadenceSampleLoad) -> Expr {
    Expr::record(
        sample
            .aliases
            .iter()
            .map(|(key, value)| (key.clone(), Expr::str_lit(value.clone())))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Number, Value};

    use super::compile_project;
    use crate::commands::TerminalStrategy;
    use crate::model::{
        CadenceInitStage, CadenceProjectDocument, CadenceSampleLoad, CadenceTrickDef,
    };
    use tessera::compiler::CompileMode;
    use tessera::graph::{Edge, Graph, Node};
    use tessera::types::{EdgeId, GridPos, TileSide};

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
            arg,
            Node {
                piece_id: "tessera.subgraph_input_1".into(),
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
            bank,
            Node {
                inline_params: BTreeMap::from([(
                    "value".into(),
                    Value::String("AlesisHR16".into()),
                )]),
                ..node("strudel.bank")
            },
        );
        nodes.insert(
            clip,
            Node {
                inline_params: BTreeMap::from([("value".into(), Value::Number(Number::from(1)))]),
                ..node("strudel.clip")
            },
        );
        nodes.insert(
            gain,
            Node {
                inline_params: BTreeMap::from([(
                    "amount".into(),
                    Value::Number(Number::from_f64(0.08).expect("number")),
                )]),
                ..node("strudel.gain")
            },
        );
        nodes.insert(
            out,
            Node {
                ..node("tessera.subgraph_output")
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: arg,
                to_node: bank,
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: bank,
                to_node: clip,
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: clip,
                to_node: gain,
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: gain,
                to_node: out,
                to_param: "input".into(),
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
            sound,
            Node {
                inline_params: BTreeMap::from([(
                    "value".into(),
                    Value::String("bd!4,[~ sd]!2,[~ hh!2 hh*2]!2".into()),
                )]),
                ..node("strudel.sound")
            },
        );
        nodes.insert(apply, node("strudel.apply"));
        nodes.insert(out, node("strudel.output"));
        nodes.insert(
            trick,
            Node {
                output_side: Some(TileSide::TOP),
                ..node("tessera.subgraph.ritmo")
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: sound,
                to_node: apply,
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: trick,
                to_node: apply,
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

    fn simple_runtime_graph() -> Graph {
        let sound = GridPos { col: 0, row: 0 };
        let out = GridPos { col: 1, row: 0 };

        let mut nodes = BTreeMap::new();
        nodes.insert(
            sound,
            Node {
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                ..node("strudel.sound")
            },
        );
        nodes.insert(out, node("strudel.output"));

        let edge = Edge {
            id: EdgeId::new(),
            from: sound,
            to_node: out,
            to_param: "pattern".into(),
        };

        Graph {
            nodes,
            edges: BTreeMap::from([(edge.id.clone(), edge)]),
            name: "runtime".into(),
            cols: 3,
            rows: 1,
        }
    }

    fn semantically_invalid_runtime_graph() -> Graph {
        let mut graph = simple_runtime_graph();
        graph.nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                ..node("strudel.fast")
            },
        );
        graph.cols = 4;
        graph
    }

    fn delay_runtime_graph() -> Graph {
        let sound = GridPos { col: 0, row: 0 };
        let delay = GridPos { col: 1, row: 0 };
        let out = GridPos { col: 2, row: 0 };

        let mut nodes = BTreeMap::new();
        nodes.insert(
            sound,
            Node {
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                ..node("strudel.sound")
            },
        );
        nodes.insert(delay, node("core.delay"));
        nodes.insert(out, node("strudel.output"));

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: sound,
                to_node: delay,
                to_param: "default".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: delay,
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
            name: "delay-runtime".into(),
            cols: 4,
            rows: 1,
        }
    }

    /// A zero-input trick: just [note] → [subgraph_output], no input nodes.
    /// Should compile to `const scala = note("c3 e3 g3")` (value, not arrow fn).
    fn zero_input_trick_graph() -> Graph {
        let note_pos = GridPos { col: 0, row: 0 };
        let out = GridPos { col: 1, row: 0 };

        let mut nodes = BTreeMap::new();
        nodes.insert(
            note_pos,
            Node {
                inline_params: BTreeMap::from([("value".into(), Value::String("c3 e3 g3".into()))]),
                ..node("strudel.note")
            },
        );
        nodes.insert(out, node("tessera.subgraph_output"));

        let edges = vec![Edge {
            id: EdgeId::new(),
            from: note_pos,
            to_node: out,
            to_param: "input".into(),
        }];

        Graph {
            nodes,
            edges: edges
                .into_iter()
                .map(|edge| (edge.id.clone(), edge))
                .collect(),
            name: "scala".into(),
            cols: 3,
            rows: 1,
        }
    }

    /// Runtime graph that references the zero-input trick as a pattern source.
    /// [scala_trick] → [scale] → [output]
    fn runtime_graph_with_value_trick() -> Graph {
        let trick = GridPos { col: 0, row: 0 };
        let scale = GridPos { col: 1, row: 0 };
        let out = GridPos { col: 2, row: 0 };

        let mut nodes = BTreeMap::new();
        nodes.insert(trick, node("tessera.subgraph.scala"));
        nodes.insert(
            scale,
            Node {
                inline_params: BTreeMap::from([("value".into(), Value::String("C:minor".into()))]),
                ..node("strudel.scale")
            },
        );
        nodes.insert(out, node("strudel.output"));

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: trick,
                to_node: scale,
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: scale,
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
            rows: 1,
        }
    }

    #[test]
    fn zero_input_trick_renders_as_value_declaration() {
        let project = CadenceProjectDocument {
            schema_version: CadenceProjectDocument::SCHEMA_VERSION,
            name: "Untitled".into(),
            graph: runtime_graph_with_value_trick(),
            init_stage: CadenceInitStage {
                cps_expr: None,
                sample_loads: Vec::new(),
                tricks: vec![CadenceTrickDef {
                    id: "scala".into(),
                    name: "scala".into(),
                    graph: zero_input_trick_graph(),
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
        // Should be a value declaration, NOT an arrow function
        assert!(
            code.contains("const scala = note(\"c3 e3 g3\")"),
            "expected value declaration, got:\n{code}"
        );
        // Should NOT contain arrow-function form
        assert!(
            !code.contains("() =>"),
            "should not contain arrow function for zero-input trick, got:\n{code}"
        );
        // The runtime should reference scala as a value
        assert!(
            code.contains("scala.scale('C:minor')"),
            "expected scala used as value in runtime, got:\n{code}"
        );
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
        assert!(code.contains("const ritmo = (x) => x.bank('AlesisHR16').clip(1).gain(0.08)"));
        assert!(code.contains("s(\"bd!4,[~ sd]!2,[~ hh!2 hh*2]!2\").apply(ritmo)"));
    }

    #[test]
    fn compile_project_renders_sample_aliases_as_single_quoted_map() {
        let project = CadenceProjectDocument {
            schema_version: CadenceProjectDocument::SCHEMA_VERSION,
            name: "Untitled".into(),
            graph: simple_runtime_graph(),
            init_stage: CadenceInitStage {
                cps_expr: None,
                sample_loads: vec![CadenceSampleLoad {
                    id: "clean_breaks".into(),
                    source: "github:yaxu/clean-breaks".into(),
                    aliases: BTreeMap::from([
                        ("bd".into(), "bd*2".into()),
                        ("sd".into(), "sd".into()),
                    ]),
                }],
                tricks: Vec::new(),
            },
        };

        let compiled = compile_project(&project, TerminalStrategy::Stack, CompileMode::Preview);
        assert!(
            compiled.diagnostics.is_empty(),
            "expected no diagnostics, got {:?}",
            compiled.diagnostics
        );
        let code = compiled.full_code.expect("rendered code");
        assert!(
            code.contains("await samples({bd: 'bd*2', sd: 'sd'}, 'github:yaxu/clean-breaks')"),
            "expected structural alias record, got:\n{code}"
        );
        assert!(
            !code.contains(
                "await samples({\"bd\":\"bd*2\",\"sd\":\"sd\"}, \"github:yaxu/clean-breaks\")"
            ),
            "did not expect JSON-quoted alias map, got:\n{code}"
        );
    }

    #[test]
    fn compile_project_preview_keeps_partial_runtime_code_when_graph_has_errors() {
        let project = CadenceProjectDocument {
            schema_version: CadenceProjectDocument::SCHEMA_VERSION,
            name: "Broken Preview".into(),
            graph: semantically_invalid_runtime_graph(),
            init_stage: CadenceInitStage::default(),
        };

        let compiled = compile_project(&project, TerminalStrategy::Stack, CompileMode::Preview);
        assert!(compiled.full_code.is_some());
        assert!(compiled.runtime_code.is_some());
        assert!(compiled.can_render);
        assert!(!compiled.can_play);
        assert!(!compiled.diagnostics.is_empty());
    }

    #[test]
    fn compile_project_runtime_stays_strict_when_graph_has_errors() {
        let project = CadenceProjectDocument {
            schema_version: CadenceProjectDocument::SCHEMA_VERSION,
            name: "Broken Runtime".into(),
            graph: semantically_invalid_runtime_graph(),
            init_stage: CadenceInitStage::default(),
        };

        let compiled = compile_project(&project, TerminalStrategy::Stack, CompileMode::Runtime);
        assert!(compiled.runtime_code.is_none());
        assert!(!compiled.can_render);
        assert!(!compiled.can_play);
        assert!(!compiled.diagnostics.is_empty());
    }

    #[test]
    fn compile_project_exposes_delay_slot_metadata() {
        let project = CadenceProjectDocument {
            schema_version: CadenceProjectDocument::SCHEMA_VERSION,
            name: "Delay".into(),
            graph: delay_runtime_graph(),
            init_stage: CadenceInitStage::default(),
        };

        let compiled = compile_project(&project, TerminalStrategy::Stack, CompileMode::Preview);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        assert!(
            compiled
                .compile_meta
                .delay_slots
                .iter()
                .any(|slot| slot.node == GridPos { col: 1, row: 0 })
        );
        assert!(compiled.compile_meta.domain_bridges.is_empty());
    }
}
