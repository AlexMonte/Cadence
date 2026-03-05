use crate::bridge::tauri::{self, ProjectDto};
use crate::pages::editor::render_editor_shell;
use crate::state::project_state::ProjectState;

pub fn app_route() -> &'static str {
    "/editor"
}

pub fn apply_project_create_result(state: &mut ProjectState, result: Result<ProjectDto, String>) {
    match result {
        Ok(project) => state.set_created_project(project),
        Err(err) => state.set_error(err),
    }
}

pub async fn create_project_and_render(project_name: &str) -> String {
    let mut state = ProjectState::default();
    if !state.begin_create() {
        return render_editor_shell(&state);
    }
    let result = tauri::project_create(project_name.to_string()).await;
    apply_project_create_result(&mut state, result);
    render_editor_shell(&state)
}

pub fn render_default_shell() -> String {
    render_editor_shell(&ProjectState::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_project_create_result_renders_success_payload() {
        let mut state = ProjectState::default();
        assert!(state.begin_create());
        apply_project_create_result(
            &mut state,
            Ok(ProjectDto {
                name: "My Song".to_string(),
                node_count: 1,
                edge_count: 0,
            }),
        );

        let rendered = render_editor_shell(&state);
        assert!(rendered.contains("status=success"));
        assert!(rendered.contains("project='My Song'"));
    }

    #[test]
    fn begin_create_is_guarded_while_loading() {
        let mut state = ProjectState::default();
        assert!(state.begin_create());
        assert!(!state.begin_create());
    }
}
