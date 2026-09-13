#[cfg(not(target_arch = "wasm32"))]
mod atomic;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use atomic::atomic_write as write_preferences;
mod editor_io;
#[cfg(not(target_arch = "wasm32"))]
pub mod recovery;
mod sample_bank_import;
mod samples;
mod wav;

pub use sample_bank_import::{
    PreparedSampleBank, PreparedSampleImport, adopt_prepared_sample_bank, adopt_sample_import,
    prepare_sample_bank, prepare_sample_import,
};
pub use samples::{
    PreparedSample, adopt_prepared_sample, import_wav_bytes, import_wav_sample, prepare_wav_bytes,
    prepare_wav_sample, relink_prepared_sample, restore_sample_snapshot, set_sample_bank,
    set_sample_options, snapshot_sample,
};

#[cfg(not(target_arch = "wasm32"))]
mod native_recent;

#[cfg(target_arch = "wasm32")]
pub mod wasm_io;

pub use editor_io::{
    EditorOpenResult, SavedProjectPath, default_project_filename, editor_open_project,
    editor_save_project, save_project_to_path,
};

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::application::session::MusaicProject;
use crate::domain::document::MusaicDocument;
use crate::domain::project::ProjectMetadata;
use crate::domain::project::samples::SampleManifest;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    metadata: ProjectMetadata,
    document: MusaicDocument,
    samples: SampleManifest,
    sound_library: crate::domain::instrument::SoundLibrary,
}

fn validate_sound_references(
    document: &MusaicDocument,
    samples: &SampleManifest,
) -> Result<(), PersistenceError> {
    for node in document.graph.nodes() {
        let crate::domain::document::DocumentNodeKind::Sound(sound) = &node.kind else {
            continue;
        };
        if let crate::domain::instrument::InstrumentSource::Sample(sample) = sound.definition.source
            && !samples.samples.contains_key(&sample)
        {
            return Err(PersistenceError::InvalidProject(format!(
                "Sound {} refers to a sample absent from this project",
                node.id.0
            )));
        }
    }
    Ok(())
}

pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        recovery::register(_app);
    }
}

/// Export the document and manifest, not an archive of the WAV files.
/// Use `save_project` to save a complete native project folder.
pub fn export_project_bytes(project: &MusaicProject) -> Result<Vec<u8>, PersistenceError> {
    project
        .document
        .validate()
        .map_err(PersistenceError::InvalidProject)?;
    crate::application::outputs::validate(&project.document)
        .map_err(PersistenceError::InvalidProject)?;
    crate::application::tricks::validate(&project.document)
        .map_err(PersistenceError::InvalidProject)?;
    samples::validate_manifest(project.samples.manifest())?;
    crate::domain::instrument::validate_sound_library(
        &project.sound_library,
        project.samples.manifest(),
    )
    .map_err(PersistenceError::InvalidProject)?;
    validate_sound_references(&project.document, project.samples.manifest())?;
    let mut metadata = project.metadata.clone();
    metadata.file_path = None;
    metadata.dirty = false;
    let file = ProjectFile {
        metadata,
        document: project.document.clone(),
        samples: project.samples.manifest().clone(),
        sound_library: project.sound_library.clone(),
    };
    serde_json::to_vec_pretty(&file).map_err(PersistenceError::Json)
}

pub fn import_project_bytes(
    bytes: &[u8],
    file_label: Option<String>,
) -> Result<MusaicProject, PersistenceError> {
    let file: ProjectFile = serde_json::from_slice(bytes).map_err(PersistenceError::Json)?;
    samples::validate_manifest(&file.samples)?;
    crate::domain::instrument::validate_sound_library(&file.sound_library, &file.samples)
        .map_err(PersistenceError::InvalidProject)?;
    validate_sound_references(&file.document, &file.samples)?;
    file.document
        .validate()
        .map_err(PersistenceError::InvalidProject)?;
    let mut metadata = file.metadata;
    metadata.file_path = file_label;
    metadata.dirty = false;
    let mut project = MusaicProject {
        document: file.document,
        metadata,
        samples: samples::from_manifest(file.samples),
        sound_library: file.sound_library,
    };
    crate::application::outputs::validate(&project.document)
        .map_err(PersistenceError::InvalidProject)?;
    crate::application::tricks::validate(&project.document)
        .map_err(PersistenceError::InvalidProject)?;
    samples::diagnose_missing_sample_bindings(&mut project);
    Ok(project)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_project(path: impl AsRef<Path>) -> Result<MusaicProject, PersistenceError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(PersistenceError::Io)?;
    let mut project = import_project_bytes(&bytes, Some(path.display().to_string()))?;
    project.metadata.file_path = Some(path.display().to_string());
    samples::resolve_project_samples(&mut project.samples, path)?;
    samples::diagnose_missing_sample_bindings(&mut project);
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
    std::fs::create_dir_all(samples::project_directory(path))?;
    samples::save_project_samples(project, path)?;
    atomic::atomic_write(path, &json)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn save_project(
    _path: impl AsRef<Path>,
    project: &MusaicProject,
) -> Result<(), PersistenceError> {
    wasm_io::download_project(project, &default_project_filename())
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
    #[error("{0}")]
    Sample(String),
    #[error("Invalid project: {0}")]
    InvalidProject(String),
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_audio_export(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    atomic::atomic_write(path, bytes)
}

pub(crate) mod file_location;
