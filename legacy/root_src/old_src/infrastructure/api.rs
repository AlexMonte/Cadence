//! Stable UX-facing backend API.

use crate::{
    application,
    infrastructure::{dto, ui},
};

pub fn graph_snapshot(
    state: &dto::SharedAppState,
    target: Option<dto::CadenceGraphTarget>,
) -> Result<dto::GraphSnapshotDto, String> {
    application::authoring::graph_snapshot(state, target)
}

pub fn graph_piece_catalog(
    state: &dto::SharedAppState,
    target: Option<dto::CadenceGraphTarget>,
) -> Result<Vec<dto::PieceDef>, String> {
    application::authoring::graph_piece_catalog(state, target)
}

pub fn graph_compile_preview(
    state: &dto::SharedAppState,
    target: Option<dto::CadenceGraphTarget>,
) -> Result<dto::GraphCompilePreviewDto, String> {
    application::authoring::compile_graph_preview(state, target)
}

pub fn editor_sync(
    state: &dto::SharedAppState,
    target: dto::CadenceGraphTarget,
    include_sample_library: bool,
) -> Result<dto::EditorSyncSnapshotDto, String> {
    application::editor::editor_sync(state, target, include_sample_library)
}

pub fn project_compile_preview(
    state: &dto::SharedAppState,
) -> Result<dto::ProjectCompilePreviewDto, String> {
    application::authoring::compile_project_preview(state)
}

pub fn graph_pick_target_param(
    state: &dto::SharedAppState,
    from: dto::GridPos,
    to_node: dto::GridPos,
    target: dto::CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<dto::GraphPickTargetParamDto, String> {
    application::authoring::graph_pick_target_param(state, from, to_node, target, to_param)
}

pub fn graph_pick_target_param_on_graph(
    state: &dto::SharedAppState,
    graph: dto::GraphSnapshotDto,
    from: dto::GridPos,
    to_node: dto::GridPos,
    target: dto::CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<dto::GraphPickTargetParamDto, String> {
    application::authoring::graph_pick_target_param_on_graph(
        state, graph, from, to_node, target, to_param,
    )
}

pub fn apply_graph_ops(
    state: &dto::SharedAppState,
    ops: Vec<dto::GraphOp>,
    request_id: Option<String>,
    target: dto::CadenceGraphTarget,
) -> Result<dto::GraphApplyResultDto, Vec<dto::DiagnosticDto>> {
    application::authoring::apply_graph_ops(state, ops, request_id, target)
}

pub fn project_create(
    state: &dto::SharedAppState,
    name: String,
) -> Result<dto::ProjectDto, String> {
    application::project::create(state, name)
}

pub fn project_new(
    state: &dto::SharedAppState,
    name: Option<String>,
) -> Result<dto::ProjectDto, String> {
    application::project::new_project(state, name)
}

pub fn project_rename(
    state: &dto::SharedAppState,
    name: String,
) -> Result<dto::ProjectViewDto, String> {
    application::project::rename(state, name)
}

pub fn project_open_path(
    state: &dto::SharedAppState,
    path: String,
) -> Result<dto::ProjectDto, String> {
    application::project::open_path(state, path)
}

pub fn project_open(state: &dto::SharedAppState, path: String) -> Result<dto::ProjectDto, String> {
    application::project::open(state, path)
}

pub fn project_save(state: &dto::SharedAppState, path: Option<String>) -> Result<String, String> {
    application::project::save(state, path)
}

pub fn project_save_current(state: &dto::SharedAppState) -> Result<String, String> {
    application::project::save_current(state)
}

pub fn project_save_as(state: &dto::SharedAppState, path: String) -> Result<String, String> {
    application::project::save_as(state, path)
}

pub fn project_dirty_status(
    state: &dto::SharedAppState,
) -> Result<dto::ProjectDirtyStatusDto, String> {
    application::project::dirty_status(state)
}

pub fn project_snapshot(state: &dto::SharedAppState) -> Result<dto::ProjectViewDto, String> {
    application::project::snapshot(state)
}

pub fn project_bootstrap(state: &dto::SharedAppState) -> Result<dto::ProjectViewDto, String> {
    application::project::bootstrap(state)
}

pub fn project_init_snapshot(
    state: &dto::SharedAppState,
) -> Result<dto::InitStageSnapshotDto, String> {
    application::project::init_snapshot(state)
}

pub fn project_sample_library(
    state: &dto::SharedAppState,
) -> Result<dto::SampleLibrarySnapshotDto, String> {
    application::project::sample_library(state)
}

pub fn project_init_apply(
    state: &dto::SharedAppState,
    ops: Vec<dto::InitStageOp>,
) -> Result<dto::InitStageSnapshotDto, String> {
    application::project::apply_init_stage(state, ops)
}

pub async fn project_pick_open_path(state: &dto::SharedAppState) -> Result<Option<String>, String> {
    application::project::pick_open_path(state).await
}

pub async fn project_pick_save_path(state: &dto::SharedAppState) -> Result<Option<String>, String> {
    application::project::pick_save_path(state).await
}

pub fn project_recovery_status(
    state: &dto::SharedAppState,
) -> Result<dto::ProjectPathChoiceDto, String> {
    application::project::recovery_status(state)
}

pub fn project_recovery_write(state: &dto::SharedAppState) -> Result<String, String> {
    application::project::recovery_write(state)
}

pub fn project_recovery_load(state: &dto::SharedAppState) -> Result<dto::ProjectDto, String> {
    application::project::recovery_load(state)
}

pub fn project_recovery_clear(state: &dto::SharedAppState) -> Result<String, String> {
    application::project::recovery_clear(state)
}

pub fn runtime_commit(
    state: &dto::SharedAppState,
    cpm: Option<f32>,
    force: Option<bool>,
    playing: Option<bool>,
    request_id: Option<u64>,
) -> Result<dto::RuntimeCommitDto, String> {
    application::runtime::commit_runtime(state, cpm, force, playing, request_id)
}

pub fn runtime_status(state: &dto::SharedAppState) -> Result<dto::RuntimeStatusDto, String> {
    application::runtime::runtime_status(state)
}

pub fn runtime_stop(state: &dto::SharedAppState) -> Result<dto::RuntimeStatusDto, String> {
    application::runtime::stop_runtime(state)
}

pub fn history_status(state: &dto::SharedAppState) -> Result<dto::HistoryStatusDto, String> {
    crate::adapter::transport::translate(application::history::history_status(state)?)
}

pub fn history_undo(state: &dto::SharedAppState) -> Result<dto::HistoryStatusDto, String> {
    crate::adapter::transport::translate(application::history::history_undo(state)?)
}

pub fn history_redo(state: &dto::SharedAppState) -> Result<dto::HistoryStatusDto, String> {
    crate::adapter::transport::translate(application::history::history_redo(state)?)
}

pub fn diagnostics_snapshot(
    state: &dto::SharedAppState,
) -> Result<dto::DiagnosticsSnapshotDto, String> {
    crate::adapter::transport::translate(ui::diagnostics_snapshot(state)?)
}

pub async fn export_pick_song_path(state: &dto::SharedAppState) -> Result<Option<String>, String> {
    application::export::export_pick_song_path(state).await
}

pub fn export_song(
    state: &dto::SharedAppState,
    path: String,
    cpm: Option<f32>,
) -> Result<dto::ExportSongResultDto, String> {
    crate::adapter::transport::translate(application::export::export_song(state, path, cpm)?)
}

pub fn ui_set_mini_console_visible(
    state: &dto::SharedAppState,
    visible: bool,
) -> Result<(), String> {
    ui::ui_set_mini_console_visible(state, visible)
}

pub fn ui_set_devtools_visible(state: &dto::SharedAppState, visible: bool) -> Result<(), String> {
    ui::ui_set_devtools_visible(state, visible)
}
