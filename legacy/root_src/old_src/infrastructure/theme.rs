use bevy::prelude::*;

#[derive(Debug, Resource, Clone)]
pub struct CadenceTheme {
    pub shell_background: Color,
    pub panel_background: Color,
    pub panel_border: Color,
    pub board_background: Color,
    pub board_grid_minor: Color,
    pub board_grid_major: Color,
    pub tile_fill: Color,
    pub tile_selected_fill: Color,
    pub tile_border: Color,
    pub wire: Color,
    pub selection: Color,
    pub text: Color,
    pub muted_text: Color,
}

impl Default for CadenceTheme {
    fn default() -> Self {
        Self {
            shell_background: Color::srgb(0.08, 0.08, 0.09),
            panel_background: Color::srgb(0.12, 0.12, 0.14),
            panel_border: Color::srgb(0.22, 0.22, 0.25),
            board_background: Color::srgb(0.05, 0.05, 0.06),
            board_grid_minor: Color::srgba(0.22, 0.24, 0.27, 0.35),
            board_grid_major: Color::srgba(0.35, 0.38, 0.42, 0.65),
            tile_fill: Color::srgb(0.18, 0.27, 0.34),
            tile_selected_fill: Color::srgb(0.32, 0.48, 0.58),
            tile_border: Color::srgb(0.72, 0.78, 0.86),
            wire: Color::srgb(0.86, 0.74, 0.35),
            selection: Color::srgb(0.96, 0.92, 0.66),
            text: Color::srgb(0.96, 0.96, 0.96),
            muted_text: Color::srgb(0.72, 0.74, 0.78),
        }
    }
}

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        let theme = CadenceTheme::default();
        app.insert_resource(ClearColor(theme.board_background))
            .insert_resource(theme);
    }
}
