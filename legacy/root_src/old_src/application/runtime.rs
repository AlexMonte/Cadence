//! Runtime orchestration use-cases.

mod ops;

use crate::{adapter::transport, infrastructure::dto};

pub fn commit_runtime(
    state: &dto::SharedAppState,
    cpm: Option<f32>,
    force: Option<bool>,
    playing: Option<bool>,
    request_id: Option<u64>,
) -> Result<dto::RuntimeCommitDto, String> {
    let commit = ops::runtime_commit(state, cpm, force, playing, request_id)?;
    transport::translate(commit)
}

pub fn runtime_status(state: &dto::SharedAppState) -> Result<dto::RuntimeStatusDto, String> {
    transport::translate(ops::runtime_status(state)?)
}

pub fn stop_runtime(state: &dto::SharedAppState) -> Result<dto::RuntimeStatusDto, String> {
    transport::translate(ops::runtime_stop(state)?)
}
