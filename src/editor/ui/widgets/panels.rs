use bevy::prelude::*;

use crate::editor::ui::{
    assets::UiPanelAssets,
    background::nine_slice_background,
    styles::{PanelStyle, UiStyles},
};

/// A scalable panel using the base panel asset.
pub fn panel(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_sized(styles, assets, styles.panels.panel.size)
}

/// A panel with explicit size.
pub fn panel_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel.clone().with_size(size);
    panel_bundle("Panel", style, assets.panel.clone())
}

/// A panel using the off-state asset.
pub fn panel_off(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_off_sized(styles, assets, styles.panels.panel_off.size)
}

/// A panel off-state with explicit size.
pub fn panel_off_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel_off.clone().with_size(size);
    panel_bundle("Panel Off", style, assets.panel_off.clone())
}

/// A panel using the on-state asset.
pub fn panel_on(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_on_sized(styles, assets, styles.panels.panel_on.size)
}

/// A panel on-state with explicit size.
pub fn panel_on_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel_on.clone().with_size(size);
    panel_bundle("Panel On", style, assets.panel_on.clone())
}

/// A scalable panel using the v2 panel asset.
pub fn panel_v2(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_v2_sized(styles, assets, styles.panels.panel_v2.size)
}

/// A v2 panel with explicit size.
pub fn panel_v2_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel_v2.clone().with_size(size);
    panel_bundle("Panel V2", style, assets.panel_v2.clone())
}

/// A v2 panel off-state asset.
pub fn panel_v2_off(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_v2_off_sized(styles, assets, styles.panels.panel_v2_off.size)
}

pub fn panel_v2_off_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel_v2_off.clone().with_size(size);
    panel_bundle("Panel V2 Off", style, assets.panel_v2_off.clone())
}

/// A v2 panel on-state asset.
pub fn panel_v2_on(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_v2_on_sized(styles, assets, styles.panels.panel_v2_on.size)
}

pub fn panel_v2_on_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel_v2_on.clone().with_size(size);
    panel_bundle("Panel V2 On", style, assets.panel_v2_on.clone())
}

/// A menu panel using the menu panel asset.
pub fn panel_menu(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_menu_sized(styles, assets, styles.panels.panel_menu.size)
}

/// A menu panel with explicit size.
pub fn panel_menu_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel_menu.clone().with_size(size);
    panel_bundle("Panel Menu", style, assets.panel_menu.clone())
}

/// An options panel using the options asset.
pub fn panel_options(styles: &UiStyles, assets: &UiPanelAssets) -> impl Bundle {
    panel_options_sized(styles, assets, styles.panels.panel_options.size)
}

/// An options panel with explicit size.
pub fn panel_options_sized(styles: &UiStyles, assets: &UiPanelAssets, size: Vec2) -> impl Bundle {
    let style = styles.panels.panel_options.clone().with_size(size);
    panel_bundle("Panel Options", style, assets.options.clone())
}

fn panel_bundle(name: &'static str, style: PanelStyle, image: Handle<Image>) -> impl Bundle {
    (
        Name::new(name),
        style.node(),
        nine_slice_background(image, style.border),
    )
}
