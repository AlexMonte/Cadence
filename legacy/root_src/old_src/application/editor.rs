//! Canonical editor hydration snapshot assembled for the frontend shell.

mod bevy;

use crate::infrastructure::dto::{self, EditorSyncSnapshotDto};
use crate::{application, infrastructure::ui};

pub use bevy::{
    EditorPlugin, EditorSnapshot,
};

pub fn editor_sync(
    state: &dto::SharedAppState,
    target: dto::CadenceGraphTarget,
    include_sample_library: bool,
) -> Result<EditorSyncSnapshotDto, String> {
    let project = application::project::bootstrap(state)?;
    let init_stage = application::project::init_snapshot(state)?;
    let graph = application::authoring::graph_snapshot(state, Some(target.clone()))?;
    let catalog = application::authoring::graph_piece_catalog(state, Some(target.clone()))?;
    let graph_preview = application::authoring::compile_graph_preview(state, Some(target))?;
    let sample_library = if include_sample_library {
        Some(application::project::sample_library(state)?)
    } else {
        None
    };
    let project_preview = application::authoring::compile_project_preview(state)?;
    let history_status =
        crate::adapter::transport::translate(application::history::history_status(state)?)?;
    let runtime_status = application::runtime::runtime_status(state)?;
    let diagnostics = crate::adapter::transport::translate(ui::diagnostics_snapshot(state)?)?;
    let recovery_path = application::project::recovery_status(state)?.path;

    Ok(EditorSyncSnapshotDto {
        project,
        init_stage,
        graph,
        catalog,
        graph_preview,
        project_preview,
        history_status,
        runtime_status,
        diagnostics,
        recovery_path,
        sample_library,
    })
}
