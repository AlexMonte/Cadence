//! Editor history, undo/redo checkpoints, and autosave orchestration.

use bevy::prelude::*;

use crate::EditorSystemSet;
use crate::core::{Project, ScopeId};
use crate::editor::AppState;
use crate::editor::gestures::pointer::LayoutChanged;
use crate::editor::state::{EditorInteractive, EditorReady, ProjectSession};
use crate::runtime::{RuntimeCommitReason, RuntimeCommitRequested};
use crate::store::{ProjectStore, StoreError};

const DEFAULT_HISTORY_MAX_ENTRIES: usize = 100;
const DEFAULT_AUTOSAVE_PATH: &str = "current_project";
const DEFAULT_AUTOSAVE_DEBOUNCE_SECS: f32 = 0.75;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<HistoryIntent>();
    app.init_resource::<EditorHistory>();
    app.init_resource::<AutosaveConfig>();
    app.init_resource::<AutosaveState>();
    app.init_resource::<HistoryApplyGuard>();
    app.add_systems(OnEnter(ProjectSession::Loaded), seed_history_baseline);
    app.add_systems(
        Update,
        (
            emit_history_intents_from_shortcuts
                .run_if(in_state(EditorInteractive))
                .in_set(EditorSystemSet::PointerInput),
            apply_history_intents
                .run_if(in_state(EditorReady))
                .in_set(EditorSystemSet::SelectionResolution),
            checkpoint_on_layout_changed
                .run_if(in_state(EditorReady))
                .in_set(EditorSystemSet::RenderSelection),
            autosave_dirty_project
                .run_if(in_state(EditorReady))
                .in_set(EditorSystemSet::RenderSelection),
        ),
    );
}

/// User intents routed into history operations.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryIntent {
    Undo,
    Redo,
    SaveNow,
}

/// Snapshot of project + active scope for deterministic history playback.
#[derive(Debug, Clone)]
pub struct ProjectSnapshot {
    pub project: Project,
    pub scope: ScopeId,
}

impl ProjectSnapshot {
    fn from_app_state(app_state: &AppState) -> Self {
        Self {
            project: app_state.project.clone(),
            scope: app_state.current_scope,
        }
    }
}

/// Ring-buffer-like history state for past/future snapshots.
#[derive(Resource, Debug)]
pub struct EditorHistory {
    pub past: Vec<ProjectSnapshot>,
    pub future: Vec<ProjectSnapshot>,
    pub max_entries: usize,
}

impl Default for EditorHistory {
    fn default() -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            max_entries: DEFAULT_HISTORY_MAX_ENTRIES,
        }
    }
}

impl EditorHistory {
    fn push_checkpoint(&mut self, snapshot: ProjectSnapshot) {
        self.past.push(snapshot);
        self.trim();
    }

    fn trim(&mut self) {
        if self.past.len() <= self.max_entries {
            return;
        }

        let overflow = self.past.len().saturating_sub(self.max_entries);
        self.past.drain(0..overflow);
    }
}

/// Autosave destination and debounce behavior.
#[derive(Resource, Debug, Clone)]
pub struct AutosaveConfig {
    pub path: String,
    pub debounce_secs: f32,
}

impl Default for AutosaveConfig {
    fn default() -> Self {
        Self {
            path: DEFAULT_AUTOSAVE_PATH.to_string(),
            debounce_secs: DEFAULT_AUTOSAVE_DEBOUNCE_SECS,
        }
    }
}

/// Mutable autosave runtime state.
#[derive(Resource, Debug)]
pub struct AutosaveState {
    pub dirty: bool,
    pub timer: Timer,
    pub last_error: Option<String>,
}

impl Default for AutosaveState {
    fn default() -> Self {
        Self {
            dirty: false,
            timer: Timer::from_seconds(DEFAULT_AUTOSAVE_DEBOUNCE_SECS, TimerMode::Once),
            last_error: None,
        }
    }
}

/// Guard to prevent history replay from recursively producing checkpoints.
#[derive(Resource, Debug, Default)]
pub struct HistoryApplyGuard {
    pub active: bool,
}

fn seed_history_baseline(
    app_state: Res<AppState>,
    mut history: ResMut<EditorHistory>,
    config: Res<AutosaveConfig>,
    mut autosave: ResMut<AutosaveState>,
) {
    history.past.clear();
    history.future.clear();
    history.push_checkpoint(ProjectSnapshot::from_app_state(&app_state));

    autosave.dirty = false;
    autosave.last_error = None;
    autosave.timer = new_autosave_timer(&config);
}

fn emit_history_intents_from_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut intents: MessageWriter<HistoryIntent>,
) {
    let primary_modifier = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight);
    if !primary_modifier {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyS) {
        intents.write(HistoryIntent::SaveNow);
    }

    if keyboard.just_pressed(KeyCode::KeyZ) {
        let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        intents.write(if shift {
            HistoryIntent::Redo
        } else {
            HistoryIntent::Undo
        });
    } else if keyboard.just_pressed(KeyCode::KeyY) {
        intents.write(HistoryIntent::Redo);
    }
}

pub fn apply_history_intents(
    mut intents: MessageReader<HistoryIntent>,
    mut history: ResMut<EditorHistory>,
    mut app_state: ResMut<AppState>,
    config: Res<AutosaveConfig>,
    mut autosave: ResMut<AutosaveState>,
    mut guard: ResMut<HistoryApplyGuard>,
    mut runtime_commits: MessageWriter<RuntimeCommitRequested>,
) {
    for intent in intents.read() {
        match intent {
            HistoryIntent::Undo => {
                if history.past.len() < 2 {
                    continue;
                }
                let Some(current) = history.past.pop() else {
                    continue;
                };
                history.future.push(current);
                if let Some(previous) = history.past.last().cloned() {
                    apply_snapshot_and_notify(
                        &previous,
                        &mut app_state,
                        &mut guard,
                        &mut autosave,
                        &config,
                        &mut runtime_commits,
                    );
                }
            }
            HistoryIntent::Redo => {
                let Some(next) = history.future.pop() else {
                    continue;
                };
                history.push_checkpoint(next.clone());
                apply_snapshot_and_notify(
                    &next,
                    &mut app_state,
                    &mut guard,
                    &mut autosave,
                    &config,
                    &mut runtime_commits,
                );
            }
            HistoryIntent::SaveNow => {
                save_project_now(&app_state, &config, &mut autosave);
            }
        }
    }
}

pub fn checkpoint_on_layout_changed(
    mut layout_changed: MessageReader<LayoutChanged>,
    app_state: Res<AppState>,
    mut history: ResMut<EditorHistory>,
    mut autosave: ResMut<AutosaveState>,
    config: Res<AutosaveConfig>,
    guard: Res<HistoryApplyGuard>,
) {
    if guard.active {
        return;
    }

    let count = layout_changed.read().count();
    if count == 0 {
        return;
    }

    history.push_checkpoint(ProjectSnapshot::from_app_state(&app_state));
    history.future.clear();
    mark_autosave_dirty(&mut autosave, &config);
}

pub fn autosave_dirty_project(
    time: Res<Time>,
    app_state: Res<AppState>,
    config: Res<AutosaveConfig>,
    mut autosave: ResMut<AutosaveState>,
) {
    if !autosave.dirty {
        return;
    }

    autosave.timer.tick(time.delta());
    if !autosave.timer.just_finished() {
        return;
    }

    let save_result = ProjectStore::save_project(&app_state.project, config.path.as_str());
    update_autosave_from_save_result(save_result, &mut autosave, Some(&config));
}

fn apply_snapshot(
    snapshot: &ProjectSnapshot,
    app_state: &mut AppState,
    guard: &mut HistoryApplyGuard,
) {
    guard.active = true;
    app_state.project = snapshot.project.clone();
    app_state.current_scope = snapshot.scope;
    guard.active = false;
}

fn apply_snapshot_and_notify(
    snapshot: &ProjectSnapshot,
    app_state: &mut AppState,
    guard: &mut HistoryApplyGuard,
    autosave: &mut AutosaveState,
    config: &AutosaveConfig,
    runtime_commits: &mut MessageWriter<RuntimeCommitRequested>,
) {
    apply_snapshot(snapshot, app_state, guard);
    mark_autosave_dirty(autosave, config);
    emit_history_runtime_commit(app_state.current_scope, runtime_commits);
}

fn emit_history_runtime_commit(
    scope: ScopeId,
    runtime_commits: &mut MessageWriter<RuntimeCommitRequested>,
) {
    runtime_commits.write(RuntimeCommitRequested {
        scope,
        force: true,
        reason: RuntimeCommitReason::HistoryApply,
    });
}

fn save_project_now(app_state: &AppState, config: &AutosaveConfig, autosave: &mut AutosaveState) {
    let save_result = ProjectStore::save_project(&app_state.project, config.path.as_str());
    update_autosave_from_save_result(save_result, autosave, None);
}

fn mark_autosave_dirty(autosave: &mut AutosaveState, config: &AutosaveConfig) {
    autosave.dirty = true;
    autosave.timer = new_autosave_timer(config);
}

fn update_autosave_from_save_result(
    save_result: Result<(), StoreError>,
    autosave: &mut AutosaveState,
    retry_config: Option<&AutosaveConfig>,
) {
    match save_result {
        Ok(()) => {
            autosave.dirty = false;
            autosave.last_error = None;
        }
        Err(err) => {
            autosave.dirty = true;
            autosave.last_error = Some(err.to_string());
            if let Some(config) = retry_config {
                autosave.timer = new_autosave_timer(config);
            }
        }
    }
}

fn new_autosave_timer(config: &AutosaveConfig) -> Timer {
    Timer::from_seconds(config.debounce_secs.max(0.01), TimerMode::Once)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use super::*;
    use crate::core::{ModelNode, NodeId, NodeKind, PortDescriptor, Project, default_layout};

    // Invariants:
    // - layout checkpoints append exactly once per commit and clear redo history.
    // - undo/redo transitions are deterministic and bounded by available snapshots.
    // - history max length is enforced.
    // - apply guard blocks checkpoint writes.
    // - autosave success clears dirty and failure preserves dirty.
    // - SaveNow bypasses debounce.

    fn project_with_node_and_position(name: &str, node_pos: Vec2) -> (Project, ScopeId, NodeId) {
        let mut project = Project::new(name.to_string());
        let scope = project.root_scope();
        let node_id = NodeId::new();
        project.add_node(
            scope,
            ModelNode {
                id: node_id,
                kind: NodeKind::Pattern {
                    pattern_type: "test".to_string(),
                },
                parent_scope: scope,
                params: HashMap::new(),
                input_ports: vec![PortDescriptor {
                    name: "in".to_string(),
                    port_type: "signal".to_string(),
                }],
                output_ports: vec![PortDescriptor {
                    name: "out".to_string(),
                    port_type: "signal".to_string(),
                }],
            },
        );

        let layout = project
            .layout
            .layouts
            .entry(scope)
            .or_insert_with(|| default_layout(scope));
        layout
            .node_positions
            .insert(node_id, (node_pos.x, node_pos.y));

        (project, scope, node_id)
    }

    fn app_with_history_systems(project: Project, scope: ScopeId) -> App {
        let mut app = App::new();
        app.add_message::<HistoryIntent>();
        app.add_message::<LayoutChanged>();
        app.add_message::<RuntimeCommitRequested>();
        app.insert_resource(AppState {
            project,
            current_scope: scope,
        });
        app.insert_resource(EditorHistory::default());
        app.insert_resource(AutosaveConfig::default());
        app.insert_resource(AutosaveState::default());
        app.insert_resource(HistoryApplyGuard::default());
        app.insert_resource(Time::<()>::default());
        app.add_systems(
            Update,
            (
                apply_history_intents,
                checkpoint_on_layout_changed,
                autosave_dirty_project,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn checkpoint_pushes_once_per_layout_changed() {
        let (project, scope, _node_id) = project_with_node_and_position("p0", Vec2::ZERO);
        let mut app = app_with_history_systems(project, scope);

        let baseline_project = app.world().resource::<AppState>().project.clone();
        app.world_mut()
            .resource_mut::<EditorHistory>()
            .past
            .push(ProjectSnapshot {
                project: baseline_project,
                scope,
            });

        {
            let mut state = app.world_mut().resource_mut::<AppState>();
            state.project.name = "p1".to_string();
        }
        app.world_mut()
            .write_message(LayoutChanged { scope: Some(scope) });
        app.update();

        let history = app.world().resource::<EditorHistory>();
        assert_eq!(history.past.len(), 2);
        assert_eq!(history.past[0].project.name, "p0");
        assert_eq!(history.past[1].project.name, "p1");
        assert!(history.future.is_empty());
    }

    #[test]
    fn undo_restores_previous_snapshot_and_clears_future_correctly() {
        let (project, scope, _node_id) = project_with_node_and_position("p0", Vec2::ZERO);
        let mut app = app_with_history_systems(project, scope);

        {
            let mut history = app.world_mut().resource_mut::<EditorHistory>();
            history.past.push(ProjectSnapshot {
                project: Project::new("p0".to_string()),
                scope,
            });
            history.past.push(ProjectSnapshot {
                project: Project::new("p1".to_string()),
                scope,
            });
            history.past.push(ProjectSnapshot {
                project: Project::new("p2".to_string()),
                scope,
            });
        }
        app.world_mut().resource_mut::<AppState>().project = Project::new("p2".to_string());
        app.world_mut().write_message(HistoryIntent::Undo);
        app.update();

        let app_state = app.world().resource::<AppState>();
        let history = app.world().resource::<EditorHistory>();
        assert_eq!(app_state.project.name, "p1");
        assert_eq!(history.past.len(), 2);
        assert_eq!(history.future.len(), 1);
        assert_eq!(history.future[0].project.name, "p2");
    }

    #[test]
    fn redo_restores_forward_snapshot() {
        let (project, scope, _node_id) = project_with_node_and_position("p0", Vec2::ZERO);
        let mut app = app_with_history_systems(project, scope);

        {
            let mut history = app.world_mut().resource_mut::<EditorHistory>();
            history.past.push(ProjectSnapshot {
                project: Project::new("p0".to_string()),
                scope,
            });
            history.past.push(ProjectSnapshot {
                project: Project::new("p1".to_string()),
                scope,
            });
            history.future.push(ProjectSnapshot {
                project: Project::new("p2".to_string()),
                scope,
            });
        }
        app.world_mut().resource_mut::<AppState>().project = Project::new("p1".to_string());
        app.world_mut().write_message(HistoryIntent::Redo);
        app.update();

        let app_state = app.world().resource::<AppState>();
        let history = app.world().resource::<EditorHistory>();
        assert_eq!(app_state.project.name, "p2");
        assert_eq!(
            history.past.last().map(|s| s.project.name.as_str()),
            Some("p2")
        );
        assert!(history.future.is_empty());
    }

    #[test]
    fn undo_when_single_snapshot_is_noop() {
        let (project, scope, _node_id) = project_with_node_and_position("p0", Vec2::ZERO);
        let mut app = app_with_history_systems(project, scope);
        app.world_mut()
            .resource_mut::<EditorHistory>()
            .past
            .push(ProjectSnapshot {
                project: Project::new("p0".to_string()),
                scope,
            });
        app.world_mut().resource_mut::<AppState>().project = Project::new("p0".to_string());

        app.world_mut().write_message(HistoryIntent::Undo);
        app.update();

        let app_state = app.world().resource::<AppState>();
        let history = app.world().resource::<EditorHistory>();
        assert_eq!(app_state.project.name, "p0");
        assert_eq!(history.past.len(), 1);
        assert!(history.future.is_empty());
    }

    #[test]
    fn history_trim_respects_max_entries() {
        let mut history = EditorHistory {
            max_entries: 3,
            ..Default::default()
        };
        let scope = ScopeId::new();
        for idx in 0..5 {
            history.push_checkpoint(ProjectSnapshot {
                project: Project::new(format!("p{idx}")),
                scope,
            });
        }

        assert_eq!(history.past.len(), 3);
        assert_eq!(history.past[0].project.name, "p2");
        assert_eq!(history.past[2].project.name, "p4");
    }

    #[test]
    fn apply_guard_prevents_recursive_checkpoint() {
        let (project, scope, _node_id) = project_with_node_and_position("p0", Vec2::ZERO);
        let mut app = app_with_history_systems(project, scope);
        {
            let mut history = app.world_mut().resource_mut::<EditorHistory>();
            history.past.push(ProjectSnapshot {
                project: Project::new("p0".to_string()),
                scope,
            });
        }
        app.world_mut().resource_mut::<HistoryApplyGuard>().active = true;
        app.world_mut()
            .write_message(LayoutChanged { scope: Some(scope) });
        app.update();

        let history = app.world().resource::<EditorHistory>();
        let autosave = app.world().resource::<AutosaveState>();
        assert_eq!(history.past.len(), 1);
        assert!(!autosave.dirty);
    }

    #[test]
    fn autosave_success_clears_dirty_and_failures_keep_dirty() {
        let (project, scope, _node_id) = project_with_node_and_position("p0", Vec2::ZERO);
        let mut app = app_with_history_systems(project, scope);
        let temp_dir = tempfile::TempDir::new().unwrap();

        {
            let mut config = app.world_mut().resource_mut::<AutosaveConfig>();
            config.path = temp_dir.path().to_string_lossy().to_string();
            config.debounce_secs = 0.01;
        }
        {
            let mut autosave = app.world_mut().resource_mut::<AutosaveState>();
            autosave.dirty = true;
            autosave.timer = Timer::from_seconds(0.01, TimerMode::Once);
        }
        app.world_mut()
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_millis(20));
        app.update();

        {
            let autosave = app.world().resource::<AutosaveState>();
            assert!(!autosave.dirty);
            assert!(autosave.last_error.is_none());
        }

        let file_path = temp_dir.path().join("not_a_directory");
        std::fs::write(&file_path, "x").unwrap();
        {
            let mut config = app.world_mut().resource_mut::<AutosaveConfig>();
            config.path = file_path.join("nested").to_string_lossy().to_string();
            config.debounce_secs = 0.01;
        }
        {
            let mut autosave = app.world_mut().resource_mut::<AutosaveState>();
            autosave.dirty = true;
            autosave.timer = Timer::from_seconds(0.01, TimerMode::Once);
        }
        app.world_mut()
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_millis(20));
        app.update();

        let autosave = app.world().resource::<AutosaveState>();
        assert!(autosave.dirty);
        assert!(autosave.last_error.is_some());
    }

    #[test]
    fn save_now_bypasses_debounce() {
        let (project, scope, _node_id) = project_with_node_and_position("p0", Vec2::ZERO);
        let mut app = app_with_history_systems(project, scope);
        let temp_dir = tempfile::TempDir::new().unwrap();

        {
            let mut config = app.world_mut().resource_mut::<AutosaveConfig>();
            config.path = temp_dir.path().to_string_lossy().to_string();
            config.debounce_secs = 1000.0;
        }
        {
            let mut autosave = app.world_mut().resource_mut::<AutosaveState>();
            autosave.dirty = true;
            autosave.timer = Timer::from_seconds(1000.0, TimerMode::Once);
        }

        app.world_mut().write_message(HistoryIntent::SaveNow);
        app.update();

        let autosave = app.world().resource::<AutosaveState>();
        assert!(!autosave.dirty);
        assert!(temp_dir.path().join("model.json").exists());
        assert!(temp_dir.path().join("layout.json").exists());
    }
}
