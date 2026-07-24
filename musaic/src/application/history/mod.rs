mod types;

#[cfg(test)]
mod undo_tests;

pub use types::{HistoryEntry, HistoryPolicy};

use bevy::prelude::*;

/// Maximum undo entries kept in memory.
pub const MAX_UNDO_ENTRIES: usize = 100;

/// Inverse-command undo stack (no document snapshots).
#[derive(Resource, Debug, Default)]
pub struct CommandHistory {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

impl CommandHistory {
    pub fn record_mutation(&mut self, entry: HistoryEntry) {
        self.undo.push(entry);
        while self.undo.len() > MAX_UNDO_ENTRIES {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn pop_undo(&mut self) -> Option<HistoryEntry> {
        self.undo.pop()
    }

    pub fn push_redo(&mut self, entry: HistoryEntry) {
        self.redo.push(entry);
        while self.redo.len() > MAX_UNDO_ENTRIES {
            self.redo.remove(0);
        }
    }

    pub fn pop_redo(&mut self) -> Option<HistoryEntry> {
        self.redo.pop()
    }

    pub fn push_undo(&mut self, entry: HistoryEntry) {
        self.undo.push(entry);
        while self.undo.len() > MAX_UNDO_ENTRIES {
            self.undo.remove(0);
        }
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::command::{EditorCommand, EditorInverse};
    use crate::application::editor::transaction::{Invalidation, PlacementTarget};
    use crate::domain::board::BoardSlot;
    use crate::domain::document::TileSpawnKind;
    use tessera::prelude::NodeId;

    fn place_command() -> EditorCommand {
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: crate::domain::board::BoardSurfaceId(0),
                slot: BoardSlot::new(0, 0),
            },
            tile: TileSpawnKind::Output {
                name: "main".into(),
            },
        }
    }

    fn delete_inverse() -> EditorInverse {
        EditorInverse::DeleteNode {
            node: NodeId::new("doc_1"),
        }
    }

    #[test]
    fn undo_stack_is_capped() {
        let mut history = CommandHistory::default();
        for _ in 0..=MAX_UNDO_ENTRIES {
            history.record_mutation(HistoryEntry {
                forward: place_command(),
                inverse: delete_inverse(),
                invalidation: Invalidation::document_changed(),
            });
        }
        assert_eq!(history.undo.len(), MAX_UNDO_ENTRIES);
    }
}
