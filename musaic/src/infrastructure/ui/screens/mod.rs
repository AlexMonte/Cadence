//! App-state screens (MainMenu, Loading, UnsavedDialog).
//!
//! Distinct from [`super::shell::menu`], which is the in-editor top chrome
//! (Play / Tiles), not the boot MainMenu.

pub mod loading;
pub mod main_menu;

pub use loading::LoadingUiPlugin;
pub use main_menu::{MainMenuPlugin, MainMenuUiPlugin};
