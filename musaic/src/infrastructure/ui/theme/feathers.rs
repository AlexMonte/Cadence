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
        (tokens::BUTTON_BG_HOVER, c.panel_inset),
        (tokens::BUTTON_BG_PRESSED, c.crumb_selected_bg),
        (tokens::BUTTON_BG_DISABLED, c.panel_inset),
        (tokens::BUTTON_PRIMARY_BG, c.accent),
        (tokens::BUTTON_PRIMARY_BG_HOVER, c.accent.lighter(0.05)),
        (tokens::BUTTON_PRIMARY_BG_PRESSED, c.accent.lighter(0.10)),
        (tokens::BUTTON_PRIMARY_BG_DISABLED, c.panel_inset),
        (tokens::BUTTON_TEXT, c.text_main),
        (tokens::BUTTON_TEXT_DISABLED, c.text_main.with_alpha(0.5)),
        (tokens::BUTTON_PRIMARY_TEXT, Color::WHITE),
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
    // Use the sans-serif face already bundled by our Feathers dependency.
    // Keep individual widget sizes while replacing Bevy's monospace fallback.
    if let Some(assets) = app.world().get_resource::<AssetServer>() {
        let handle = assets.load("embedded://bevy_feathers/assets/fonts/FiraSans-Regular.ttf");
        let symbols = assets.load("fonts/MusaicSymbols-Regular.ttf");
        app.insert_resource(EditorDefaultFont {
            handle,
            symbols,
            ready: false,
        })
        .add_systems(Update, apply_default_editor_font);
    }
    let theme = MusaicUiTheme::default_dark();
    let feathers = feathers_theme_from(&theme);
    app.insert_resource(theme)
        .insert_resource(UiTheme(feathers));
}

#[derive(Resource)]
struct EditorDefaultFont {
    handle: Handle<Font>,
    symbols: Handle<Font>,
    ready: bool,
}

fn apply_default_editor_font(
    mut source: ResMut<EditorDefaultFont>,
    mut fonts: ResMut<Assets<Font>>,
    mut font_system: ResMut<bevy::text::CosmicFontSystem>,
    mut pipeline: ResMut<bevy::text::TextPipeline>,
) {
    if source.ready {
        return;
    }
    if let Some(font) = fonts.get(&source.handle).cloned()
        && fonts.contains(&source.symbols)
    {
        // Register the shared symbols before replacing the default font. That
        // replacement triggers text remeasurement with the fallback available.
        // Latin text keeps Fira Sans; only missing symbols use these outlines.
        bevy::text::load_font_to_fontdb(
            &TextFont {
                font: source.symbols.clone(),
                ..default()
            },
            &mut font_system.0,
            &mut pipeline.map_handle_to_font_id,
            &fonts,
        );
        fonts
            .insert(bevy::asset::AssetId::default(), font)
            .expect("default font slot");
        source.ready = true;
    }
}

#[cfg(test)]
mod symbol_tests {
    use super::*;
    use bevy::text::{
        ComputedTextBlock, CosmicFontSystem, FontHinting, LineBreak, LineHeight, TextBounds,
        TextPipeline,
    };

    #[test]
    fn musical_fallback_shapes_real_symbols_without_replacing_latin_text() {
        let mut fonts = Assets::<Font>::default();
        let latin =
            fonts.add(Font::try_from_bytes(bevy::text::DEFAULT_FONT_DATA.to_vec()).unwrap());
        let symbols = fonts.add(
            Font::try_from_bytes(
                include_bytes!("../../../../assets/fonts/MusaicSymbols-Regular.ttf").to_vec(),
            )
            .unwrap(),
        );
        let mut pipeline = TextPipeline::default();
        let mut system = CosmicFontSystem::default();
        let mut block = ComputedTextBlock::default();
        let text_font = TextFont {
            font: latin.clone(),
            font_size: 24.0,
            ..default()
        };
        let shape = |pipeline: &mut TextPipeline,
                     system: &mut CosmicFontSystem,
                     block: &mut ComputedTextBlock| {
            pipeline
                .update_buffer(
                    &fonts,
                    [(
                        Entity::PLACEHOLDER,
                        0,
                        "D♮4 ♯♭↗↔↑↓≡",
                        &text_font,
                        Color::WHITE,
                        LineHeight::default(),
                    )]
                    .into_iter(),
                    LineBreak::NoWrap,
                    Justify::Left,
                    TextBounds::UNBOUNDED,
                    1.0,
                    block,
                    system,
                    FontHinting::Disabled,
                )
                .unwrap();
        };
        shape(&mut pipeline, &mut system, &mut block);
        assert!(
            block
                .buffer()
                .layout_runs()
                .flat_map(|run| run.glyphs)
                .any(|glyph| glyph.glyph_id == 0),
            "fixture's original face lacks the musical signs"
        );
        // Production registers fallback before assigning the primary UI face.
        pipeline = TextPipeline::default();
        system = CosmicFontSystem::default();
        block = ComputedTextBlock::default();
        bevy::text::load_font_to_fontdb(
            &TextFont {
                font: symbols.clone(),
                ..default()
            },
            &mut system.0,
            &mut pipeline.map_handle_to_font_id,
            &fonts,
        );
        shape(&mut pipeline, &mut system, &mut block);
        let glyphs: Vec<_> = block
            .buffer()
            .layout_runs()
            .flat_map(|run| run.glyphs)
            .collect();
        assert!(!glyphs.is_empty());
        assert!(
            glyphs.iter().all(|glyph| glyph.glyph_id != 0),
            "every requested symbol must have an outline: {glyphs:?}"
        );
        let latin_id = pipeline.map_handle_to_font_id[&latin.id()].0;
        let symbol_id = pipeline.map_handle_to_font_id[&symbols.id()].0;
        assert_eq!(glyphs[0].font_id, latin_id, "D keeps the primary font");
        assert_eq!(glyphs[1].font_id, symbol_id, "natural uses the fallback");
        assert_eq!(glyphs[2].font_id, latin_id, "octave keeps the primary font");
    }
}
