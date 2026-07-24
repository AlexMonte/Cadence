//! Shell panel chrome (`panel_bg` / `section_panel_bg` sprites with theme fallback).

use bevy::prelude::*;

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::ui_sprites;

/// Main menu / inspector / board frame background (fallback when sprites are not loaded).
/// Prefer [`MusaicUiTheme::chrome.panel_bg`] at spawn sites.
pub fn panel_bg(theme: &MusaicUiTheme) -> Color {
    theme.chrome.panel_bg
}

/// Inset areas (minimap grid, piano-roll body, drawer scroll).
pub fn panel_inset_bg(theme: &MusaicUiTheme) -> Color {
    theme.chrome.panel_inset
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelBackdrop {
    /// No bitmap — caller supplies `BackgroundColor` on the panel node.
    None,
    /// `ui/panel_bg.png`
    Panel,
    /// `ui/section_panel_bg.png`
    Section,
}

/// Flex column wrapper for a shell region.
pub fn spawn_shell_panel(
    parent: &mut ChildSpawnerCommands<'_>,
    panel: impl Bundle,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
    backdrop: PanelBackdrop,
    content: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    parent.spawn(panel).with_children(|panel| {
        if backdrop != PanelBackdrop::None {
            ui_sprites::spawn_panel_backdrop(panel, images, sprites, backdrop);
        }
        content(panel);
    });
}
