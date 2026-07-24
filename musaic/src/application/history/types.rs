use crate::application::command::{EditorCommand, EditorInverse};
use crate::application::editor::transaction::Invalidation;

/// Whether a command should be recorded on the undo stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryPolicy {
    RecordMutation,
    Ephemeral,
}

/// One reversible edit: executing `inverse` undoes `forward`.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub forward: EditorCommand,
    pub inverse: EditorInverse,
    /// Invalidation captured when the forward mutation was accepted.
    /// Undo/redo must apply this — not a recomputed inverse result.
    pub invalidation: Invalidation,
}
