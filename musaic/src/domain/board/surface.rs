use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tessera::prelude::ContainerId;

pub use tessera::prelude::BoardSlot;

/// Canonical identity for an authored board surface.
///
/// Session-only concerns like pan, zoom, and visible bounds live elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BoardSurfaceId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardSurfaceKind {
    RootBoard,
    ContainerStack { container: ContainerId },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceLayoutKind {
    #[default]
    Board,
    Stack,
}

impl BoardSurfaceKind {
    pub fn layout_kind(&self) -> SurfaceLayoutKind {
        match self {
            BoardSurfaceKind::RootBoard => SurfaceLayoutKind::Board,
            BoardSurfaceKind::ContainerStack { .. } => SurfaceLayoutKind::Stack,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardSurface {
    pub id: BoardSurfaceId,
    pub kind: BoardSurfaceKind,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoardSurfaces {
    surfaces: BTreeMap<BoardSurfaceId, BoardSurface>,
    root: Option<BoardSurfaceId>,
}

impl BoardSurfaces {
    pub fn insert(&mut self, surface: BoardSurface) -> Result<(), BoardSurfaceError> {
        if self.surfaces.contains_key(&surface.id) {
            return Err(BoardSurfaceError::DuplicateSurface(surface.id));
        }

        if matches!(surface.kind, BoardSurfaceKind::RootBoard) {
            if let Some(existing_root) = self.root {
                return Err(BoardSurfaceError::DuplicateRoot {
                    existing: existing_root,
                    attempted: surface.id,
                });
            }
            self.root = Some(surface.id);
        }

        self.surfaces.insert(surface.id, surface);
        Ok(())
    }

    pub fn get(&self, id: BoardSurfaceId) -> Option<&BoardSurface> {
        self.surfaces.get(&id)
    }

    pub fn contains(&self, id: BoardSurfaceId) -> bool {
        self.surfaces.contains_key(&id)
    }

    pub fn kind(&self, id: BoardSurfaceId) -> Option<BoardSurfaceKind> {
        self.get(id).map(|surface| surface.kind.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSurfaceError {
    DuplicateSurface(BoardSurfaceId),
    DuplicateRoot {
        existing: BoardSurfaceId,
        attempted: BoardSurfaceId,
    },
}
