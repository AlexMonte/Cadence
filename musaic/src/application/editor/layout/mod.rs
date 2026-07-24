use serde::{Deserialize, Serialize};

use crate::application::editor::workspace::{NavigationMode, WorkspaceMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceLayoutKind {
    /// Compact top controls.
    /// Lower region: board 2/3, context panel 1/3.
    ComposeBoardInspector,
    /// Expanded top controls/search/debug.
    /// Middle region: full-width timeline/piano roll.
    /// Lower region: board 2/3, context panel 1/3.
    TimelineStackedOverBoardInspector,
}

pub fn layout_for_mode(mode: WorkspaceMode) -> WorkspaceLayoutKind {
    match mode {
        WorkspaceMode::Compose => WorkspaceLayoutKind::ComposeBoardInspector,
        WorkspaceMode::Navigate(NavigationMode::Timeline) => {
            WorkspaceLayoutKind::TimelineStackedOverBoardInspector
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_mode_uses_board_inspector_layout() {
        assert_eq!(
            layout_for_mode(WorkspaceMode::Compose),
            WorkspaceLayoutKind::ComposeBoardInspector
        );
    }

    #[test]
    fn timeline_mode_uses_stacked_timeline_layout() {
        assert_eq!(
            layout_for_mode(WorkspaceMode::Navigate(NavigationMode::Timeline)),
            WorkspaceLayoutKind::TimelineStackedOverBoardInspector
        );
    }
}
