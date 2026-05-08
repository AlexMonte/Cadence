//! Runtime orchestration around score-first playback.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use crate::adapter::samples::bundled_sample_root;
use crate::{
    adapter::cadence_core::{PlaybackState, PlaybackTime},
    adapter::tessera::cadence_diagnostics,
    application::{
        authoring::compile::{CompiledProject, compile_project},
        project::ops::active_project,
        state::{AppStore, RuntimeProgramState},
    },
    domain::{
        preview::{Diagnostic, DiagnosticSeverity, RationalTime},
        project::CadenceSampleLoad,
    },
    infrastructure::dto::SharedAppState,
};

#[derive(Debug, Clone)]
struct RuntimeCommitArgs {
    cpm: Option<f32>,
    force: Option<bool>,
    playing: Option<bool>,
    request_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCommitDto {
    pub success: bool,
    pub rev: u64,
    pub changed: bool,
    pub status: RuntimeStatusDto,
    #[serde(default)]
    pub sample_loads: Vec<CadenceSampleLoad>,
    pub output_count: usize,
    pub request_id: Option<u64>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProgramStateDto {
    None,
    Current,
    StaleLastGood,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStatusDto {
    pub rev: u64,
    pub playing: bool,
    pub program_state: RuntimeProgramStateDto,
    pub has_program: bool,
    pub last_error: Option<String>,
    pub play_elapsed_ms: u64,
    pub cycle_position: RationalTime,
    pub cps: RationalTime,
}

pub(crate) fn hard_stop_runtime_state(store: &mut AppStore, clear_program: bool) {
    let _ = store.runtime_host.stop();
    store.runtime.set_playing(false);
    store.runtime.reset_playback_clock();
    store.runtime.last_error = None;
    if clear_program {
        store.runtime.clear_program();
    }
}

fn sync_runtime_playing_from_host(store: &mut AppStore) {
    let transport = store.runtime_host.status();
    store
        .runtime
        .set_playing(matches!(transport.state, PlaybackState::Playing));
}

fn compile_runtime_project(store: &AppStore) -> Result<CompiledProject, Vec<Diagnostic>> {
    let project = active_project(store).map_err(|_| Vec::new())?;
    let compiled = compile_project(project);
    if !compiled.can_play {
        return Err(cadence_diagnostics(compiled.diagnostics.clone()));
    }
    Ok(compiled)
}

fn runtime_fingerprint(compiled: &CompiledProject) -> String {
    serde_json::to_string(&compiled.program).unwrap_or_else(|_| compiled.output_count.to_string())
}

fn current_program_state(
    runtime: &crate::application::state::RuntimeState,
) -> RuntimeProgramStateDto {
    match runtime.program_state {
        RuntimeProgramState::None => RuntimeProgramStateDto::None,
        RuntimeProgramState::Current => RuntimeProgramStateDto::Current,
        RuntimeProgramState::StaleLastGood => RuntimeProgramStateDto::StaleLastGood,
    }
}

pub fn runtime_commit(
    state: &SharedAppState,
    cpm: Option<f32>,
    force: Option<bool>,
    playing: Option<bool>,
    request_id: Option<u64>,
) -> Result<RuntimeCommitDto, String> {
    let args = RuntimeCommitArgs {
        cpm,
        force,
        playing,
        request_id,
    };
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;

    let project = active_project(&store).map_err(|err| err.to_string())?;
    let sample_loads = project.init_stage.sample_loads.clone();
    let compiled = match compile_runtime_project(&store) {
        Ok(compiled) => compiled,
        Err(diagnostics) => {
            let error_count = diagnostics
                .iter()
                .filter(|diagnostic| matches!(diagnostic.severity, DiagnosticSeverity::Error))
                .count();
            let error = if error_count > 0 {
                format!("compile failed with {error_count} error diagnostics")
            } else if diagnostics.is_empty() {
                "compile failed: missing or unplayable project".to_string()
            } else {
                "compile failed: no playable output".to_string()
            };
            store.runtime.mark_program_stale();
            store.runtime.last_error = Some(error.clone());
            let status = runtime_status_from_store(&store);
            store.push_diagnostic("runtime_commit_error", error.clone());
            return Ok(RuntimeCommitDto {
                success: false,
                rev: store.runtime.rev,
                changed: false,
                status,
                sample_loads,
                output_count: 0,
                request_id: args.request_id,
                diagnostics,
                error: Some(error),
            });
        }
    };

    let fingerprint = runtime_fingerprint(&compiled);
    let force = args.force.unwrap_or(false);
    let changed = store.runtime.last_program_fingerprint != fingerprint;

    if let Some(requested_playing) = args.playing {
        if requested_playing {
            let sample_root = configured_sample_root(&store);
            store.runtime_host.set_sample_root(sample_root);
            if let Err(error) = store.runtime_host.preload_sample_loads(
                sample_loads.as_slice(),
                compiled.sample_selectors.as_slice(),
            ) {
                sync_runtime_playing_from_host(&mut store);
                store.runtime.mark_program_stale();
                store.runtime.last_error = Some(error.clone());
                let status = runtime_status_from_store(&store);
                store.push_diagnostic("runtime_commit_error", error.clone());
                return Ok(RuntimeCommitDto {
                    success: false,
                    rev: store.runtime.rev,
                    changed: changed || force,
                    status,
                    sample_loads,
                    output_count: compiled.output_count,
                    request_id: args.request_id,
                    diagnostics: cadence_diagnostics(compiled.diagnostics.clone()),
                    error: Some(error),
                });
            }
            match store.runtime_host.play(&compiled, args.cpm) {
                Ok(()) => {
                    if changed || force {
                        store.runtime.rev = store.runtime.rev.saturating_add(1);
                    }
                    store.runtime.set_current_program(fingerprint.clone());
                    sync_runtime_playing_from_host(&mut store);
                    store.runtime.last_error = None;
                }
                Err(error) => {
                    sync_runtime_playing_from_host(&mut store);
                    store.runtime.mark_program_stale();
                    store.runtime.last_error = Some(error.clone());
                    let status = runtime_status_from_store(&store);
                    store.push_diagnostic("runtime_commit_error", error.clone());
                    return Ok(RuntimeCommitDto {
                        success: false,
                        rev: store.runtime.rev,
                        changed: changed || force,
                        status,
                        sample_loads,
                        output_count: compiled.output_count,
                        request_id: args.request_id,
                        diagnostics: cadence_diagnostics(compiled.diagnostics.clone()),
                        error: Some(error),
                    });
                }
            }
        } else {
            hard_stop_runtime_state(&mut store, false);
        }
    }

    let status = runtime_status_from_store(&store);
    let rev = store.runtime.rev;
    let changed = changed || force;
    store.push_diagnostic(
        "runtime_commit",
        format!(
            "rev={} changed={} playing={} elapsed={}ms",
            rev, changed, status.playing, status.play_elapsed_ms
        ),
    );

    Ok(RuntimeCommitDto {
        success: true,
        rev,
        changed,
        status,
        sample_loads,
        output_count: compiled.output_count,
        request_id: args.request_id,
        diagnostics: cadence_diagnostics(compiled.diagnostics),
        error: None,
    })
}

pub fn runtime_stop(state: &SharedAppState) -> Result<RuntimeStatusDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    hard_stop_runtime_state(&mut store, false);
    store.push_diagnostic("runtime_stop", "playback stopped".to_string());
    Ok(runtime_status_from_store(&store))
}

pub fn runtime_status(state: &SharedAppState) -> Result<RuntimeStatusDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    Ok(runtime_status_from_store(&store))
}

fn runtime_status_from_store(store: &AppStore) -> RuntimeStatusDto {
    let transport = store.runtime_host.status();
    RuntimeStatusDto {
        rev: store.runtime.rev,
        playing: matches!(transport.state, PlaybackState::Playing),
        program_state: current_program_state(&store.runtime),
        has_program: store.runtime.has_program(),
        last_error: store.runtime.last_error.clone(),
        play_elapsed_ms: store.runtime.elapsed_ms(),
        cycle_position: rational_time(transport.cycle_position),
        cps: rational_time(transport.cps),
    }
}

fn configured_sample_root(store: &AppStore) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CADENCE_SAMPLE_ROOT") {
        return Some(PathBuf::from(path));
    }

    let project_samples = store
        .current_path
        .as_ref()
        .and_then(|path| (!path.contains("://")).then_some(path))
        .and_then(|path| Path::new(path).parent())
        .map(|parent| parent.join("samples"));
    if project_samples.is_some() {
        return project_samples;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        bundled_sample_root()
    }

    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn rational_time(value: PlaybackTime) -> RationalTime {
    RationalTime {
        numerator: value.numerator(),
        denominator: value.denominator(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use tessera::types::GridPos;

    use super::*;
    use crate::{
        application::project::ops::{active_project_mut, project_new_internal},
        infrastructure::dto::SharedAppState,
    };

    #[test]
    fn failed_runtime_commit_marks_last_good_program_as_stale() {
        let state = SharedAppState::new();
        {
            let mut store = state.store.lock().expect("app state lock");
            project_new_internal(&mut store, Some("Runtime".to_string())).expect("seed project");
            let project = active_project_mut(&mut store).expect("project");
            project.init_stage.sample_loads.clear();
            project
                .runtime_graph_mut()
                .nodes
                .get_mut(&GridPos { col: 0, row: 0 })
                .expect("sound node")
                .inline_params
                .insert("value".to_string(), Value::String("sine".to_string()));
        }

        let first = runtime_commit(&state, None, Some(false), Some(true), Some(1))
            .expect("runtime commit should return a dto");
        assert!(first.success, "expected initial playback commit to succeed");
        assert_eq!(first.status.program_state, RuntimeProgramStateDto::Current);

        {
            let mut store = state.store.lock().expect("app state lock");
            let project = active_project_mut(&mut store).expect("project");
            project
                .runtime_graph_mut()
                .nodes
                .remove(&GridPos { col: 1, row: 0 });
            project.runtime_graph_mut().edges.clear();
        }

        let second = runtime_commit(&state, None, Some(false), Some(true), Some(2))
            .expect("runtime commit should return a dto");
        assert!(!second.success, "expected invalid project to fail commit");
        assert_eq!(
            second.status.program_state,
            RuntimeProgramStateDto::StaleLastGood
        );
        assert!(second.status.has_program);
        assert!(second.status.last_error.is_some());

        let stopped = runtime_stop(&state).expect("runtime stop");
        assert!(!stopped.playing);
        assert_eq!(stopped.program_state, RuntimeProgramStateDto::StaleLastGood);
    }
}
