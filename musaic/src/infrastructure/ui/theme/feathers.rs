//! Map [`MusaicUiTheme`] → Feathers [`ThemeProps`] / [`UiTheme`].

use bevy::color::{Alpha, Luminance};
use bevy::prelude::*;
use bevy_feathers::{
    dark_theme::create_dark_theme,
    theme::{ThemeProps, UiTheme},
    tokens,
};

use super::MusaicUiTheme;

/// Build a Feathers theme from Musaic design tokens (keeps Feathers widget
/// tokens that Musaic does not override, then remaps chrome roles).
pub fn feathers_theme_from(theme: &MusaicUiTheme) -> ThemeProps {
    let mut props = create_dark_theme();
    let c = &theme.chrome;

    let overrides = [
        (tokens::WINDOW_BG, c.window_bg),
        (tokens::BUTTON_BG, c.button_bg),
        (tokens::BUTTON_BG_HOVER, c.button_bg.lighter(0.05)),
        (tokens::BUTTON_BG_PRESSED, c.button_bg.lighter(0.10)),
        (tokens::BUTTON_BG_DISABLED, c.panel_inset),
        (tokens::BUTTON_PRIMARY_BG, c.accent),
        (tokens::BUTTON_PRIMARY_BG_HOVER, c.accent.lighter(0.05)),
        (tokens::BUTTON_PRIMARY_BG_PRESSED, c.accent.lighter(0.10)),
        (tokens::BUTTON_PRIMARY_BG_DISABLED, c.panel_inset),
        (tokens::BUTTON_TEXT, c.text_main),
        (tokens::BUTTON_TEXT_DISABLED, c.text_main.with_alpha(0.5)),
        (tokens::BUTTON_PRIMARY_TEXT, c.text_main),
        (
            tokens::BUTTON_PRIMARY_TEXT_DISABLED,
            c.text_main.with_alpha(0.5),
        ),
        (tokens::SLIDER_BG, c.panel_inset),
        (tokens::SLIDER_BAR, c.accent),
        (tokens::SLIDER_BAR_DISABLED, c.panel_inset),
        (tokens::SLIDER_TEXT, c.text_main),
        (tokens::SLIDER_TEXT_DISABLED, c.text_dim),
        (tokens::CHECKBOX_BG, c.button_bg),
        (tokens::CHECKBOX_BG_CHECKED, c.accent),
        (tokens::CHECKBOX_TEXT, c.text_dim),
        (tokens::RADIO_BORDER, c.border),
        (tokens::RADIO_MARK, c.accent),
        (tokens::RADIO_TEXT, c.text_dim),
        (tokens::SWITCH_BG, c.button_bg),
        (tokens::SWITCH_BG_CHECKED, c.accent),
        (tokens::SWITCH_BORDER, c.border),
        (tokens::COLOR_PLANE_BG, c.panel_inset),
        (tokens::FOCUS_RING, theme.semantic.focus_accent),
        (tokens::TEXT_MAIN, c.text_main),
        (tokens::TEXT_DIM, c.text_dim),
    ];

    for (token, color) in overrides {
        props.color.insert(token, color);
    }
    props
}

pub fn insert_musaic_ui_theme(app: &mut App) {
    let theme = MusaicUiTheme::default_dark();
    let feathers = feathers_theme_from(&theme);
    app.insert_resource(theme)
        .insert_resource(UiTheme(feathers));
}
