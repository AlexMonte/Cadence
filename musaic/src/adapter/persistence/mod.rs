mod editor_io;

#[cfg(not(target_arch = "wasm32"))]
mod native_recent;

#[cfg(target_arch = "wasm32")]
pub mod wasm_io;

pub use editor_io::{
    SavedProjectPath, default_project_filename, editor_open_project, editor_save_project,
    save_project_to_path,
};

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::application::session::MusaicProject;
use crate::domain::document::{MusaicDocument, migrate_legacy_connections};
use crate::domain::project::ProjectMetadata;

pub const FORMAT_VERSION: u32 = 3;
pub const LEGACY_FORMAT_VERSION: u32 = 1;
const V2_FORMAT_VERSION: u32 = 2;

#[derive(Debug, Serialize, Deserialize)]
pub struct MusaicProjectFile {
    pub format_version: u32,
    pub metadata: ProjectMetadata,
    pub document: MusaicDocument,
}

pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, _app: &mut App) {}
}

pub fn export_project_bytes(project: &MusaicProject) -> Result<Vec<u8>, PersistenceError> {
    let file = MusaicProjectFile {
        format_version: FORMAT_VERSION,
        metadata: project.metadata.clone(),
        document: project.document.clone(),
    };
    serde_json::to_vec_pretty(&file).map_err(PersistenceError::Json)
}

pub fn import_project_bytes(
    bytes: &[u8],
    file_label: Option<String>,
) -> Result<MusaicProject, PersistenceError> {
    let file: MusaicProjectFile = serde_json::from_slice(bytes).map_err(PersistenceError::Json)?;
    if file.format_version != FORMAT_VERSION
        && file.format_version != LEGACY_FORMAT_VERSION
        && file.format_version != V2_FORMAT_VERSION
    {
        return Err(PersistenceError::UnsupportedVersion(file.format_version));
    }
    let mut document = file.document;
    migrate_legacy_connections(&mut document);
    let mut metadata = file.metadata;
    metadata.file_path = file_label;
    metadata.dirty = false;
    Ok(MusaicProject {
        document,
        metadata,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_project(path: impl AsRef<Path>) -> Result<MusaicProject, PersistenceError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(PersistenceError::Io)?;
    let mut project = import_project_bytes(&bytes, Some(path.display().to_string()))?;
    project.metadata.file_path = Some(path.display().to_string());
    Ok(project)
}

#[cfg(target_arch = "wasm32")]
pub fn load_project(path: impl AsRef<Path>) -> Result<MusaicProject, PersistenceError> {
    let _ = path;
    Err(PersistenceError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "use import_project_bytes on wasm",
    )))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_project(
    path: impl AsRef<Path>,
    project: &MusaicProject,
) -> Result<(), PersistenceError> {
    let path = path.as_ref();
    let json = export_project_bytes(project)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(PersistenceError::Io)?;
    }
    std::fs::write(path, json).map_err(PersistenceError::Io)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn save_project(
    _path: impl AsRef<Path>,
    project: &MusaicProject,
) -> Result<(), PersistenceError> {
    wasm_io::download_project(project, default_project_filename())
}

pub fn load_recent_projects() -> Vec<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native_recent::load_recent_projects()
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_io::load_recent_projects()
    }
}

pub fn push_recent_project(path: impl AsRef<Path>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native_recent::push_recent_project(path);
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_io::push_recent_project(path);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("unsupported project format version {0}")]
    UnsupportedVersion(u32),
}
