//! Editor shell: root layout, in-editor top chrome, breadcrumbs, rebuild driver.
//!
//! [`menu`] is the editor Play/Tiles bar — not the boot MainMenu
//! ([`super::screens::main_menu`]).

pub mod breadcrumbs;
pub mod layout;
pub mod menu;
pub mod rebuild;

#[cfg(not(target_arch = "wasm32"))]
pub mod export_dialog;

pub(crate) mod dropdown;
