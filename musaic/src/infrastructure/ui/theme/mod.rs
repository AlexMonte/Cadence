//! Musaic design tokens — the single stylesheet for shell, screens, and board chrome.
//!
//! Feathers [`UiTheme`] is derived from the same tokens via [`feathers::feathers_theme_from`].

pub mod feathers;
pub mod motion;
pub mod semantic;

use bevy::prelude::*;

pub use feathers::{feathers_theme_from, insert_musaic_ui_theme};
pub(crate) use motion::{
    INSPECTOR_SLIDE_DISTANCE_FALLBACK, InspectorPanelHost, InspectorSlideLayer,
    InspectorTransitionQueue, MotionTokens, PendingInspectorTransition, SlideDirection,
    UiInspectorBody, UiInspectorHeader, UiInspectorViewport, drive_inspector_slides,
    finish_active_slides,
};
pub use semantic::{ArtistPalette, SemanticColors, atom_value_color};

/// Canonical Musaic UI theme resource.
#[derive(Resource, Debug, Clone)]
pub struct MusaicUiTheme {
    pub spacing: SpacingScale,
    pub radii: RadiiScale,
    pub typography: TypeScale,
    pub chrome: ChromeColors,
    pub semantic: SemanticColors,
    pub motion: MotionTokens,
    pub board: BoardMaterialColors,
}

impl MusaicUiTheme {
    pub fn default_dark() -> Self {
        Self {
            spacing: SpacingScale::default_dark(),
            radii: RadiiScale::default_dark(),
            typography: TypeScale::default_dark(),
            chrome: ChromeColors::default_dark(),
            semantic: SemanticColors::artist_default(),
            motion: MotionTokens::default_dark(),
            board: BoardMaterialColors::default_dark(),
        }
    }
}

impl Default for MusaicUiTheme {
    fn default() -> Self {
        Self::default_dark()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpacingScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub panel_gap: f32,
    pub shell_padding: f32,
    pub panel_padding: f32,
}

impl SpacingScale {
    pub const fn default_dark() -> Self {
        Self {
            xs: 4.0,
            sm: 6.0,
            md: 8.0,
            lg: 12.0,
            xl: 16.0,
            panel_gap: 12.0,
            shell_padding: 10.0,
            panel_padding: 14.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RadiiScale {
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
}

impl RadiiScale {
    pub const fn default_dark() -> Self {
        Self {
            sm: 4.0,
            md: 6.0,
            lg: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeScale {
    pub body: f32,
    pub title: f32,
    pub hero: f32,
    pub brand: f32,
}

impl TypeScale {
    pub const fn default_dark() -> Self {
        Self {
            body: 14.0,
            title: 20.0,
            hero: 28.0,
            brand: 32.0,
        }
    }
}

/// Chrome surface / role colors for 2D UI spawners.
#[derive(Debug, Clone, Copy)]
pub struct ChromeColors {
    pub window_bg: Color,
    pub panel_bg: Color,
    pub panel_inset: Color,
    pub section_tint: Color,
    pub border: Color,
    pub button_bg: Color,
    pub button_border: Color,
    pub overlay_scrim: Color,
    pub accent: Color,
    pub danger: Color,
    pub playhead: Color,
    pub edge_handle: Color,
    pub edge_handle_idle: Color,
    pub edge_handle_active: Color,
    pub diagnostics_bg: Color,
    pub crumb_active_text: Color,
    pub crumb_selected_bg: Color,
    pub text_main: Color,
    pub text_dim: Color,
    pub board_clear: Color,
    pub palette_clear: Color,
    pub palette_fallback_tile: Color,
}

impl ChromeColors {
    pub const fn default_dark() -> Self {
        Self {
            window_bg: Color::srgb(0.08, 0.09, 0.12),
            panel_bg: Color::srgb(0.14, 0.15, 0.19),
            panel_inset: Color::srgb(0.10, 0.11, 0.14),
            section_tint: Color::srgb(0.19, 0.20, 0.25),
            border: Color::srgba(0.35, 0.38, 0.45, 1.0),
            button_bg: Color::srgba(0.22, 0.24, 0.30, 1.0),
            button_border: Color::srgba(0.40, 0.43, 0.50, 1.0),
            overlay_scrim: Color::srgba(0.0, 0.0, 0.0, 0.55),
            accent: Color::srgb(0.18, 0.75, 0.85),
            danger: Color::srgb(0.85, 0.28, 0.28),
            playhead: Color::srgba(1.0, 0.85, 0.2, 0.9),
            edge_handle: Color::srgba(0.72, 0.76, 0.82, 0.85),
            edge_handle_idle: Color::srgba(0.35, 0.40, 0.50, 0.22),
            edge_handle_active: Color::srgba(0.35, 0.40, 0.50, 0.15),
            diagnostics_bg: Color::srgba(0.1, 0.1, 0.12, 0.85),
            crumb_active_text: Color::srgb(0.13, 0.14, 0.18),
            crumb_selected_bg: Color::srgba(1.0, 1.0, 1.0, 0.14),
            text_main: Color::srgb(1.0, 1.0, 1.0),
            text_dim: Color::srgb(0.70, 0.72, 0.76),
            board_clear: Color::srgb(0.11, 0.12, 0.15),
            palette_clear: Color::srgb(0.10, 0.11, 0.14),
            palette_fallback_tile: Color::srgb(0.22, 0.24, 0.30),
        }
    }
}

/// 3D board / connection material base colors.
#[derive(Debug, Clone, Copy)]
pub struct BoardMaterialColors {
    pub board: Color,
    pub grid_line: Color,
    pub container: Color,
    pub atom: Color,
    pub output: Color,
    pub trick: Color,
    pub generic: Color,
    pub selected: Color,
    pub focused: Color,
    pub locked_slot: Color,
    pub connection_scalar: Color,
    pub connection_control: Color,
    pub connection_preview: Color,
    pub drag_preview: Color,
    pub gltf_fallback: Color,
}

impl BoardMaterialColors {
    pub const fn default_dark() -> Self {
        Self {
            board: Color::srgb(0.10, 0.105, 0.13),
            grid_line: Color::srgb(0.28, 0.31, 0.38),
            container: Color::srgb(0.29, 0.36, 0.52),
            atom: Color::srgb(0.22, 0.48, 0.34),
            output: Color::srgb(0.61, 0.42, 0.18),
            trick: Color::srgb(0.45, 0.32, 0.60),
            generic: Color::srgb(0.38, 0.40, 0.46),
            selected: Color::srgb(0.95, 0.78, 0.22),
            focused: Color::srgb(0.18, 0.75, 0.85),
            locked_slot: Color::srgb(0.05, 0.055, 0.07),
            connection_scalar: Color::srgb(0.20, 0.78, 0.92),
            connection_control: Color::srgb(0.95, 0.55, 0.18),
            connection_preview: Color::srgba(0.55, 0.85, 1.0, 0.45),
            drag_preview: Color::srgba(0.86, 0.90, 0.98, 0.55),
            gltf_fallback: Color::srgb(0.5, 0.4, 0.3),
        }
    }
}
