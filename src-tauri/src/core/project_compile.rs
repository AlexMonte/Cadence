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

    let setup = build_setup(project.init_stage.cps_expr.as_deref(), &project.init_stage.sample_loads);
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

fn build_setup(cps_expr: Option<&str>, sample_loads: &[CadenceSampleLoad]) -> Vec<CadenceSetupStmt> {
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
    matches!(diag.severity, tile_graph::diagnostics::DiagnosticSeverity::Error)
}
