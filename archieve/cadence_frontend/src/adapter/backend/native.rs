use std::sync::OnceLock;

use cadence::infrastructure::dto::SharedAppState;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::types::*;

static APP_STATE: OnceLock<SharedAppState> = OnceLock::new();

fn state() -> &'static SharedAppState {
    APP_STATE.get_or_init(SharedAppState::new)
}

fn convert<T, R>(value: T) -> Result<R, String>
where
    T: Serialize,
    R: DeserializeOwned,
{
    let raw = serde_json::to_value(value).map_err(|err| err.to_string())?;
    serde_json::from_value(raw).map_err(|err| err.to_string())
}

fn lift<T, R>(result: Result<T, String>) -> Result<R, String>
where
    T: Serialize,
    R: DeserializeOwned,
{
    result.and_then(convert)
}

fn stringify_error<T>(value: T) -> String
where
    T: Serialize,
{
    serde_json::to_string(&value).unwrap_or_else(|_| "backend operation failed".to_string())
}

pub async fn project_create(name: String) -> Result<ProjectDto, String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err("name cannot be empty".to_string());
    }

    lift(cadence::infrastructure::project_create(
        state(),
        normalized.to_string(),
    ))
}

pub async fn project_new(name: Option<String>) -> Result<ProjectDto, String> {
    lift(cadence::infrastructure::project_new(state(), name))
}

pub async fn project_rename(name: String) -> Result<ProjectViewDto, String> {
    lift(cadence::infrastructure::project_rename(state(), name))
}

pub async fn project_open_path(path: String) -> Result<ProjectDto, String> {
    lift(cadence::infrastructure::project_open_path(state(), path))
}

pub async fn project_save(path: Option<String>) -> Result<String, String> {
    cadence::infrastructure::project_save(state(), path)
}

pub async fn project_save_current() -> Result<String, String> {
    cadence::infrastructure::project_save_current(state())
}

pub async fn project_save_as(path: String) -> Result<String, String> {
    cadence::infrastructure::project_save_as(state(), path)
}

pub async fn project_dirty_status() -> Result<ProjectDirtyStatusDto, String> {
    lift(cadence::infrastructure::project_dirty_status(state()))
}

pub async fn project_snapshot() -> Result<ProjectViewDto, String> {
    lift(cadence::infrastructure::project_snapshot(state()))
}

pub async fn project_bootstrap() -> Result<ProjectViewDto, String> {
    lift(cadence::infrastructure::project_bootstrap(state()))
}

pub async fn project_init_snapshot() -> Result<InitStageSnapshotDto, String> {
    lift(cadence::infrastructure::project_init_snapshot(state()))
}

pub async fn project_sample_library() -> Result<SampleLibrarySnapshotDto, String> {
    lift(cadence::infrastructure::project_sample_library(state()))
}

pub async fn project_init_apply(ops: Vec<InitStageOp>) -> Result<InitStageSnapshotDto, String> {
    let ops = convert::<_, Vec<cadence::infrastructure::dto::InitStageOp>>(ops)?;
    lift(cadence::infrastructure::project_init_apply(state(), ops))
}

pub async fn project_compile_preview() -> Result<ProjectCompilePreviewDto, String> {
    lift(cadence::infrastructure::project_compile_preview(state()))
}

pub async fn project_pick_open_path() -> Result<Option<String>, String> {
    cadence::infrastructure::project_pick_open_path(state()).await
}

pub async fn project_pick_save_path() -> Result<Option<String>, String> {
    cadence::infrastructure::project_pick_save_path(state()).await
}

pub async fn project_recovery_status() -> Result<ProjectPathChoiceDto, String> {
    lift(cadence::infrastructure::project_recovery_status(state()))
}

pub async fn project_recovery_write() -> Result<String, String> {
    cadence::infrastructure::project_recovery_write(state())
}

pub async fn project_recovery_load() -> Result<ProjectDto, String> {
    lift(cadence::infrastructure::project_recovery_load(state()))
}

pub async fn project_recovery_clear() -> Result<String, String> {
    cadence::infrastructure::project_recovery_clear(state())
}

pub async fn app_quit() -> Result<(), String> {
    std::process::exit(0);
}

pub async fn window_close_main() -> Result<(), String> {
    std::process::exit(0);
}

pub async fn graph_snapshot(target: CadenceGraphTarget) -> Result<GraphSnapshotDto, String> {
    let target = convert::<_, cadence::infrastructure::dto::CadenceGraphTarget>(target)?;
    lift(cadence::infrastructure::graph_snapshot(
        state(),
        Some(target),
    ))
}

pub async fn graph_piece_catalog(target: CadenceGraphTarget) -> Result<Vec<PieceDef>, String> {
    let target = convert::<_, cadence::infrastructure::dto::CadenceGraphTarget>(target)?;
    lift(cadence::infrastructure::graph_piece_catalog(
        state(),
        Some(target),
    ))
}

pub async fn graph_compile_preview(
    target: CadenceGraphTarget,
) -> Result<GraphCompilePreviewDto, String> {
    let target = convert::<_, cadence::infrastructure::dto::CadenceGraphTarget>(target)?;
    lift(cadence::infrastructure::graph_compile_preview(
        state(),
        Some(target),
    ))
}

pub async fn editor_sync(
    target: CadenceGraphTarget,
    include_sample_library: bool,
) -> Result<EditorSyncSnapshotDto, String> {
    let target = convert::<_, cadence::infrastructure::dto::CadenceGraphTarget>(target)?;
    lift(cadence::infrastructure::editor_sync(
        state(),
        target,
        include_sample_library,
    ))
}

pub async fn graph_apply_ops(
    ops: Vec<GraphOp>,
    request_id: Option<String>,
    target: CadenceGraphTarget,
) -> Result<GraphApplyResultDto, String> {
    let ops = convert::<_, Vec<cadence::infrastructure::dto::GraphOp>>(ops)?;
    let target = convert::<_, cadence::infrastructure::dto::CadenceGraphTarget>(target)?;
    match cadence::infrastructure::apply_graph_ops(state(), ops, request_id, target) {
        Ok(value) => convert(value),
        Err(error) => Err(stringify_error(error)),
    }
}

pub async fn graph_pick_target_param(
    from: GridPos,
    to_node: GridPos,
    target: CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<GraphPickTargetParamDto, String> {
    let from = convert::<_, cadence::infrastructure::dto::GridPos>(from)?;
    let to_node = convert::<_, cadence::infrastructure::dto::GridPos>(to_node)?;
    let target = convert::<_, cadence::infrastructure::dto::CadenceGraphTarget>(target)?;
    lift(cadence::infrastructure::graph_pick_target_param(
        state(),
        from,
        to_node,
        target,
        to_param,
    ))
}

pub async fn graph_pick_target_param_on_graph(
    graph: GraphSnapshotDto,
    from: GridPos,
    to_node: GridPos,
    target: CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<GraphPickTargetParamDto, String> {
    let graph = convert::<_, cadence::infrastructure::dto::GraphSnapshotDto>(graph)?;
    let from = convert::<_, cadence::infrastructure::dto::GridPos>(from)?;
    let to_node = convert::<_, cadence::infrastructure::dto::GridPos>(to_node)?;
    let target = convert::<_, cadence::infrastructure::dto::CadenceGraphTarget>(target)?;
    lift(cadence::infrastructure::graph_pick_target_param_on_graph(
        state(),
        graph,
        from,
        to_node,
        target,
        to_param,
    ))
}

pub async fn runtime_commit(args: RuntimeCommitArgs) -> Result<RuntimeCommitDto, String> {
    lift(cadence::infrastructure::runtime_commit(
        state(),
        args.cpm,
        args.force,
        args.playing,
        args.request_id,
    ))
}

pub async fn runtime_status() -> Result<RuntimeStatusDto, String> {
    lift(cadence::infrastructure::runtime_status(state()))
}

pub async fn runtime_stop() -> Result<RuntimeStatusDto, String> {
    lift(cadence::infrastructure::runtime_stop(state()))
}

pub async fn history_status() -> Result<HistoryStatusDto, String> {
    lift(cadence::infrastructure::history_status(state()))
}

pub async fn history_undo() -> Result<HistoryStatusDto, String> {
    lift(cadence::infrastructure::history_undo(state()))
}

pub async fn history_redo() -> Result<HistoryStatusDto, String> {
    lift(cadence::infrastructure::history_redo(state()))
}

pub async fn diagnostics_snapshot() -> Result<DiagnosticsSnapshotDto, String> {
    lift(cadence::infrastructure::diagnostics_snapshot(state()))
}

pub async fn export_pick_song_path() -> Result<Option<String>, String> {
    cadence::infrastructure::export_pick_song_path(state()).await
}

pub async fn export_song(path: String, cpm: Option<f32>) -> Result<ExportSongResultDto, String> {
    lift(cadence::infrastructure::export_song(state(), path, cpm))
}

pub async fn ui_set_mini_console_visible(visible: bool) -> Result<(), String> {
    cadence::infrastructure::ui_set_mini_console_visible(state(), visible)
}

pub async fn ui_set_devtools_visible(visible: bool) -> Result<(), String> {
    cadence::infrastructure::ui_set_devtools_visible(state(), visible)
}
