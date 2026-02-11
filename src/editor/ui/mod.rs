//! Shared editor UI foundations: assets, styles, and primitive widgets.

use bevy::prelude::*;
use std::borrow::Cow;

use crate::shared::resources::LoadResource;

pub mod assets;
pub mod background;
pub mod buttons;
pub mod interactions;
pub mod palette;
pub mod prelude;
#[cfg(feature = "ui_preview")]
pub mod preview;
pub mod styles;
pub mod widgets;

pub use assets::*;
pub use palette::*;
#[cfg(feature = "ui_preview")]
pub use preview::UiPreviewConfig;
pub use styles::*;
pub use widgets::*;

const DEFAULT_FONT: &str = "fonts/kenney_fonts/Kenney Mini.ttf";
const DEFAULT_FONT_BOLD: &str = "fonts/kenney_fonts/Kenney Mini Square.ttf";
const DEFAULT_FONT_MONO: &str = "fonts/kenney_fonts/Kenney Mini Square Mono.ttf";
const CAD_FONT: &str = "fonts/kenney_fonts/Kenney Pixel.ttf";
const CAD_FONT_BOLD: &str = "fonts/kenney_fonts/Kenney Pixel Square.ttf";

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(interactions::plugin);
    #[cfg(feature = "ui_reflect")]
    {
        app.register_type::<UiStyles>();
        app.register_type::<FontAssets>();
    }
    app.init_resource::<UiStyles>();
    app.init_resource::<FontAssets>();
    app.load_resource::<FontAssets>();
    app.load_resource::<UiButtonNormalAssets>();
    app.load_resource::<UiButtonLongAssets>();
    app.load_resource::<UiButtonBigAssets>();
    app.load_resource::<UiButtonSkewedAssets>();
    app.load_resource::<UiButtonArrowAssets>();
    app.load_resource::<UiButtonTabAssets>();
    app.load_resource::<UiButtonTabsAssets>();
    app.load_resource::<UiButtonCheckAssets>();
    app.load_resource::<UiButtonCheckboxAssets>();
    app.load_resource::<UiPanelAssets>();
    app.load_resource::<UiRangeAssets>();
    app.load_resource::<UiComponentAssets>();
    app.load_resource::<UiIconAssets>();
    app.load_resource::<UiKeybindingIconAssets>();
    app.load_resource::<UiBackgroundAssets>();
    app.load_resource::<UiFxAssets>();
}

/// A root UI node that fills the window and centers its content.
pub fn ui_root(name: impl Into<Cow<'static, str>>) -> impl Bundle {
    (
        Name::new(name),
        GlobalTransform::default(),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_self: AlignSelf::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            position_type: PositionType::Absolute,
            ..Default::default()
        },
        // Don't block picking events for other UI roots.
        Pickable::IGNORE,
    )
}

pub fn label(text: impl Into<String>, styles: &UiStyles) -> impl Bundle {
    label_with_style(text, &styles.text.label)
}

/// Spawns a text label using an explicit text style.
pub fn label_with_style(text: impl Into<String>, style: &TextStyle) -> impl Bundle {
    (
        Name::new("Label"),
        Text(text.into()),
        style.text_font(),
        TextColor(style.color),
    )
}
/// A simple header label. Bigger than [`label`].
pub fn header(text: impl Into<String>, styles: &UiStyles) -> impl Bundle {
    header_with_style(text, &styles.text.header)
}

/// Spawns a header label using an explicit text style.
pub fn header_with_style(text: impl Into<String>, style: &TextStyle) -> impl Bundle {
    (
        Name::new("Header"),
        Text(text.into()),
        style.text_font(),
        TextColor(style.color),
    )
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
/// Handles for fonts used by editor UI text styles.
pub struct FontAssets {
    #[dependency]
    pub default_font: Handle<Font>,
    #[dependency]
    pub default_font_bold: Handle<Font>,
    #[dependency]
    pub default_font_mono: Handle<Font>,
    #[dependency]
    pub pixel_font: Handle<Font>,
    #[dependency]
    pub pixel_font_bold: Handle<Font>,
}

impl FromWorld for FontAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            default_font: assets.load(DEFAULT_FONT),
            default_font_bold: assets.load(DEFAULT_FONT_BOLD),
            default_font_mono: assets.load(DEFAULT_FONT_MONO),
            pixel_font: assets.load(CAD_FONT),
            pixel_font_bold: assets.load(CAD_FONT_BOLD),
        }
    }
}
