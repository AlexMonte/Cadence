//! Aggregated Tauri command modules exposed by the desktop shell.

mod app_commands;
#[cfg(test)]
mod contract_tests;
mod diagnostics;
mod export;
mod graph_commands;
mod history;
mod project_commands;
mod runtime_commands;

pub use app_commands::{app_quit, take_main_window_close_allowance, window_close_main};
pub use diagnostics::{diagnostics_snapshot, ui_set_devtools_visible, ui_set_mini_console_visible};
pub use export::{export_pick_song_path, export_song};
pub use graph_commands::{
    graph_apply_ops, graph_compile_preview, graph_pick_target_param, graph_piece_catalog,
    graph_snapshot, project_compile_preview,
};
pub use history::{history_redo, history_status, history_undo};
pub use project_commands::{
    SharedAppState, project_bootstrap, project_create, project_dirty_status, project_init_apply,
    project_init_snapshot, project_new, project_open, project_open_path, project_pick_open_path,
    project_pick_save_path, project_prompt_unsaved, project_recovery_clear, project_recovery_load,
    project_recovery_status, project_recovery_write, project_rename, project_save, project_save_as,
    project_save_current, project_snapshot,
};
pub use runtime_commands::{
    TerminalStrategy, runtime_commit, runtime_reset_on_project_swap, runtime_status, runtime_stop,
};
