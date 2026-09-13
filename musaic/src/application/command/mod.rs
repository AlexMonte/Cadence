//! Editor command dispatch — the single intent vocabulary via [`EditorCommandBus`](bus::EditorCommandBus).
//!
//! **Owner:** command history + command execution
//! **Inputs:** pick classification, UI controls, keyboard shortcuts, undo/redo

mod bus;
pub mod connection;
pub mod editing;
mod logic;
mod musical_edits;
pub mod note_entry;
pub mod sound_library;
mod types;

pub use crate::application::editor::transaction::{Invalidation, PlacementTarget};
pub use bus::{EditorCommandBus, register_command_bus};
pub use logic::{execute_command, execute_inverse};
pub use types::{EditorCommand, EditorInverse};

pub mod flow;
