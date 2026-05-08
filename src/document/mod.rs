use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use tessera::prelude::{AuthoredTesseraProgram, NodeId};
use uuid::Uuid;

#[derive(Resource, Debug, Clone)]
pub struct CadenceProject {
    pub document: CadenceDocument,
    pub editor: crate::editor::EditorState,
    pub runtime: crate::runtime::RuntimeState,
}

impl Default for CadenceProject {
    fn default() -> Self {
        Self {
            document: CadenceDocument::default(),
            editor: crate::editor::EditorState::default(),
            runtime: crate::runtime::RuntimeState::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceDocument {
    pub id: DocumentId,
    pub name: String,
    pub tessera: TesseraDocumentState,
    pub routing: RoutingState,
    pub assets: AssetLibraryState,
    pub playback: PlaybackDefaults,
}

impl Default for CadenceDocument {
    fn default() -> Self {
        Self {
            id: DocumentId::new(),
            name: "Untitled".into(),
            tessera: TesseraDocumentState::default(),
            routing: RoutingState::default(),
            assets: AssetLibraryState::default(),
            playback: PlaybackDefaults::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentId(Uuid);

impl DocumentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TesseraDocumentState {
    pub authored_program: AuthoredTesseraProgram,
}

impl Default for TesseraDocumentState {
    fn default() -> Self {
        Self {
            authored_program: AuthoredTesseraProgram::empty(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingState {
    pub outputs: Vec<OutputRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputRoute {
    pub tessera_output: NodeId,
    pub target: PlaybackTarget,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum PlaybackTarget {
    #[default]
    Master,
    Bus(BusId),
    Track(TrackId),
    Preview,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BusId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrackId(pub String);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybackDefaults {
    pub bpm: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetLibraryState {
    pub samples: BTreeMap<SampleId, SampleAssetRef>,
    pub synths: BTreeMap<SynthId, SynthPreset>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SampleId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SynthId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleAssetRef {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthPreset {
    pub name: String,
}

pub struct DocumentPlugin;

impl Plugin for DocumentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CadenceProject>();
    }
}
