//! Durable editor command types and history policy.
//!
//! [`EditorCommand`] is the public user-intent vocabulary.
//! [`EditorInverse`] is the private undo vocabulary — never emitted by UI.

use tessera::prelude::{NodeId, SpatialSide};

use crate::application::board_view_settings::{AtomDisplayMode, AtomDisplayScope};
use crate::application::editor::{
    selection::SelectionMode, transaction::PlacementTarget, workspace::FocusTarget,
};
use crate::application::history::HistoryPolicy;
use crate::application::pipeline::runtime::ProjectedEventId;
use crate::application::session::MusaicProject;
use crate::domain::BoardSurfaceId;
use crate::domain::document::{AtomValue, DocumentPatch, TileSpawnKind};

/// The single editor intent vocabulary: everything the user can ask for,
/// from focus changes to durable document mutations.
#[derive(Debug, Clone)]
pub enum EditorCommand {
    SoundLibrary(super::sound_library::SoundLibraryCommand),
    EditTiles(super::editing::TileEdit),
    SetSampleBank {
        sample: crate::domain::project::samples::SampleId,
        definition: Option<crate::domain::project::samples::SampleBankDefinition>,
    },
    BeginSampleOptionsEdit {
        sample: crate::domain::project::samples::SampleId,
    },
    EndSampleOptionsEdit,
    SetSampleOptions {
        sample: crate::domain::project::samples::SampleId,
        options: crate::domain::project::samples::SampleImportOptions,
    },
    RelinkSample {
        sample: crate::domain::project::samples::SampleId,
        path: std::path::PathBuf,
    },
    ChooseAudioExport {
        cycles: u32,
    },
    RevealProjectFile,
    RevealLastExport,
    ExportAudio {
        path: std::path::PathBuf,
        cycles: u32,
    },
    AuditionSound {
        sound: NodeId,
    },
    BeginSoundEdit {
        sound: NodeId,
    },
    EndSoundEdit,
    SetSound {
        sound: NodeId,
        definition: crate::domain::instrument::InstrumentDefinition,
    },
    ImportSample {
        path: std::path::PathBuf,
        sound: Option<NodeId>,
    },
    Focus {
        target: FocusTarget,
    },
    SelectNode {
        node: NodeId,
        mode: SelectionMode,
    },
    ClearSelection,
    EnterTimelineMode,
    EnterCompose,
    PlaceTile {
        target: PlacementTarget,
        tile: TileSpawnKind,
    },
    SetAtomValue {
        node: NodeId,
        value: AtomValue,
    },
    MoveModifierGroup {
        owner: NodeId,
        step: i8,
    },
    BeginAtomEdit {
        node: NodeId,
    },
    EndAtomEdit,
    EnterContainer {
        container: NodeId,
    },
    NavigateToSurface {
        surface: BoardSurfaceId,
    },
    ConnectTiles {
        from: NodeId,
        to: NodeId,
    },
    DeleteSelection,
    JumpToTimelineSource {
        event: ProjectedEventId,
    },
    ArmPlacementTool {
        tile: TileSpawnKind,
    },
    CancelPlacement,
    BindOutputSide {
        node: NodeId,
        side: SpatialSide,
    },
    CycleConnection {
        from: NodeId,
        to: NodeId,
    },
    ToggleDrawer,
    SetDrawerOpen {
        open: bool,
    },
    ToggleMinimap,
    StartConnection {
        source: NodeId,
    },
    AbortConnection,
    Undo,
    Redo,
    TransportPlay,
    TransportPause,
    TransportPanic,
    TransportStop,
    TransportToggle,
    TransportSeek {
        position_cycles: f64,
    },
    /// Browse timing without seeking audio; None resumes following playback.
    PreviewCycle {
        cycle: Option<u32>,
    },
    SetBpm {
        bpm: f64,
    },
    SetBeatsPerCycle {
        beats: u32,
    },
    SetViewSettings {
        scope: AtomDisplayScope,
        mode: AtomDisplayMode,
    },
    /// Start a blank project. Clears history.
    NewProject,
    /// Load a project from disk. Clears history.
    OpenProject {
        path: std::path::PathBuf,
    },
    /// Adopt an already-loaded project (wasm picker / in-memory). Clears history.
    ///
    AdoptProject {
        project: MusaicProject,
        path: Option<std::path::PathBuf>,
    },
    /// Persist the open project (last path, or platform save UI if none).
    SaveProject,
    /// Persist to an explicit path (no picker). Clears dirty on success.
    SaveProjectAs {
        path: std::path::PathBuf,
    },
}

/// Private undo vocabulary. Produced only by accepted durable transactions;
/// never emitted by UI or keyboard producers.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorInverse {
    RestoreSoundLibrary {
        library: crate::domain::instrument::SoundLibrary,
    },
    RestoreTileEdit {
        record: Box<super::editing::TileEditRecord>,
    },
    RestoreSampleBank {
        sample: crate::domain::project::samples::SampleId,
        definition: Option<crate::domain::project::samples::SampleBankDefinition>,
    },
    RestoreSampleOptions {
        sample: crate::domain::project::samples::SampleId,
        options: crate::domain::project::samples::SampleImportOptions,
    },
    RestoreSampleAsset {
        sample: crate::domain::project::samples::SampleId,
        // History is in memory; decoded media never enters the project schema.
        before: Option<crate::application::session::SampleAssetSnapshot>,
        after: Option<crate::application::session::SampleAssetSnapshot>,
    },
    RestoreSound {
        sound: NodeId,
        definition: crate::domain::instrument::InstrumentDefinition,
    },
    RestoreStackOrder {
        surface: BoardSurfaceId,
        order: Vec<NodeId>,
    },
    RestoreTempo {
        bpm: f64,
        beats_per_cycle: u32,
    },
    RestoreAtomValue {
        node: NodeId,
        value: AtomValue,
    },
    DeleteNode {
        node: NodeId,
    },
    RestoreSubtree {
        patch: DocumentPatch,
    },
    DisconnectTiles {
        from: NodeId,
        to: NodeId,
    },
    RestoreConnections {
        relations: Option<Vec<tessera::prelude::RootRelation>>,
        bindings: std::collections::BTreeMap<NodeId, Option<tessera::prelude::NodeSpatialBindings>>,
    },
}

impl EditorCommand {
    pub fn history_policy(&self) -> HistoryPolicy {
        match self {
            EditorCommand::EditTiles(_) => HistoryPolicy::Ephemeral,
            EditorCommand::SoundLibrary(_)
            | EditorCommand::SetSampleBank { .. }
            | EditorCommand::SetSampleOptions { .. }
            | EditorCommand::RelinkSample { .. }
            | EditorCommand::SetSound { .. }
            | EditorCommand::PlaceTile { .. }
            | EditorCommand::SetAtomValue { .. }
            | EditorCommand::MoveModifierGroup { .. }
            | EditorCommand::SetBpm { .. }
            | EditorCommand::SetBeatsPerCycle { .. }
            | EditorCommand::DeleteSelection
            | EditorCommand::ConnectTiles { .. }
            | EditorCommand::BindOutputSide { .. }
            | EditorCommand::CycleConnection { .. } => HistoryPolicy::RecordMutation,
            EditorCommand::BeginSampleOptionsEdit { .. }
            | EditorCommand::EndSampleOptionsEdit
            | EditorCommand::ChooseAudioExport { .. }
            | EditorCommand::RevealProjectFile
            | EditorCommand::RevealLastExport
            | EditorCommand::ExportAudio { .. }
            | EditorCommand::AuditionSound { .. }
            | EditorCommand::BeginSoundEdit { .. }
            | EditorCommand::EndSoundEdit
            | EditorCommand::ImportSample { .. }
            | EditorCommand::Focus { .. }
            | EditorCommand::BeginAtomEdit { .. }
            | EditorCommand::EndAtomEdit
            | EditorCommand::SelectNode { .. }
            | EditorCommand::ClearSelection
            | EditorCommand::EnterTimelineMode
            | EditorCommand::EnterCompose
            | EditorCommand::EnterContainer { .. }
            | EditorCommand::NavigateToSurface { .. }
            | EditorCommand::JumpToTimelineSource { .. }
            | EditorCommand::ArmPlacementTool { .. }
            | EditorCommand::CancelPlacement
            | EditorCommand::ToggleDrawer
            | EditorCommand::SetDrawerOpen { .. }
            | EditorCommand::ToggleMinimap
            | EditorCommand::StartConnection { .. }
            | EditorCommand::AbortConnection
            | EditorCommand::Undo
            | EditorCommand::Redo
            | EditorCommand::TransportPlay
            | EditorCommand::TransportPause
            | EditorCommand::TransportPanic
            | EditorCommand::TransportStop
            | EditorCommand::TransportToggle
            | EditorCommand::TransportSeek { .. }
            | EditorCommand::PreviewCycle { .. }
            | EditorCommand::SetViewSettings { .. }
            | EditorCommand::NewProject
            | EditorCommand::OpenProject { .. }
            | EditorCommand::AdoptProject { .. }
            | EditorCommand::SaveProject
            | EditorCommand::SaveProjectAs { .. } => HistoryPolicy::Ephemeral,
        }
    }
}

impl PartialEq for EditorCommand {
    fn eq(&self, other: &Self) -> bool {
        use EditorCommand::*;
        match (self, other) {
            (ChooseAudioExport { cycles: a }, ChooseAudioExport { cycles: b }) => a == b,
            (RevealProjectFile, RevealProjectFile) | (RevealLastExport, RevealLastExport) => true,
            (SoundLibrary(a), SoundLibrary(b)) => a == b,
            (EditTiles(a), EditTiles(b)) => a == b,
            (
                SetSampleBank {
                    sample: a,
                    definition: av,
                },
                SetSampleBank {
                    sample: b,
                    definition: bv,
                },
            ) => a == b && av == bv,
            (BeginSampleOptionsEdit { sample: a }, BeginSampleOptionsEdit { sample: b }) => a == b,
            (EndSampleOptionsEdit, EndSampleOptionsEdit) => true,
            (
                SetSampleOptions {
                    sample: a,
                    options: av,
                },
                SetSampleOptions {
                    sample: b,
                    options: bv,
                },
            ) => a == b && av == bv,
            (
                RelinkSample {
                    sample: a,
                    path: av,
                },
                RelinkSample {
                    sample: b,
                    path: bv,
                },
            ) => a == b && av == bv,
            (
                ExportAudio {
                    path: a,
                    cycles: ac,
                },
                ExportAudio {
                    path: b,
                    cycles: bc,
                },
            ) => a == b && ac == bc,
            (AuditionSound { sound: a }, AuditionSound { sound: b })
            | (BeginSoundEdit { sound: a }, BeginSoundEdit { sound: b }) => a == b,
            (EndSoundEdit, EndSoundEdit) => true,
            (
                SetSound {
                    sound: a,
                    definition: av,
                },
                SetSound {
                    sound: b,
                    definition: bv,
                },
            ) => a == b && av == bv,
            (ImportSample { path: a, sound: av }, ImportSample { path: b, sound: bv }) => {
                a == b && av == bv
            }
            (SetAtomValue { node: a, value: av }, SetAtomValue { node: b, value: bv }) => {
                a == b && av == bv
            }
            (
                MoveModifierGroup { owner: a, step: av },
                MoveModifierGroup { owner: b, step: bv },
            ) => a == b && av == bv,
            (BeginAtomEdit { node: a }, BeginAtomEdit { node: b }) => a == b,
            (EndAtomEdit, EndAtomEdit) => true,
            (Focus { target: a }, Focus { target: b }) => a == b,
            (SetDrawerOpen { open: a }, SetDrawerOpen { open: b }) => a == b,
            (SelectNode { node: a, mode: ma }, SelectNode { node: b, mode: mb }) => {
                a == b && ma == mb
            }
            (ClearSelection, ClearSelection)
            | (EnterTimelineMode, EnterTimelineMode)
            | (EnterCompose, EnterCompose)
            | (DeleteSelection, DeleteSelection)
            | (ToggleDrawer, ToggleDrawer)
            | (ToggleMinimap, ToggleMinimap)
            | (CancelPlacement, CancelPlacement)
            | (AbortConnection, AbortConnection)
            | (Undo, Undo)
            | (Redo, Redo)
            | (TransportPlay, TransportPlay)
            | (TransportPause, TransportPause)
            | (TransportPanic, TransportPanic)
            | (TransportStop, TransportStop)
            | (TransportToggle, TransportToggle)
            | (NewProject, NewProject)
            | (SaveProject, SaveProject) => true,
            (
                PlaceTile {
                    target: ta,
                    tile: a,
                },
                PlaceTile {
                    target: tb,
                    tile: b,
                },
            ) => ta == tb && a == b,
            (EnterContainer { container: a }, EnterContainer { container: b }) => a == b,
            (NavigateToSurface { surface: a }, NavigateToSurface { surface: b }) => a == b,
            (ConnectTiles { from: fa, to: ta }, ConnectTiles { from: fb, to: tb }) => {
                fa == fb && ta == tb
            }
            (JumpToTimelineSource { event: a }, JumpToTimelineSource { event: b }) => a == b,
            (ArmPlacementTool { tile: a }, ArmPlacementTool { tile: b }) => a == b,
            (BindOutputSide { node: a, side: sa }, BindOutputSide { node: b, side: sb }) => {
                a == b && sa == sb
            }
            (CycleConnection { from: fa, to: ta }, CycleConnection { from: fb, to: tb }) => {
                fa == fb && ta == tb
            }
            (StartConnection { source: a }, StartConnection { source: b }) => a == b,
            (TransportSeek { position_cycles: a }, TransportSeek { position_cycles: b }) => a == b,
            (PreviewCycle { cycle: a }, PreviewCycle { cycle: b }) => a == b,
            (SetBpm { bpm: a }, SetBpm { bpm: b }) => a == b,
            (SetBeatsPerCycle { beats: a }, SetBeatsPerCycle { beats: b }) => a == b,
            (
                SetViewSettings {
                    scope: sa,
                    mode: ma,
                },
                SetViewSettings {
                    scope: sb,
                    mode: mb,
                },
            ) => sa == sb && ma == mb,
            (OpenProject { path: a }, OpenProject { path: b }) => a == b,
            (AdoptProject { path: a, .. }, AdoptProject { path: b, .. }) => a == b,
            (SaveProjectAs { path: a }, SaveProjectAs { path: b }) => a == b,
            _ => false,
        }
    }
}
