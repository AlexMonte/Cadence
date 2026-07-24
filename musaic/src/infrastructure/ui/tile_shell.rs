//! Compose shell panel chrome (`panel_bg` / `section_panel_bg` sprites with color fallback).

use bevy::prelude::*;

use crate::adapter::load_up::UiSpriteAssets;

use super::ui_sprites;

/// Main menu / inspector / board frame background (fallback when sprites are not loaded).
pub const PANEL_BG: Color = Color::srgb(0.14, 0.15, 0.19);

/// Inset areas (minimap grid, piano-roll body, drawer scroll).
pub const PANEL_INSET_BG: Color = Color::srgb(0.10, 0.11, 0.14);

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
