//! Reusable UI primitives — one button path, panel chrome, dialog card.

mod button;
mod dialog;
mod panel;

pub use button::{
    MusaicClickable, MusaicClickablePlugin, musaic_button, musaic_chrome_button, musaic_clickable,
};
pub use dialog::{spawn_dialog_action_button, spawn_dialog_card, spawn_dialog_overlay};
pub use panel::{PanelBackdrop, panel_bg, panel_inset_bg, spawn_shell_panel};
