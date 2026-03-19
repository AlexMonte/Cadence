//! Cadence's Strudel-specific implementation layer built on top of `tessera`.

pub mod cadence_program;
pub mod compile_support;
pub mod graph_defaults;
pub mod host_adapter;
pub mod piece_registry;
pub mod pieces;
pub mod project_compile;
pub mod selection;
pub mod strudel_schema;
pub mod terminal_strategy;

#[cfg(test)]
mod grid_tests;
