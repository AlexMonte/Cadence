use bevy::prelude::*;

use crate::editor::ui::{assets::UiFxAssets, styles::UiStyles};

/// A light-blue fx sprite.
pub fn fx_light_blue(styles: &UiStyles, assets: &UiFxAssets) -> impl Bundle {
    fx_light_blue_sized(styles, assets, styles.fx.light_blue_size)
}

/// A light-blue fx sprite with explicit size.
pub fn fx_light_blue_sized(_styles: &UiStyles, assets: &UiFxAssets, size: Vec2) -> impl Bundle {
    (
        Name::new("FX Light Blue"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.light_blue.clone()),
    )
}
