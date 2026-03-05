use serde::{Deserialize, Serialize};

use crate::commands::project_commands::{SharedAppState, active_project};
use crate::core::compiler::compile_graph;
use crate::core::diagnostics::Diagnostic;
use crate::core::piece_registry::PieceRegistry;
use crate::core::semantic::semantic_pass;
use crate::core::types::PortType;
use crate::store::app_state::AppStore;

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeCommitArgs {
    pub cpm: Option<f32>,
    pub force: Option<bool>,
    pub playing: Option<bool>,
    pub code_override: Option<String>,
    pub request_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCommitDto {
    pub success: bool,
    pub rev: u64,
    pub changed: bool,
    pub playing: bool,
    pub code: Option<String>,
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

fn compile_runtime_code(
    store: &AppStore,
    code_override: Option<&str>,
) -> Result<(String, usize), Vec<Diagnostic>> {
    if let Some(override_code) = code_override {
        let trimmed = override_code.trim();
        if trimmed.is_empty() {
            return Err(Vec::new());
        }
        return Ok((trimmed.to_string(), 1));
    }

    let project = active_project(store).map_err(|_| Vec::new())?;
    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&project.graph, &registry);
    if !sem.is_valid() {
        return Err(sem.errors);
    }
    let voice_count = project
        .graph
        .nodes
        .values()
        .filter_map(|node| registry.get(node.piece_id.as_str()))
        .filter(|piece| matches!(piece.def().output_type, Some(PortType::Pattern)))
        .count();

    let expr = compile_graph(&project.graph, &registry, &sem)?;
    Ok((expr.render(), voice_count))
}

fn commit_runtime(store: &mut AppStore, args: RuntimeCommitArgs) -> RuntimeCommitDto {
    let _ = args.cpm.unwrap_or(120.0);
    let force = args.force.unwrap_or(false);

    let compile = compile_runtime_code(store, args.code_override.as_deref());
    let (success, code, voice_count, diagnostics, error) = match compile {
        Ok((code, voice_count)) => (true, Some(code), voice_count, Vec::new(), None),
        Err(diagnostics) => {
            let message = if diagnostics.is_empty() {
                "compile failed: invalid override or missing project".to_string()
            } else {
                format!("compile failed with {} diagnostics", diagnostics.len())
            };
            (false, None, 0, diagnostics, Some(message))
        }
    };

    if let Some(playing) = args.playing {
        store.runtime.set_playing(playing);
    }

    let mut changed = false;
    if success {
        if let Some(compiled_code) = code.as_ref() {
            changed = store.runtime.last_code != *compiled_code;
            if changed || force {
                store.runtime.rev = store.runtime.rev.saturating_add(1);
                store.runtime.last_code = compiled_code.clone();
            }
            store.runtime.last_error = None;
        }
    } else {
        store.runtime.last_error = error.clone();
    }

    RuntimeCommitDto {
        success,
        rev: store.runtime.rev,
        changed: changed || force,
        playing: store.runtime.playing,
        code,
        voice_count,
        play_elapsed_ms: store.runtime.elapsed_ms(),
        request_id: args.request_id,
        diagnostics,
        error,
    }
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
    use crate::core::graph::{Edge, Graph, Node, ProjectDocument};
    use crate::core::types::{EdgeId, GridPos};

    fn valid_project() -> ProjectDocument {
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
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "strudel.output".to_string(),
                inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
},
        );
        let edge = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 0, row: 0 },
            to_node: GridPos { col: 1, row: 0 },
            to_param: "pattern".to_string(),
        };
        ProjectDocument::new(
            "demo".to_string(),
            Graph {
                nodes,
                edges: BTreeMap::from([(edge.id.clone(), edge)]),
                name: "runtime".to_string(),
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
                request_id: None,
            },
        );
        assert!(!failed.success);
        assert_eq!(store.runtime.last_code, last_good);
    }
}
