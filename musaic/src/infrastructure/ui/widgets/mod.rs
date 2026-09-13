//! Reusable UI primitives — one button path, panel chrome, dialog card.

mod button;
mod dialog;
mod focus;
mod panel;
mod slider;

pub use button::{
    ButtonAccessibilityLabel, ButtonLabel, MusaicClickable, MusaicClickablePlugin, musaic_button,
    musaic_chrome_button,
};
pub(crate) use dialog::scroll_dialog;
pub use dialog::{spawn_dialog_action_button, spawn_dialog_card, spawn_dialog_overlay};
pub use panel::{PanelBackdrop, panel_bg, panel_inset_bg, spawn_shell_panel};
pub(crate) use slider::ExactSlider;
pub use slider::{MusaicSliderPlugin, slider};

pub(crate) mod exact_number;
