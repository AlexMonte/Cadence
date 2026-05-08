//! Cadence-owned semantic models.
//!
//! This layer is the home for app concepts and meaning. During the refactor we
//! keep some legacy implementations in place, but expose the intended
//! vocabulary from here so other layers stop depending on ad hoc module names.

pub mod common;
pub mod board;
pub mod preview;
pub mod program;
pub mod project;
pub mod script;
pub mod selection;
