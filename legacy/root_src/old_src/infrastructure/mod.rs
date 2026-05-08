//! Public backend surface consumed by the frontend and contract tests.

pub mod app;
pub mod api;
pub mod board;
pub mod dto;
pub mod shell;
pub mod theme;
pub mod ui;

pub use api::*;

#[cfg(test)]
mod contract_tests;
