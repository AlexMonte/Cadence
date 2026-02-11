//! Runtime bridge orchestration between editor state and host process.

use bevy::prelude::*;

mod host;
pub mod host_process;
pub mod music_ir;
pub mod protocol;
mod strudel_codegen;

use host::RuntimeHost;
pub use host::RuntimeHostConfig;
use protocol::{
    RuntimeEvent, RuntimeIntent, RuntimeRevisions, RuntimeState, is_supported_protocol_version,
};
pub use strudel_codegen::{CodegenError, generate_scope_code, generate_scope_ir};

use crate::core::{NodeKind, Project, ScopeId};
use crate::editor::AppState;
use crate::editor::gestures::pointer::LayoutChanged;
use crate::editor::menus::TextDraftCommitted;
use crate::editor::state::EditorReady;

pub const RUNTIME_HOST_FLAG: &str = "--runtime-host";

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCommitRequested {
    pub scope: ScopeId,
    pub force: bool,
    pub reason: RuntimeCommitReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCommitReason {
    LayoutCommit,
    HistoryApply,
    ProjectSwap,
    ScopeChange,
    TextDraftApply,
    Startup,
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeEvalCache {
    pub last_code: String,
    pub last_scope: Option<ScopeId>,
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeCommitOverrides {
    text_draft_code: Option<(ScopeId, String)>,
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeScopeTracker {
    current_scope: Option<ScopeId>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<RuntimeIntent>()
        .add_message::<RuntimeEvent>()
        .add_message::<RuntimeCommitRequested>()
        .add_message::<TextDraftCommitted>()
        .init_resource::<RuntimeState>()
        .init_resource::<RuntimeRevisions>()
        .init_resource::<RuntimeEvalCache>()
        .init_resource::<RuntimeCommitOverrides>()
        .init_resource::<RuntimeScopeTracker>()
        .add_plugins(host::plugin)
        .add_systems(OnEnter(EditorReady), emit_startup_runtime_commit_request)
        .add_systems(
            Update,
            (
                emit_runtime_commit_request_on_layout_changed.run_if(in_state(EditorReady)),
                emit_runtime_commit_request_on_scope_change.run_if(in_state(EditorReady)),
                emit_runtime_commit_request_on_text_draft_commit.run_if(in_state(EditorReady)),
                dispatch_eval_on_runtime_commit.run_if(in_state(EditorReady)),
                forward_runtime_intents_to_host,
                pump_runtime_host_events,
                reduce_runtime_events,
            )
                .chain(),
        );
}

fn emit_startup_runtime_commit_request(
    app_state: Res<AppState>,
    mut scope_tracker: ResMut<RuntimeScopeTracker>,
    mut commit_requests: MessageWriter<RuntimeCommitRequested>,
) {
    scope_tracker.current_scope = Some(app_state.current_scope);
    commit_requests.write(RuntimeCommitRequested {
        scope: app_state.current_scope,
        force: true,
        reason: RuntimeCommitReason::Startup,
    });
}

fn emit_runtime_commit_request_on_layout_changed(
    mut layout_changed: MessageReader<LayoutChanged>,
    app_state: Res<AppState>,
    mut commit_requests: MessageWriter<RuntimeCommitRequested>,
) {
    if layout_changed.read().count() == 0 {
        return;
    }

    commit_requests.write(RuntimeCommitRequested {
        scope: app_state.current_scope,
        force: false,
        reason: RuntimeCommitReason::LayoutCommit,
    });
}

fn emit_runtime_commit_request_on_scope_change(
    app_state: Res<AppState>,
    mut scope_tracker: ResMut<RuntimeScopeTracker>,
    mut commit_requests: MessageWriter<RuntimeCommitRequested>,
) {
    let next_scope = app_state.current_scope;
    match scope_tracker.current_scope {
        Some(current) if current == next_scope => {}
        Some(_) => {
            scope_tracker.current_scope = Some(next_scope);
            commit_requests.write(RuntimeCommitRequested {
                scope: next_scope,
                force: false,
                reason: RuntimeCommitReason::ScopeChange,
            });
        }
        None => {
            scope_tracker.current_scope = Some(next_scope);
        }
    }
}

fn emit_runtime_commit_request_on_text_draft_commit(
    mut text_draft_commits: MessageReader<TextDraftCommitted>,
    mut overrides: ResMut<RuntimeCommitOverrides>,
    mut commit_requests: MessageWriter<RuntimeCommitRequested>,
) {
    for commit in text_draft_commits.read() {
        overrides.text_draft_code = Some((commit.scope, commit.code.clone()));
        commit_requests.write(RuntimeCommitRequested {
            scope: commit.scope,
            force: true,
            reason: RuntimeCommitReason::TextDraftApply,
        });
    }
}

fn dispatch_eval_on_runtime_commit(
    mut commit_requests: MessageReader<RuntimeCommitRequested>,
    app_state: Res<AppState>,
    mut revisions: ResMut<RuntimeRevisions>,
    mut runtime_state: ResMut<RuntimeState>,
    mut eval_cache: ResMut<RuntimeEvalCache>,
    mut overrides: ResMut<RuntimeCommitOverrides>,
    mut intents: MessageWriter<RuntimeIntent>,
    mut runtime_events: MessageWriter<RuntimeEvent>,
) {
    let mut has_request_for_current_scope = false;
    let mut force = false;
    let mut has_text_draft_request = false;
    for request in commit_requests.read() {
        if request.scope != app_state.current_scope {
            continue;
        }
        has_request_for_current_scope = true;
        force |= request.force;
        has_text_draft_request |= request.reason == RuntimeCommitReason::TextDraftApply;
    }

    if !has_request_for_current_scope {
        return;
    }

    if !scope_has_playable_graph(&app_state.project, app_state.current_scope) {
        reset_runtime_state_for_unplayable_scope(
            app_state.current_scope,
            runtime_state.as_mut(),
            eval_cache.as_mut(),
            &mut intents,
        );
        return;
    }

    if has_text_draft_request
        && let Some((scope, code)) = overrides.text_draft_code.as_ref()
        && *scope == app_state.current_scope
    {
        let draft_code = code.clone();
        overrides.text_draft_code = None;
        dispatch_eval_code(
            app_state.current_scope,
            force,
            draft_code,
            revisions.as_mut(),
            runtime_state.as_mut(),
            eval_cache.as_mut(),
            &mut intents,
        );
        return;
    }

    match strudel_codegen::generate_scope_code(&app_state.project, app_state.current_scope) {
        Ok(code) => dispatch_eval_code(
            app_state.current_scope,
            force,
            code,
            revisions.as_mut(),
            runtime_state.as_mut(),
            eval_cache.as_mut(),
            &mut intents,
        ),
        Err(err) => {
            let rev = revisions.commit_next();
            let message = format!("Codegen failed: {err}");
            runtime_state.last_error_rev = Some(rev);
            runtime_state.last_error = Some(message.clone());
            runtime_events.write(RuntimeEvent::error(rev, message));
        }
    }
}

fn dispatch_eval_code(
    scope: ScopeId,
    force: bool,
    code: String,
    revisions: &mut RuntimeRevisions,
    runtime_state: &mut RuntimeState,
    eval_cache: &mut RuntimeEvalCache,
    intents: &mut MessageWriter<RuntimeIntent>,
) {
    let unchanged = !force && eval_cache.last_scope == Some(scope) && eval_cache.last_code == code;
    if unchanged {
        runtime_state.last_error = None;
        runtime_state.last_error_rev = None;
        return;
    }

    let rev = revisions.commit_next();
    runtime_state.last_eval_rev = rev;
    runtime_state.last_code = code.clone();
    runtime_state.last_error = None;
    runtime_state.last_error_rev = None;

    eval_cache.last_scope = Some(scope);
    eval_cache.last_code = code.clone();

    intents.write(RuntimeIntent::eval(rev, code));
}

fn reset_runtime_state_for_unplayable_scope(
    scope: ScopeId,
    runtime_state: &mut RuntimeState,
    eval_cache: &mut RuntimeEvalCache,
    intents: &mut MessageWriter<RuntimeIntent>,
) {
    if runtime_state.playing {
        intents.write(RuntimeIntent::stop());
        runtime_state.playing = false;
    }

    runtime_state.last_error = None;
    runtime_state.last_error_rev = None;
    runtime_state.last_code.clear();
    eval_cache.last_scope = Some(scope);
    eval_cache.last_code.clear();
}

pub fn is_runtime_host_mode() -> bool {
    std::env::args().any(|arg| arg == RUNTIME_HOST_FLAG)
}

pub fn run_runtime_host_mode_or_exit() -> ! {
    host_process::run_runtime_host_mode_or_exit()
}

fn forward_runtime_intents_to_host(
    mut intents: MessageReader<RuntimeIntent>,
    host: Option<Res<RuntimeHost>>,
    mut runtime_state: ResMut<RuntimeState>,
    mut runtime_events: MessageWriter<RuntimeEvent>,
) {
    for intent in intents.read() {
        if !is_supported_protocol_version(intent.protocol_version()) {
            runtime_events.write(RuntimeEvent::error(
                0,
                format!(
                    "Unsupported runtime protocol version {}",
                    intent.protocol_version()
                ),
            ));
            continue;
        }

        if let Some(host) = host.as_ref() {
            if let Err(err) = host.try_send(intent.clone()) {
                runtime_events.write(RuntimeEvent::error(
                    0,
                    format!("Failed to send intent to runtime host: {err}"),
                ));
                continue;
            }

            // Mirror transport controls locally so UI feedback is immediate.
            match intent {
                RuntimeIntent::Play { .. } => {
                    runtime_state.playing = true;
                }
                RuntimeIntent::Stop { .. } => {
                    runtime_state.playing = false;
                }
                RuntimeIntent::SetTempo { cpm, .. } => {
                    runtime_state.tempo_cpm = *cpm;
                }
                RuntimeIntent::Eval { .. } | RuntimeIntent::Validate { .. } => {}
            }
        } else {
            runtime_events.write(RuntimeEvent::error(
                0,
                "Runtime host unavailable".to_string(),
            ));
        }
    }
}

fn pump_runtime_host_events(
    host: Option<Res<RuntimeHost>>,
    mut runtime_events: MessageWriter<RuntimeEvent>,
) {
    let Some(host) = host else {
        return;
    };
    for event in host.try_drain_events() {
        runtime_events.write(event);
    }
}

fn reduce_runtime_events(
    mut events: MessageReader<RuntimeEvent>,
    mut revisions: ResMut<RuntimeRevisions>,
    mut runtime_state: ResMut<RuntimeState>,
) {
    for event in events.read() {
        if !is_supported_protocol_version(event.protocol_version()) {
            runtime_state.last_error = Some(format!(
                "Received unsupported runtime protocol version {}",
                event.protocol_version()
            ));
            continue;
        }

        match event {
            RuntimeEvent::Ready { .. } => {
                runtime_state.ready = true;
            }
            RuntimeEvent::Status {
                playing,
                current_rev,
                ..
            } => {
                // Drop stale status updates that refer to older commits.
                if *current_rev < revisions.commit_rev {
                    continue;
                }
                runtime_state.playing = *playing;
                runtime_state.last_ok_rev = Some(*current_rev);
                revisions.mark_runtime_rev(*current_rev);
                runtime_state.last_error = None;
            }
            RuntimeEvent::Error { rev, message, .. } => {
                // Drop stale errors from superseded commits.
                if *rev < revisions.commit_rev {
                    continue;
                }
                runtime_state.last_error_rev = Some(*rev);
                runtime_state.last_error = Some(message.clone());
                revisions.mark_runtime_rev(*rev);
            }
            RuntimeEvent::Validation { .. } => {}
        }
    }
}

fn scope_has_playable_graph(project: &Project, scope_id: ScopeId) -> bool {
    let Some(scope) = project.model.scopes.get(&scope_id) else {
        return false;
    };

    let mut has_pattern = false;
    let mut has_output = false;

    for node_id in &scope.node_ids {
        let Some(node) = project.model.nodes.get(node_id) else {
            continue;
        };
        match node.kind {
            NodeKind::Pattern { .. } => has_pattern = true,
            NodeKind::Output => has_output = true,
            _ => {}
        }

        if has_pattern && has_output {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::core::{Layout, ModelNode, NodeId, Scope};

    use super::*;

    fn make_node(kind: NodeKind, parent_scope: ScopeId) -> ModelNode {
        ModelNode {
            id: NodeId::new(),
            kind,
            parent_scope,
            params: HashMap::new(),
            input_ports: vec![],
            output_ports: vec![],
        }
    }

    fn make_project(pattern: &str, include_output: bool) -> (Project, ScopeId) {
        let mut project = Project::new("runtime-loop".to_string());
        let scope = project.root_scope();

        let mut pattern_node = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern_node.params.insert(
            "pattern".to_string(),
            serde_json::Value::String(pattern.to_string()),
        );
        pattern_node.params.insert(
            "sample".to_string(),
            serde_json::Value::String(pattern.to_string()),
        );
        project.add_node(scope, pattern_node);

        if include_output {
            project.add_node(scope, make_node(NodeKind::Output, scope));
        }

        (project, scope)
    }

    fn add_playable_scope(project: &mut Project, parent_scope: ScopeId, sample: &str) -> ScopeId {
        let scope = ScopeId::new();
        project.model.scopes.insert(
            scope,
            Scope {
                id: scope,
                parent_scope: Some(parent_scope),
                child_scopes: Vec::new(),
                node_ids: Vec::new(),
            },
        );
        project
            .model
            .scopes
            .get_mut(&parent_scope)
            .expect("parent scope should exist")
            .child_scopes
            .push(scope);
        project.layout.layouts.insert(
            scope,
            Layout {
                scope_id: scope,
                node_positions: HashMap::new(),
                camera_pos: (0.0, 0.0),
                zoom: 1.0,
            },
        );

        let mut pattern_node = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern_node.params.insert(
            "pattern".to_string(),
            serde_json::Value::String(sample.to_string()),
        );
        pattern_node.params.insert(
            "sample".to_string(),
            serde_json::Value::String(sample.to_string()),
        );
        project.add_node(scope, pattern_node);
        project.add_node(scope, make_node(NodeKind::Output, scope));
        scope
    }

    fn app_with_runtime_commit_dispatch(project: Project, scope: ScopeId) -> App {
        let mut app = App::new();
        app.add_message::<RuntimeCommitRequested>();
        app.add_message::<TextDraftCommitted>();
        app.add_message::<RuntimeIntent>();
        app.add_message::<RuntimeEvent>();
        app.insert_resource(AppState {
            project,
            current_scope: scope,
        });
        app.insert_resource(RuntimeRevisions::default());
        app.insert_resource(RuntimeState::default());
        app.insert_resource(RuntimeEvalCache::default());
        app.insert_resource(RuntimeCommitOverrides::default());
        app.add_systems(
            Update,
            (
                emit_runtime_commit_request_on_text_draft_commit,
                dispatch_eval_on_runtime_commit,
            )
                .chain(),
        );
        app
    }

    fn app_with_scope_change_pipeline(project: Project, scope: ScopeId) -> App {
        let mut app = App::new();
        app.add_message::<RuntimeCommitRequested>();
        app.add_message::<RuntimeIntent>();
        app.add_message::<RuntimeEvent>();
        app.insert_resource(AppState {
            project,
            current_scope: scope,
        });
        app.insert_resource(RuntimeRevisions::default());
        app.insert_resource(RuntimeState::default());
        app.insert_resource(RuntimeEvalCache::default());
        app.insert_resource(RuntimeCommitOverrides::default());
        app.insert_resource(RuntimeScopeTracker {
            current_scope: Some(scope),
        });
        app.add_systems(
            Update,
            (
                emit_runtime_commit_request_on_scope_change,
                dispatch_eval_on_runtime_commit,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn scope_has_playable_graph_requires_pattern_and_output() {
        let mut project = Project::new("scope-playable".to_string());
        let scope = project.root_scope();

        project.add_node(
            scope,
            make_node(
                NodeKind::Pattern {
                    pattern_type: "sample".to_string(),
                },
                scope,
            ),
        );
        assert!(!scope_has_playable_graph(&project, scope));

        project.add_node(scope, make_node(NodeKind::Output, scope));
        assert!(scope_has_playable_graph(&project, scope));
    }

    #[test]
    fn runtime_commit_dispatches_eval_for_playable_scope() {
        let (project, scope) = make_project("bd", true);
        let mut app = app_with_runtime_commit_dispatch(project, scope);

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();

        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();
        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();

        assert_eq!(intents.len(), 1);
        let RuntimeIntent::Eval { rev, .. } = intents[0].clone() else {
            panic!("expected runtime eval intent");
        };
        assert_eq!(rev, 1);
        assert_eq!(app.world().resource::<RuntimeRevisions>().commit_rev, 1);
    }

    #[test]
    fn undo_redo_commit_forces_eval_even_when_layout_event_absent() {
        let (project, scope) = make_project("bd", true);
        let mut app = app_with_runtime_commit_dispatch(project, scope);

        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();
        let first: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(first.len(), 1);

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: true,
            reason: RuntimeCommitReason::HistoryApply,
        });
        app.update();
        let second: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(second.len(), 1);
        let RuntimeIntent::Eval { rev, .. } = second[0].clone() else {
            panic!("expected runtime eval intent");
        };
        assert_eq!(rev, 2);
    }

    #[test]
    fn project_swap_commit_dispatches_eval() {
        let (project, scope) = make_project("bd", true);
        let mut app = app_with_runtime_commit_dispatch(project, scope);
        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();
        let _ = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect::<Vec<_>>();

        let (swapped_project, swapped_scope) = make_project("sn", true);
        {
            let mut app_state = app.world_mut().resource_mut::<AppState>();
            app_state.project = swapped_project;
            app_state.current_scope = swapped_scope;
        }

        app.world_mut().write_message(RuntimeCommitRequested {
            scope: swapped_scope,
            force: true,
            reason: RuntimeCommitReason::ProjectSwap,
        });
        app.update();

        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(intents.len(), 1);
        let RuntimeIntent::Eval { rev, code, .. } = intents[0].clone() else {
            panic!("expected runtime eval intent");
        };
        assert_eq!(rev, 2);
        assert!(code.contains("s(\"sn\")"));
    }

    #[test]
    fn non_playable_scope_stops_playback_without_eval() {
        let (project, scope) = make_project("bd", true);
        let mut app = app_with_runtime_commit_dispatch(project, scope);
        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();
        let _ = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect::<Vec<_>>();

        let (non_playable, non_playable_scope) = make_project("bd", false);
        {
            let mut app_state = app.world_mut().resource_mut::<AppState>();
            app_state.project = non_playable;
            app_state.current_scope = non_playable_scope;
        }
        {
            let mut runtime_state = app.world_mut().resource_mut::<RuntimeState>();
            runtime_state.playing = true;
        }

        app.world_mut().write_message(RuntimeCommitRequested {
            scope: non_playable_scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();

        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(intents.len(), 1);
        assert!(matches!(intents[0], RuntimeIntent::Stop { .. }));
        assert_eq!(app.world().resource::<RuntimeRevisions>().commit_rev, 1);
        assert_eq!(app.world().resource::<RuntimeState>().last_error, None);
    }

    #[test]
    fn identical_code_skips_eval_when_not_forced() {
        let (project, scope) = make_project("bd", true);
        let mut app = app_with_runtime_commit_dispatch(project, scope);
        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();
        let first: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(first.len(), 1);

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();
        let second: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert!(second.is_empty());
        assert_eq!(app.world().resource::<RuntimeRevisions>().commit_rev, 1);
    }

    #[test]
    fn forced_commit_bypasses_dedupe() {
        let (project, scope) = make_project("bd", true);
        let mut app = app_with_runtime_commit_dispatch(project, scope);
        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();
        let _ = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect::<Vec<_>>();

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: true,
            reason: RuntimeCommitReason::HistoryApply,
        });
        app.update();
        let second: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(second.len(), 1);
        let RuntimeIntent::Eval { rev, .. } = second[0].clone() else {
            panic!("expected runtime eval intent");
        };
        assert_eq!(rev, 2);
    }

    #[test]
    fn scope_change_emits_eval_without_explicit_commit_message() {
        let (mut project, root_scope) = make_project("bd", true);
        let alternate_scope = add_playable_scope(&mut project, root_scope, "sn");
        let mut app = app_with_scope_change_pipeline(project, root_scope);

        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();

        {
            let mut app_state = app.world_mut().resource_mut::<AppState>();
            app_state.current_scope = alternate_scope;
        }
        app.update();

        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(intents.len(), 1);
        let RuntimeIntent::Eval { rev, code, .. } = intents[0].clone() else {
            panic!("expected eval intent for scope change");
        };
        assert_eq!(rev, 1);
        assert!(code.contains("s(\"sn\")"));
    }

    #[test]
    fn text_draft_commit_dispatches_eval_without_model_mutation() {
        let (project, scope) = make_project("bd", true);
        let mut app = app_with_runtime_commit_dispatch(project, scope);
        let mut cursor = app
            .world()
            .resource::<Messages<RuntimeIntent>>()
            .get_cursor();

        app.world_mut().write_message(TextDraftCommitted {
            scope,
            code: "let main = s(\"sn\");\nmain\n".to_string(),
            draft_rev: 7,
        });
        app.update();

        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(intents.len(), 1);
        let RuntimeIntent::Eval { rev, code, .. } = intents[0].clone() else {
            panic!("expected runtime eval intent");
        };
        assert_eq!(rev, 1);
        assert!(code.contains("s(\"sn\")"));

        app.world_mut().write_message(RuntimeCommitRequested {
            scope,
            force: false,
            reason: RuntimeCommitReason::LayoutCommit,
        });
        app.update();

        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<RuntimeIntent>>())
            .cloned()
            .collect();
        assert_eq!(intents.len(), 1);
        let RuntimeIntent::Eval { rev, code, .. } = intents[0].clone() else {
            panic!("expected runtime eval intent");
        };
        assert_eq!(rev, 2);
        assert!(code.contains("s(\"bd\")"));
    }
}
