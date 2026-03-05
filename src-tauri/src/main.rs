mod commands;
mod core;
mod errors;
mod store;

use commands::{
    SharedAppState, diagnostics_snapshot, export_song, graph_apply_ops, graph_compile_preview,
    graph_piece_catalog, graph_snapshot, history_redo, history_status, history_undo,
    project_create, project_dirty_status, project_new, project_open, project_open_path,
    project_pick_open_path, project_pick_save_path, project_prompt_unsaved, project_recovery_clear,
    project_recovery_load, project_recovery_status, project_recovery_write, project_save,
    project_save_as, project_save_current, project_snapshot, runtime_commit,
    runtime_reset_on_project_swap, runtime_status, runtime_stop, ui_set_devtools_visible,
    ui_set_mini_console_visible,
};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

const MAIN_WINDOW_LABEL: &str = "main";
const DEV_INSPECTOR_LABEL: &str = "dev-inspector";
const UI_MENU_EVENT: &str = "ga://menu";

const MENU_FILE_NEW: &str = "file.new";
const MENU_FILE_OPEN: &str = "file.open";
const MENU_FILE_SAVE: &str = "file.save";
const MENU_FILE_SAVE_AS: &str = "file.save_as";
const MENU_FILE_EXPORT: &str = "file.export_song";
const MENU_FILE_QUIT: &str = "file.quit";

const MENU_EDIT_UNDO: &str = "edit.undo";
const MENU_EDIT_REDO: &str = "edit.redo";

const MENU_VIEW_TOGGLE_CONSOLE: &str = "view.toggle_mini_console";
const MENU_VIEW_TOGGLE_DEV_INSPECTOR: &str = "view.toggle_dev_inspector";

#[derive(Debug, Clone, Serialize)]
struct MenuActionPayload {
    action: String,
}

fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let file_new = MenuItem::with_id(app, MENU_FILE_NEW, "New", true, Some("CmdOrCtrl+N"))?;
    let file_open = MenuItem::with_id(app, MENU_FILE_OPEN, "Open...", true, Some("CmdOrCtrl+O"))?;
    let file_save = MenuItem::with_id(app, MENU_FILE_SAVE, "Save", true, Some("CmdOrCtrl+S"))?;
    let file_save_as = MenuItem::with_id(
        app,
        MENU_FILE_SAVE_AS,
        "Save As...",
        true,
        Some("CmdOrCtrl+Shift+S"),
    )?;
    let file_export = MenuItem::with_id(
        app,
        MENU_FILE_EXPORT,
        "Export Song...",
        true,
        Some("CmdOrCtrl+E"),
    )?;
    let file_quit = MenuItem::with_id(app, MENU_FILE_QUIT, "Quit", true, Some("CmdOrCtrl+Q"))?;

    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &file_new,
            &file_open,
            &file_save,
            &file_save_as,
            &PredefinedMenuItem::separator(app)?,
            &file_export,
            &file_quit,
        ],
    )?;

    let edit_undo = MenuItem::with_id(app, MENU_EDIT_UNDO, "Undo", true, Some("CmdOrCtrl+Z"))?;
    let edit_redo =
        MenuItem::with_id(app, MENU_EDIT_REDO, "Redo", true, Some("CmdOrCtrl+Shift+Z"))?;
    let edit_cut = PredefinedMenuItem::cut(app, None)?;
    let edit_copy = PredefinedMenuItem::copy(app, None)?;
    let edit_paste = PredefinedMenuItem::paste(app, None)?;
    let edit_select_all = PredefinedMenuItem::select_all(app, None)?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &edit_undo,
            &edit_redo,
            &PredefinedMenuItem::separator(app)?,
            &edit_cut,
            &edit_copy,
            &edit_paste,
            &edit_select_all,
        ],
    )?;

    let view_toggle_console = MenuItem::with_id(
        app,
        MENU_VIEW_TOGGLE_CONSOLE,
        "Toggle Mini Console",
        true,
        None::<&str>,
    )?;
    let view_toggle_dev = MenuItem::with_id(
        app,
        MENU_VIEW_TOGGLE_DEV_INSPECTOR,
        "Toggle Dev Inspector",
        true,
        None::<&str>,
    )?;
    let view_menu =
        Submenu::with_items(app, "View", true, &[&view_toggle_console, &view_toggle_dev])?;

    #[cfg(target_os = "macos")]
    {
        let app_submenu = Submenu::with_items(
            app,
            "GrooveAtlas",
            true,
            &[
                &PredefinedMenuItem::about(app, None, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::services(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::show_all(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::quit(app, None)?,
            ],
        )?;

        return Menu::with_items(app, &[&app_submenu, &file_menu, &edit_menu, &view_menu]);
    }

    #[cfg(not(target_os = "macos"))]
    {
        Menu::with_items(app, &[&file_menu, &edit_menu, &view_menu])
    }
}

fn emit_menu_action<R: Runtime>(app: &AppHandle<R>, action: &str) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let action_json =
            serde_json::to_string(action).unwrap_or_else(|_| "\"menu_action_error\"".to_string());
        let _ = window.emit(
            UI_MENU_EVENT,
            MenuActionPayload {
                action: action.to_string(),
            },
        );
        let _ = window.eval(format!("window.__GA_MENU_ACTION?.({action_json});").as_str());
    }
}

fn set_devtools_visible<R: Runtime>(app: &AppHandle<R>, visible: bool) {
    if let Ok(mut store) = app.state::<SharedAppState>().store.lock() {
        store.devtools_visible = visible;
        store.push_diagnostic("menu_dev_inspector", format!("visible={visible}"));
    }
}

fn toggle_dev_inspector_window<R: Runtime>(app: &AppHandle<R>) {
    if !cfg!(debug_assertions) {
        emit_menu_action(app, "view.toggle_dev_inspector_blocked");
        return;
    }

    if let Some(window) = app.get_webview_window(DEV_INSPECTOR_LABEL) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            set_devtools_visible(app, false);
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            set_devtools_visible(app, true);
        }
        return;
    }

    let created = WebviewWindowBuilder::new(
        app,
        DEV_INSPECTOR_LABEL,
        WebviewUrl::App("devtools.html".into()),
    )
    .title("GrooveAtlas Dev Inspector")
    .resizable(true)
    .inner_size(920.0, 620.0)
    .min_inner_size(680.0, 420.0)
    .build();

    if let Ok(window) = created {
        let _ = window.set_focus();
        set_devtools_visible(app, true);
    }
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, menu_id: &str) {
    match menu_id {
        MENU_FILE_NEW => emit_menu_action(app, "file.new"),
        MENU_FILE_OPEN => emit_menu_action(app, "file.open"),
        MENU_FILE_SAVE => emit_menu_action(app, "file.save"),
        MENU_FILE_SAVE_AS => emit_menu_action(app, "file.save_as"),
        MENU_FILE_EXPORT => emit_menu_action(app, "file.export_song"),
        MENU_FILE_QUIT => app.exit(0),
        MENU_EDIT_UNDO => emit_menu_action(app, "edit.undo"),
        MENU_EDIT_REDO => emit_menu_action(app, "edit.redo"),
        MENU_VIEW_TOGGLE_CONSOLE => emit_menu_action(app, "view.toggle_mini_console"),
        MENU_VIEW_TOGGLE_DEV_INSPECTOR => {
            toggle_dev_inspector_window(app);
            emit_menu_action(app, "view.toggle_dev_inspector");
        }
        _ => {}
    }
}

fn main() {
    eprintln!("starting GrooveAtlas Tauri shell");

    let app = tauri::Builder::default()
        .menu(app_menu)
        .on_menu_event(|app, event| {
            handle_menu_event(app, event.id().as_ref());
        })
        .manage(SharedAppState::default())
        .invoke_handler(tauri::generate_handler![
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
            project_open,
            project_save,
            project_snapshot,
            graph_snapshot,
            graph_piece_catalog,
            graph_compile_preview,
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
            export_song,
        ]);

    if let Err(err) = app.run(tauri::generate_context!()) {
        eprintln!("failed to run GrooveAtlas Tauri shell: {err}");
        std::process::exit(1);
    }
}
