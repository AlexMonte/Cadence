use crate::bridge::tauri::ProjectDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectCreateStatus {
    #[default]
    Idle,
    Loading,
    Success,
    Error,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectState {
    pub status: ProjectCreateStatus,
    pub last_error: Option<String>,
    pub project: Option<ProjectDto>,
}

impl ProjectState {
    pub fn begin_create(&mut self) -> bool {
        if matches!(self.status, ProjectCreateStatus::Loading) {
            return false;
        }
        self.status = ProjectCreateStatus::Loading;
        self.last_error = None;
        true
    }

    pub fn set_created_project(&mut self, project: ProjectDto) {
        self.project = Some(project);
        self.status = ProjectCreateStatus::Success;
        self.last_error = None;
    }

    pub fn set_error(&mut self, message: String) {
        self.status = ProjectCreateStatus::Error;
        self.last_error = Some(message);
    }
}
