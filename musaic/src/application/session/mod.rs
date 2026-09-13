mod demo;
mod first_loop;
mod samples;

pub use first_loop::{FirstLoop, first_loop};

pub(crate) use samples::LoadedProjectSample;
pub use samples::{ProjectSamples, SampleAssetSnapshot};

use bevy::prelude::*;

use crate::domain::document::MusaicDocument;
use crate::domain::project::ProjectMetadata;

/// Document authority: graph + file metadata.
///
/// **Single writer:** [`crate::application::command::bus`] `dispatch_commands`.
/// Pipeline stages and UI may read; they must not `ResMut` this resource.
#[derive(Resource, Debug, Clone)]
pub struct MusaicProject {
    pub document: MusaicDocument,
    pub metadata: ProjectMetadata,
    pub samples: ProjectSamples,
    pub sound_library: crate::domain::instrument::SoundLibrary,
}

impl Default for MusaicProject {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl MusaicProject {
    pub fn new_empty() -> Self {
        Self {
            document: MusaicDocument::new_empty(),
            sound_library: Default::default(),
            metadata: ProjectMetadata {
                display_name: "Untitled".into(),
                ..default()
            },
            samples: ProjectSamples::default(),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.metadata.dirty = true;
    }

    pub fn sound_definition(
        &self,
        node: &tessera::prelude::NodeId,
    ) -> Option<&crate::domain::instrument::InstrumentDefinition> {
        self.document.graph.sound_definition(node)
    }

    pub fn set_sound_definition(
        &mut self,
        node: &tessera::prelude::NodeId,
        definition: crate::domain::instrument::InstrumentDefinition,
    ) -> Result<crate::domain::instrument::InstrumentDefinition, String> {
        if let crate::domain::instrument::InstrumentSource::Sample(sample) = definition.source
            && !self.samples.manifest().samples.contains_key(&sample)
        {
            return Err("That sample is not in this project".into());
        }
        self.document.graph.set_sound_definition(node, definition)
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ProjectSession {
    pub recent_paths: Vec<String>,
    pub pending_open: Option<std::path::PathBuf>,
    /// Last path used for save (native) or download filename stem (wasm).
    pub last_saved_path: Option<std::path::PathBuf>,
    // Future: `default_save_directory` / filename from a settings panel.
}

impl ProjectSession {
    /// Session with the recent-projects list restored from disk.
    pub fn restored() -> Self {
        Self {
            recent_paths: crate::adapter::persistence::load_recent_projects()
                .into_iter()
                .map(|p| p.display().to_string())
                .collect(),
            ..Self::default()
        }
    }

    pub fn open_path(&mut self, path: impl Into<std::path::PathBuf>) {
        self.pending_open = Some(path.into());
    }
}
