//! Frontend surface area for the Dioxus app.
//!
//! Keep the mounted editor rooted in `app_shell` and `native_editor` so there is
//! a single supported entry path for the desktop UI.

mod app_shell;
mod modal;
mod native_editor;

pub use self::app_shell::{AppShell, app_route};
