use serde::{Deserialize, Serialize};

use crate::commands::project_commands::{SharedAppState, active_project, active_project_mut};
use crate::core::project_compile::compile_project;
use crate::core::terminal_strategy::{DollarRenderer, StackRenderer, TerminalRenderer};
use crate::model::CadenceSampleLoad;
use crate::store::app_state::AppStore;
use tile_graph::code_expr::CodeExpr;
use tile_graph::compiler::{CompileMode, NodeStateUpdate};
use tile_graph::diagnostics::Diagnostic;

/// Serializable strategy selector passed from the frontend per `runtime_commit`
/// call. Maps to one of the built-in [`TerminalRenderer`] implementations via
/// [`TerminalStrategy::as_renderer`].
///
/// To add a new output strategy, implement [`TerminalRenderer`] in
/// `core/terminal_strategy.rs` and add a variant here.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStrategy {
    /// Wrap all terminals in `stack(a, b, ...)`. With a single terminal,
    /// emits the expression directly.
    Stack,
    /// Emit one `$: expr` line per terminal — independent Strudel voices.
    Dollar,
}

impl Default for TerminalStrategy {
    fn default() -> Self {
        Self::Stack
    }
}

impl TerminalStrategy {
    /// Return the corresponding [`TerminalRenderer`] for this strategy.
    pub fn as_renderer(self) -> Box<dyn TerminalRenderer> {
        match self {
            TerminalStrategy::Stack => Box::new(StackRenderer),
            TerminalStrategy::Dollar => Box::new(DollarRenderer),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeCommitArgs {
    pub cpm: Option<f32>,
    pub force: Option<bool>,
    pub playing: Option<bool>,
    pub code_override: Option<String>,
    pub terminal_strategy: Option<TerminalStrategy>,
    pub request_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCommitDto {
    pub success: bool,
    pub rev: u64,
    pub changed: bool,
    pub playing: bool,
    pub code: Option<String>,
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoad>,
    pub declaration_code: Vec<String>,
    pub runtime_code: Option<String>,
    pub voice_count: usize,
    pub play_elapsed_ms: u64,
    pub request_id: Option<u64>,
    pub diagnostics: Vec<Diagnostic>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatusDto {
    pub rev: u64,
    pub playing: bool,
    pub has_program: bool,
    pub last_error: Option<String>,
    pub play_elapsed_ms: u64,
}

pub(crate) fn hard_stop_runtime_state(store: &mut AppStore, clear_program: bool) {
    store.runtime.set_playing(false);
    store.runtime.reset_playback_clock();
    store.runtime.last_error = None;
    if clear_program {
        store.runtime.last_code.clear();
    }
}

fn compile_runtime_plan(
    store: &AppStore,
    code_override: Option<&str>,
    strategy: TerminalStrategy,
) -> Result<(RuntimeCommitDto, Vec<NodeStateUpdate>), Vec<Diagnostic>> {
    if let Some(override_code) = code_override {
        let trimmed = override_code.trim();
        if trimmed.is_empty() {
            return Err(Vec::new());
        }
        return Ok((
            RuntimeCommitDto {
                success: true,
                rev: 0,
                changed: false,
                playing: false,
                code: Some(trimmed.to_string()),
                cps_expr: None,
                sample_loads: Vec::new(),
                declaration_code: Vec::new(),
                runtime_code: Some(trimmed.to_string()),
                voice_count: 1,
                play_elapsed_ms: 0,
                request_id: None,
                diagnostics: Vec::new(),
                error: None,
            },
            Vec::new(),
        ));
    }

    let project = active_project(store).map_err(|_| Vec::new())?;
    let compiled = compile_project(project, strategy, CompileMode::Runtime);
    let voice_count = compiled
        .program
        .runtime
        .as_ref()
        .map(|runtime| runtime.terminals.len())
        .unwrap_or(0);
    if !compiled.can_play {
        return Err(compiled.diagnostics);
    }
    Ok((
        RuntimeCommitDto {
            success: true,
            rev: 0,
            changed: false,
            playing: false,
            code: compiled.full_code,
            cps_expr: project.init_stage.cps_expr.clone(),
            sample_loads: project.init_stage.sample_loads.clone(),
            declaration_code: compiled.program.declaration_code(),
            runtime_code: compiled.runtime_code,
            voice_count,
            play_elapsed_ms: 0,
            request_id: None,
            diagnostics: compiled.diagnostics,
            error: None,
        },
        compiled.state_updates,
    ))
}

/// Render compiled terminal expressions into a program string using the
/// supplied [`TerminalRenderer`]. Call [`TerminalStrategy::as_renderer`] to
/// obtain a renderer from the serializable strategy enum, or provide any
/// custom implementation of [`TerminalRenderer`] directly.
pub(crate) fn render_terminals(terminals: &[CodeExpr], renderer: &dyn TerminalRenderer) -> String {
    if terminals.is_empty() {
        return String::new();
    }
    renderer.render(terminals)
}

fn commit_runtime(store: &mut AppStore, args: RuntimeCommitArgs) -> RuntimeCommitDto {
    let _ = args.cpm.unwrap_or(120.0);
    let force = args.force.unwrap_or(false);
    let strategy = args.terminal_strategy.unwrap_or_default();

    let compile = compile_runtime_plan(store, args.code_override.as_deref(), strategy);
    let (success, mut commit, state_updates, error) = match compile {
        Ok((commit, state_updates)) => (true, commit, state_updates, None),
        Err(diagnostics) => {
            let message = if diagnostics.is_empty() {
                "compile failed: invalid override or missing project".to_string()
            } else {
                format!("compile failed with {} diagnostics", diagnostics.len())
            };
            (
                false,
                RuntimeCommitDto {
                    success: false,
                    rev: 0,
                    changed: false,
                    playing: false,
                    code: None,
                    cps_expr: None,
                    sample_loads: Vec::new(),
                    declaration_code: Vec::new(),
                    runtime_code: None,
                    voice_count: 0,
                    play_elapsed_ms: 0,
                    request_id: args.request_id,
                    diagnostics,
                    error: Some(message.clone()),
                },
                Vec::new(),
                Some(message),
            )
        }
    };

    if let Some(playing) = args.playing {
        store.runtime.set_playing(playing);
    }

    let mut changed = false;
    if success {
        if let Some(compiled_code) = commit.code.as_ref() {
            changed = store.runtime.last_code != *compiled_code;
            if changed || force {
                store.runtime.rev = store.runtime.rev.saturating_add(1);
                store.runtime.last_code = compiled_code.clone();
            }
            if !state_updates.is_empty() {
                if let Ok(project) = active_project_mut(store) {
                    for update in state_updates {
                        if let Some(node) = project.graph.nodes.get_mut(&update.position) {
                            node.node_state = Some(update.state);
                        }
                    }
                }
            }
            store.runtime.last_error = None;
        }
    } else {
        store.runtime.last_error = error.clone();
    }

    commit.success = success;
    commit.rev = store.runtime.rev;
    commit.changed = changed || force;
    commit.playing = store.runtime.playing;
    commit.play_elapsed_ms = store.runtime.elapsed_ms();
    commit.request_id = args.request_id;
    commit.error = error;
    commit
}

#[tauri::command]
pub fn runtime_commit(
    state: tauri::State<'_, SharedAppState>,
    args: RuntimeCommitArgs,
) -> Result<RuntimeCommitDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;

    let result = commit_runtime(&mut store, args);
    if result.success {
        store.push_diagnostic(
            "runtime_commit",
            format!(
                "rev={} changed={} playing={} elapsed={}ms",
                result.rev, result.changed, result.playing, result.play_elapsed_ms
            ),
        );
    } else {
        store.push_diagnostic(
            "runtime_commit_error",
            result
                .error
                .clone()
                .unwrap_or_else(|| "compile failed".to_string()),
        );
    }

    Ok(result)
}

#[tauri::command]
pub fn runtime_stop(state: tauri::State<'_, SharedAppState>) -> Result<RuntimeStatusDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    hard_stop_runtime_state(&mut store, false);
    store.push_diagnostic("runtime_stop", "playback stopped".to_string());

    Ok(RuntimeStatusDto {
        rev: store.runtime.rev,
        playing: store.runtime.playing,
        has_program: !store.runtime.last_code.is_empty(),
        last_error: store.runtime.last_error.clone(),
        play_elapsed_ms: store.runtime.elapsed_ms(),
    })
}

#[tauri::command]
pub fn runtime_reset_on_project_swap(
    state: tauri::State<'_, SharedAppState>,
) -> Result<RuntimeStatusDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    hard_stop_runtime_state(&mut store, true);
    store.push_diagnostic("runtime_reset", "project swap reset".to_string());

    Ok(RuntimeStatusDto {
        rev: store.runtime.rev,
        playing: store.runtime.playing,
        has_program: !store.runtime.last_code.is_empty(),
        last_error: store.runtime.last_error.clone(),
        play_elapsed_ms: store.runtime.elapsed_ms(),
    })
}

#[tauri::command]
pub fn runtime_status(state: tauri::State<'_, SharedAppState>) -> Result<RuntimeStatusDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;

    Ok(RuntimeStatusDto {
        rev: store.runtime.rev,
        playing: store.runtime.playing,
        has_program: !store.runtime.last_code.is_empty(),
        last_error: store.runtime.last_error.clone(),
        play_elapsed_ms: store.runtime.elapsed_ms(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::*;
    use crate::model::CadenceProjectDocument;
    use tile_graph::code_expr::CodeExpr;
    use tile_graph::graph::{Edge, Graph, Node};
    use tile_graph::types::{EdgeId, GridPos};

    fn valid_project() -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "strudel.sound".to_string(),
                inline_params: BTreeMap::from([(
                    "value".to_string(),
                    Value::String("bd".to_string()),
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
            from: GridPos { col: 0, row: 0 },
            to_node: GridPos { col: 1, row: 0 },
            to_param: "pattern".to_string(),
        };
        CadenceProjectDocument::new(
            "demo".to_string(),
            Graph {
                nodes,
                edges: BTreeMap::from([(edge.id.clone(), edge)]),
                name: "runtime".to_string(),
                cols: 9,
                rows: 9,
            },
        )
    }

    #[test]
    fn runtime_keeps_last_known_good_code_on_compile_failure() {
        let mut store = AppStore {
            current_project: Some(valid_project()),
            ..Default::default()
        };

        let first = commit_runtime(
            &mut store,
            RuntimeCommitArgs {
                cpm: Some(120.0),
                force: Some(false),
                playing: Some(true),
                code_override: None,
                terminal_strategy: None,
                request_id: None,
            },
        );
        assert!(first.success);
        let last_good = store.runtime.last_code.clone();
        assert!(!last_good.is_empty());

        if let Some(project) = store.current_project.as_mut() {
            project.graph.nodes.remove(&GridPos { col: 1, row: 0 });
            project.graph.edges.clear();
        }

        let failed = commit_runtime(
            &mut store,
            RuntimeCommitArgs {
                cpm: Some(120.0),
                force: Some(false),
                playing: Some(true),
                code_override: None,
                terminal_strategy: None,
                request_id: None,
            },
        );
        assert!(!failed.success);
        assert_eq!(store.runtime.last_code, last_good);
    }

    #[test]
    fn render_terminals_supports_stack_and_dollar_strategies() {
        use crate::core::terminal_strategy::{DollarRenderer, SingleOutputRenderer, StackRenderer};

        let terminals = vec![
            CodeExpr::Call {
                func: "note".to_string(),
                args: vec![CodeExpr::Literal(Value::String("c3".to_string()))],
            },
            CodeExpr::Call {
                func: "s".to_string(),
                args: vec![CodeExpr::Literal(Value::String("bd".to_string()))],
            },
        ];

        let stacked = render_terminals(terminals.as_slice(), &StackRenderer);
        assert_eq!(stacked, "stack(note(\"c3\"), s(\"bd\"))");

        let dollar = render_terminals(terminals.as_slice(), &DollarRenderer);
        assert_eq!(dollar, "$: note(\"c3\")\n$: s(\"bd\")");

        let single = render_terminals(terminals.as_slice(), &SingleOutputRenderer);
        assert_eq!(single, "note(\"c3\")");

        // Single terminal — Stack should not wrap in stack().
        let one = vec![CodeExpr::Call {
            func: "note".to_string(),
            args: vec![CodeExpr::Literal(Value::String("e3".to_string()))],
        }];
        let stacked_one = render_terminals(one.as_slice(), &StackRenderer);
        assert_eq!(stacked_one, "note(\"e3\")");
    }
}
