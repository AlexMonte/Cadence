//! Boot / MainMenu screen and leave-editor unsaved prompt.
//!
//! Not the editor shell top bar — that lives in [`crate::infrastructure::ui::shell::menu`].

pub mod launch;
mod plugin;
mod ui;
mod unsaved_dialog;

pub use plugin::MainMenuPlugin;
pub use ui::MainMenuUiPlugin;
