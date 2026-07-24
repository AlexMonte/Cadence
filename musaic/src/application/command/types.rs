//! Durable editor command types and history policy.
//!
//! [`EditorCommand`] is the public user-intent vocabulary.
//! [`EditorInverse`] is the private undo vocabulary — never emitted by UI.

use serde::{Deserialize, Serialize};
use tessera::prelude::{NodeId, SpatialSide};

use crate::application::board_view_settings::{AtomDisplayMode, AtomDisplayScope};
use crate::application::editor::{
    PortSlotState, selection::SelectionMode, transaction::PlacementTarget, workspace::FocusTarget,
};
use crate::application::history::HistoryPolicy;
use crate::application::pipeline::runtime::ProjectedEventId;
use crate::application::session::MusaicProject;
use crate::domain::BoardSurfaceId;
use crate::domain::document::{DocumentPatch, RemovedBoardBinding, TileSpawnKind};

/// The single editor intent vocabulary: everything the user can ask for,
/// from focus changes to durable document mutations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorCommand {
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
    ToggleMinimap,
    StartConnection {
        source: NodeId,
    },
    AbortConnection,
    Undo,
    Redo,
    TransportPlay,
    TransportStop,
    TransportToggle,
    TransportSeek {
        position_cycles: f64,
    },
    SetBpm {
        bpm: f64,
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
    /// `project` is skipped in serde — live document payload, not a persisted command.
    AdoptProject {
        #[serde(skip)]
        project: MusaicProject,
        #[serde(default)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EditorInverse {
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
    RestorePortBinding {
        node: NodeId,
        side: SpatialSide,
        port_state: PortSlotState,
        #[serde(default)]
        removed_binding: Option<RemovedBoardBinding>,
    },
}

impl EditorCommand {
    pub fn history_policy(&self) -> HistoryPolicy {
        match self {
            EditorCommand::PlaceTile { .. }
            | EditorCommand::DeleteSelection
            | EditorCommand::ConnectTiles { .. }
            | EditorCommand::BindOutputSide { .. }
            | EditorCommand::CycleConnection { .. } => HistoryPolicy::RecordMutation,
            EditorCommand::Focus { .. }
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
            | EditorCommand::ToggleMinimap
            | EditorCommand::StartConnection { .. }
            | EditorCommand::AbortConnection
            | EditorCommand::Undo
            | EditorCommand::Redo
            | EditorCommand::TransportPlay
            | EditorCommand::TransportStop
            | EditorCommand::TransportToggle
            | EditorCommand::TransportSeek { .. }
            | EditorCommand::SetBpm { .. }
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
            (Focus { target: a }, Focus { target: b }) => a == b,
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
            (SetBpm { bpm: a }, SetBpm { bpm: b }) => a == b,
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
