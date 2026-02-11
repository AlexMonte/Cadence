use bevy::prelude::*;

use crate::editor::ui::{assets::UiBackgroundAssets, styles::UiStyles};

/// A background image node.
pub fn ui_background(styles: &UiStyles, assets: &UiBackgroundAssets) -> impl Bundle {
    ui_background_sized(styles, assets, styles.background.background_size)
}

/// A background image node with explicit size.
pub fn ui_background_sized(
    _styles: &UiStyles,
    assets: &UiBackgroundAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("UI Background"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.background.clone()),
    )
}

/// A title image node.
pub fn ui_title(styles: &UiStyles, assets: &UiBackgroundAssets) -> impl Bundle {
    ui_title_sized(styles, assets, styles.background.title_size)
}

/// A title image node with explicit size.
pub fn ui_title_sized(_styles: &UiStyles, assets: &UiBackgroundAssets, size: Vec2) -> impl Bundle {
    (
        Name::new("UI Title"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.title.clone()),
    )
}
