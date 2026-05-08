use super::EditorState;

pub fn set_runtime_panel_visibility(state: &mut EditorState, visible: bool) {
    state.diagnostics.mini_console_visible = visible;
    state.compile_open = visible;
    if visible {
        state.activity_tab = super::ActivityTab::Diagnostics;
    } else {
        state.status_message = None;
    }
}
