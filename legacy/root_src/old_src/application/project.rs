//! Project lifecycle and init-stage use-cases.

pub(crate) mod ops;

use crate::{
    adapter::{tessera, transport},
    infrastructure::dto,
};

pub fn create(state: &dto::SharedAppState, name: String) -> Result<dto::ProjectDto, String> {
    transport::translate(ops::project_create(state, name)?)
}

pub fn new_project(
    state: &dto::SharedAppState,
    name: Option<String>,
) -> Result<dto::ProjectDto, String> {
    transport::translate(ops::project_new(state, name)?)
}

pub fn rename(state: &dto::SharedAppState, name: String) -> Result<dto::ProjectViewDto, String> {
    transport::translate(ops::project_rename(state, name)?)
}

pub fn open_path(state: &dto::SharedAppState, path: String) -> Result<dto::ProjectDto, String> {
    transport::translate(ops::project_open_path(state, path)?)
}

pub fn open(state: &dto::SharedAppState, path: String) -> Result<dto::ProjectDto, String> {
    transport::translate(ops::project_open(state, path)?)
}

pub fn save(state: &dto::SharedAppState, path: Option<String>) -> Result<String, String> {
    ops::project_save(state, path)
}

pub fn save_current(state: &dto::SharedAppState) -> Result<String, String> {
    ops::project_save_current(state)
}

pub fn save_as(state: &dto::SharedAppState, path: String) -> Result<String, String> {
    ops::project_save_as(state, path)
}

pub fn dirty_status(state: &dto::SharedAppState) -> Result<dto::ProjectDirtyStatusDto, String> {
    transport::translate(ops::project_dirty_status(state)?)
}

pub fn snapshot(state: &dto::SharedAppState) -> Result<dto::ProjectViewDto, String> {
    transport::translate(ops::project_snapshot(state)?)
}

pub fn bootstrap(state: &dto::SharedAppState) -> Result<dto::ProjectViewDto, String> {
    transport::translate(ops::project_bootstrap(state)?)
}

pub fn init_snapshot(state: &dto::SharedAppState) -> Result<dto::InitStageSnapshotDto, String> {
    transport::translate(ops::project_init_snapshot(state)?)
}

pub fn sample_library(
    state: &dto::SharedAppState,
) -> Result<dto::SampleLibrarySnapshotDto, String> {
    transport::translate(ops::project_sample_library(state)?)
}

pub fn apply_init_stage(
    state: &dto::SharedAppState,
    ops: Vec<dto::InitStageOp>,
) -> Result<dto::InitStageSnapshotDto, String> {
    let ops = tessera::init_stage_ops_to_internal(ops)?;
    transport::translate(ops::project_init_apply(state, ops)?)
}

pub async fn pick_open_path(state: &dto::SharedAppState) -> Result<Option<String>, String> {
    ops::project_pick_open_path(state).await
}

pub async fn pick_save_path(state: &dto::SharedAppState) -> Result<Option<String>, String> {
    ops::project_pick_save_path(state).await
}

pub fn recovery_status(state: &dto::SharedAppState) -> Result<dto::ProjectPathChoiceDto, String> {
    transport::translate(ops::project_recovery_status(state)?)
}

pub fn recovery_write(state: &dto::SharedAppState) -> Result<String, String> {
    ops::project_recovery_write(state)
}

pub fn recovery_load(state: &dto::SharedAppState) -> Result<dto::ProjectDto, String> {
    transport::translate(ops::project_recovery_load(state)?)
}

pub fn recovery_clear(state: &dto::SharedAppState) -> Result<String, String> {
    ops::project_recovery_clear(state)
}
