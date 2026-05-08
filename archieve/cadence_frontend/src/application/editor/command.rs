use crate::adapter::GridPos;

use super::{PendingProjectAction, WorkspaceMode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorCommand {
    OpenPicker,
    PlacePiece { piece_id: String },
    SwitchWorkspace { mode: WorkspaceMode },
    OpenCompile,
    OpenInspector,
    ProjectAction { action: PendingProjectAction },
    SaveProject { force_dialog: bool },
    ExportSong,
    PlayRuntime,
    StopRuntime,
    Undo,
    Redo,
    ToggleConsole,
    ToggleDevInspector,
    ClearTransientPanels,
    SelectTile {
        position: GridPos,
        additive: bool,
    },
    SelectCell {
        position: GridPos,
        additive: bool,
    },
    BeginTileDrag {
        position: GridPos,
    },
    PreviewTileDrag {
        position: GridPos,
    },
    CommitTileDrag {
        position: GridPos,
    },
    BeginConnection {
        from: GridPos,
    },
    PreviewConnection {
        position: GridPos,
    },
    CommitConnection {
        position: GridPos,
    },
    BeginMarquee {
        origin: GridPos,
        additive: bool,
    },
    PreviewMarquee {
        current: GridPos,
    },
    CommitMarquee,
    ClearBoardInteraction,
}
