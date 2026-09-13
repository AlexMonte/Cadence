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
pub use semantic::SemanticColors;

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
        let semantic = SemanticColors::artist_default();
        Self {
            spacing: SpacingScale::default_dark(),
            radii: RadiiScale::default_dark(),
            typography: TypeScale::default_dark(),
            chrome: ChromeColors::default_dark(),
            semantic,
            motion: MotionTokens::default_dark(),
            board: BoardMaterialColors::from_semantic(semantic),
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
            panel_gap: 1.0,
            shell_padding: 0.0,
            panel_padding: 16.0,
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
    /// The approved paper, forest and brass editor palette.
    pub const fn default_dark() -> Self {
        Self {
            window_bg: Color::srgb(0.961, 0.957, 0.941),
            panel_bg: Color::WHITE,
            panel_inset: Color::srgb(0.937, 0.937, 0.918),
            section_tint: Color::srgb(0.961, 0.957, 0.941),
            border: Color::srgb(0.863, 0.882, 0.851),
            button_bg: Color::WHITE,
            button_border: Color::srgb(0.863, 0.882, 0.851),
            overlay_scrim: Color::srgba(0.10, 0.14, 0.12, 0.45),
            accent: Color::srgb(0.157, 0.420, 0.310),
            danger: Color::srgb(0.67, 0.25, 0.22),
            playhead: Color::srgb(0.157, 0.420, 0.310),
            edge_handle: Color::srgba(0.39, 0.44, 0.42, 0.7),
            edge_handle_idle: Color::srgba(0.39, 0.44, 0.42, 0.14),
            edge_handle_active: Color::srgba(0.16, 0.42, 0.31, 0.3),
            diagnostics_bg: Color::srgb(0.961, 0.957, 0.941),
            crumb_active_text: Color::srgb(0.157, 0.420, 0.310),
            crumb_selected_bg: Color::srgb(0.886, 0.933, 0.898),
            text_main: Color::srgb(0.153, 0.200, 0.184),
            text_dim: Color::srgb(0.396, 0.443, 0.416),
            board_clear: Color::srgb(0.961, 0.957, 0.941),
            palette_clear: Color::srgb(0.961, 0.957, 0.941),
            palette_fallback_tile: Color::srgb(0.886, 0.933, 0.898),
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
    /// Board PBR fallbacks share semantic artist colors so 3D and 2D chrome match.
    pub const fn from_semantic(semantic: SemanticColors) -> Self {
        Self {
            board: Color::srgb(0.925, 0.933, 0.914),
            grid_line: Color::srgb(0.843, 0.863, 0.831),
            container: semantic.container,
            atom: semantic.atom_operator,
            output: semantic.output,
            trick: semantic.flow_control,
            generic: semantic.transform_generic,
            selected: semantic.focus_accent,
            focused: semantic.focus_accent,
            locked_slot: Color::srgb(0.925, 0.933, 0.906),
            connection_scalar: semantic.flow_scalar,
            connection_control: semantic.flow_control,
            connection_preview: semantic.flow_control,
            drag_preview: Color::srgba(0.86, 0.90, 0.98, 0.55),
            gltf_fallback: semantic.transform_generic,
        }
    }

    pub const fn default_dark() -> Self {
        Self::from_semantic(SemanticColors::artist_default())
    }
}
