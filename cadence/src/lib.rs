pub mod commands;
pub mod core;
pub mod errors;
pub mod model;
pub mod store;

use crate::commands::{
    app_quit, diagnostics_snapshot, export_pick_song_path, export_song, graph_apply_ops,
    graph_compile_preview, graph_pick_target_param, graph_piece_catalog, graph_snapshot,
    history_redo, history_status, history_undo, project_bootstrap, project_compile_preview,
    project_create, project_dirty_status, project_init_apply, project_init_snapshot, project_new,
    project_open, project_open_path, project_pick_open_path, project_pick_save_path,
    project_prompt_unsaved, project_recovery_clear, project_recovery_load, project_recovery_status,
    project_recovery_write, project_rename, project_save, project_save_as, project_save_current,
    project_snapshot, runtime_commit, runtime_reset_on_project_swap, runtime_status, runtime_stop,
    ui_set_devtools_visible, ui_set_mini_console_visible, window_close_main,
};

pub fn desktop_invoke_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool {
    tauri::generate_handler![
        app_quit,
        window_close_main,
        project_new,
        project_open_path,
        project_save_current,
        project_save_as,
        project_dirty_status,
        project_recovery_status,
        project_recovery_write,
        project_recovery_load,
        project_recovery_clear,
        project_pick_open_path,
        project_pick_save_path,
        project_prompt_unsaved,
        project_create,
        project_rename,
        project_open,
        project_save,
        project_snapshot,
        project_bootstrap,
        project_init_snapshot,
        project_init_apply,
        graph_snapshot,
        graph_piece_catalog,
        graph_pick_target_param,
        graph_compile_preview,
        project_compile_preview,
        graph_apply_ops,
        runtime_commit,
        runtime_status,
        runtime_stop,
        runtime_reset_on_project_swap,
        history_status,
        history_undo,
        history_redo,
        diagnostics_snapshot,
        ui_set_mini_console_visible,
        ui_set_devtools_visible,
        export_pick_song_path,
        export_song,
    ]
}
