//! Cadence's Strudel-specific implementation layer built on top of `tile_graph`.

pub mod cadence_program;
pub mod piece_registry;
pub mod pieces;
pub mod project_compile;
pub mod selection;
pub mod strudel_schema;
pub mod terminal_strategy;
pub mod tricks;

#[cfg(test)]
mod grid_tests;
