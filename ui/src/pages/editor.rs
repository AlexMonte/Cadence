use crate::state::project_state::{ProjectCreateStatus, ProjectState};

pub fn editor_page_title() -> &'static str {
    "Cadence Editor"
}

pub fn render_editor_shell(state: &ProjectState) -> String {
    let status = match state.status {
        ProjectCreateStatus::Idle => "idle",
        ProjectCreateStatus::Loading => "loading",
        ProjectCreateStatus::Success => "success",
        ProjectCreateStatus::Error => "error",
    };

    let mut out = format!("editor_shell status={status}");
    if let Some(project) = &state.project {
        out.push_str(&format!(
            " project='{}' nodes={} edges={}",
            project.name, project.node_count, project.edge_count
        ));
    }

    if let Some(err) = &state.last_error {
        out.push_str(&format!(" error='{}'", err));
    }

    out
}
