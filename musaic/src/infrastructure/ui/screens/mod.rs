//! App-state screens (MainMenu, Loading, UnsavedDialog).
//!
//! Distinct from [`super::shell::menu`], which is the in-editor top chrome
//! (Play / Tiles), not the boot MainMenu.
//!
//! **Phase 4 note for Phase 6:** `infrastructure/menu` was moved here as
//! `screens/main_menu/`. Loading lives at `screens/loading.rs`. Prefer wiring
//! schedule / camera against these paths — do not rename the folder again.

pub mod loading;
pub mod main_menu;

pub use loading::LoadingUiPlugin;
pub use main_menu::{MainMenuPlugin, MainMenuUiPlugin};
