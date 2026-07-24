//! Canonical editor workspace types — focus, active surface, navigation mode.
//!
//! Focus transitions are validated against [`DocumentQueries`]: illegal targets
//! are rejected at the API instead of silently accepted.

use bevy::prelude::{Message, Resource};
use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;

use crate::application::pipeline::runtime::ProjectedEventId;
use crate::domain::board::{BoardSlot, BoardSurfaceId, BoardSurfaces};
use crate::domain::document::{DocumentQueries, PortId, StackIndex};

/// WorkspaceMode changes layout and available navigation tools.
/// ActiveSpace changes which authored board is the current home location.
/// FocusTarget changes what the user is examining or acting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceMode {
    Compose,
    Navigate(NavigationMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NavigationMode {
    Timeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActiveSpace {
    Board(BoardSurfaceId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FocusTarget {
    None,
    EmptySlot {
        surface: BoardSurfaceId,
        slot: BoardSlot,
    },
    StackInsert {
        surface: BoardSurfaceId,
        index: StackIndex,
    },
    Tile {
        node: NodeId,
    },
    Atom {
        node: NodeId,
    },
    Port {
        node: NodeId,
        port: PortId,
    },
    TimelineEvent {
        event: ProjectedEventId,
    },
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EditorAttention {
    pub workspace_mode: WorkspaceMode,
    pub active_space: ActiveSpace,
    pub focus: FocusTarget,
}

#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSurfaceChanged {
    pub previous: BoardSurfaceId,
    pub current: BoardSurfaceId,
    pub reason: ActiveSurfaceChangeReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveSurfaceChangeReason {
    EnterContainer { container_id: NodeId },
    BreadcrumbJump,
    DeleteFallbackToRoot,
    TimelineJump,
    OpenDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSurfaceError {
    MissingSurface(BoardSurfaceId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusError {
    MissingSurface(BoardSurfaceId),
    MissingNode(NodeId),
    SlotNotEmpty {
        surface: BoardSurfaceId,
        slot: BoardSlot,
    },
}

impl std::fmt::Display for FocusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSurface(surface) => write!(f, "focus surface {} missing", surface.0),
            Self::MissingNode(node) => write!(f, "focus node {} missing", node.0),
            Self::SlotNotEmpty { surface, slot } => {
                write!(
                    f,
                    "focus empty slot ({}, {}) on surface {} is occupied",
                    slot.x, slot.y, surface.0
                )
            }
        }
    }
}

/// Ergonomic read/write API for canonical editor attention.
impl EditorAttention {
    pub fn new(root_surface: BoardSurfaceId) -> Self {
        Self {
            workspace_mode: WorkspaceMode::Compose,
            active_space: ActiveSpace::Board(root_surface),
            focus: FocusTarget::None,
        }
    }

    pub fn active_board(&self) -> BoardSurfaceId {
        match self.active_space {
            ActiveSpace::Board(surface) => surface,
        }
    }

    pub fn enter_compose(&mut self) {
        self.workspace_mode = WorkspaceMode::Compose;
    }

    pub fn enter_navigation(&mut self, mode: NavigationMode) {
        self.workspace_mode = WorkspaceMode::Navigate(mode);
    }

    pub fn set_active_board(
        &mut self,
        surfaces: &BoardSurfaces,
        surface: BoardSurfaceId,
        reason: ActiveSurfaceChangeReason,
    ) -> Result<Option<ActiveSurfaceChanged>, ActiveSurfaceError> {
        if !surfaces.contains(surface) {
            return Err(ActiveSurfaceError::MissingSurface(surface));
        }

        let previous = self.active_board();
        if previous == surface {
            return Ok(None);
        }

        self.active_space = ActiveSpace::Board(surface);
        self.focus = FocusTarget::None;

        Ok(Some(ActiveSurfaceChanged {
            previous,
            current: surface,
            reason,
        }))
    }

    /// Validate and set focus. Illegal targets are rejected — callers must not
    /// invent nodes or slots the document does not contain.
    pub fn focus(
        &mut self,
        queries: &DocumentQueries<'_>,
        target: FocusTarget,
    ) -> Result<(), FocusError> {
        validate_focus_target(queries, &target)?;
        self.focus = target;
        Ok(())
    }
}

fn validate_focus_target(
    queries: &DocumentQueries<'_>,
    target: &FocusTarget,
) -> Result<(), FocusError> {
    match target {
        FocusTarget::None | FocusTarget::TimelineEvent { .. } => Ok(()),
        FocusTarget::EmptySlot { surface, slot } => {
            if !queries.has_surface(*surface) {
                return Err(FocusError::MissingSurface(*surface));
            }
            if !queries.is_board_slot_empty(*surface, *slot) {
                return Err(FocusError::SlotNotEmpty {
                    surface: *surface,
                    slot: *slot,
                });
            }
            Ok(())
        }
        FocusTarget::StackInsert { surface, .. } => {
            if !queries.has_surface(*surface) {
                return Err(FocusError::MissingSurface(*surface));
            }
            Ok(())
        }
        FocusTarget::Tile { node }
        | FocusTarget::Atom { node }
        | FocusTarget::Port { node, .. } => {
            if !queries.contains_node(node) {
                return Err(FocusError::MissingNode(node.clone()));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::MusaicDocument;

    #[test]
    fn focus_none_always_ok() {
        let document = MusaicDocument::new_empty();
        let queries = DocumentQueries::new(&document);
        let mut attention = EditorAttention::new(document.root_surface);
        attention.focus(&queries, FocusTarget::None).unwrap();
    }

    #[test]
    fn focus_missing_node_is_rejected() {
        let document = MusaicDocument::new_empty();
        let queries = DocumentQueries::new(&document);
        let mut attention = EditorAttention::new(document.root_surface);
        let err = attention
            .focus(
                &queries,
                FocusTarget::Tile {
                    node: NodeId::new("missing"),
                },
            )
            .unwrap_err();
        assert!(matches!(err, FocusError::MissingNode(_)));
    }

    #[test]
    fn focus_empty_slot_on_root_ok() {
        let document = MusaicDocument::new_empty();
        let queries = DocumentQueries::new(&document);
        let mut attention = EditorAttention::new(document.root_surface);
        attention
            .focus(
                &queries,
                FocusTarget::EmptySlot {
                    surface: document.root_surface,
                    slot: BoardSlot::new(0, 0),
                },
            )
            .unwrap();
    }
}
