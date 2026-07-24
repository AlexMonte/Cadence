//! Cross-platform save/open helpers (native dialogs vs wasm download/picker).
//!
//! Persistence adapters are pure I/O: they do not mutate project/session
//! resources. The command dispatcher applies success side-effects.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::application::session::MusaicProject;

use super::{PersistenceError, push_recent_project, save_project};

/// Default filename for new saves (user can override via save dialog or future settings).
pub fn default_project_filename() -> String {
    "document.musaic.json".to_string()
}

/// Open result: either a filesystem path (native) or an in-memory project (wasm).
#[derive(Debug, Clone)]
pub enum EditorOpenResult {
    Path(std::path::PathBuf),
    Project(MusaicProject),
}

/// Result of a successful save (path or download name). Cancelled picker → `None` Ok.
#[derive(Debug, Clone)]
pub struct SavedProjectPath {
    pub path: PathBuf,
}

/// Save using an already-chosen path (no picker).
pub fn save_project_to_path(
    project: &MusaicProject,
    path: &Path,
) -> Result<SavedProjectPath, PersistenceError> {
    save_project(path, project)?;
    push_recent_project(path);
    Ok(SavedProjectPath {
        path: path.to_path_buf(),
    })
}

/// Save the project; returns the path or download name used on success.
///
/// - If `explicit_path` is set, writes there (no picker).
/// - Else if `last_saved_path` is set and `force_picker` is false, writes there.
/// - Else opens the platform save UI (native dialog / wasm download name).
pub fn editor_save_project(
    project: &MusaicProject,
    last_saved_path: Option<&Path>,
    explicit_path: Option<PathBuf>,
    force_picker: bool,
) -> Result<Option<SavedProjectPath>, PersistenceError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = if let Some(path) = explicit_path {
            Some(path)
        } else if !force_picker {
            last_saved_path
                .map(|p| p.to_path_buf())
                .or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("Musaic project", &["json"])
                        .set_file_name(&default_project_filename())
                        .save_file()
                })
        } else {
            rfd::FileDialog::new()
                .add_filter("Musaic project", &["json"])
                .set_file_name(&default_project_filename())
                .save_file()
        };
        let Some(path) = path else {
            return Ok(None);
        };
        Ok(Some(save_project_to_path(project, &path)?))
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = force_picker;
        let name = explicit_path
            .as_ref()
            .map(|p| p.as_path())
            .or(last_saved_path)
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(default_project_filename);
        let path = explicit_path.unwrap_or_else(|| PathBuf::from(&name));
        super::wasm_io::download_project(project, &name)?;
        push_recent_project(&path);
        Ok(Some(SavedProjectPath { path }))
    }
}

/// Pick a project to open (native dialog or wasm file input).
pub fn editor_open_project() -> Option<EditorOpenResult> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rfd::FileDialog::new()
            .add_filter("Musaic project", &["json"])
            .pick_file()
            .map(EditorOpenResult::Path)
    }

    #[cfg(target_arch = "wasm32")]
    {
        super::wasm_io::request_open_project();
        None
    }
}
