use bevy::prelude::*;

use crate::editor::ui::{
    assets::{UiButtonCheckAssets, UiButtonCheckboxAssets},
    background::nine_slice_background,
    styles::UiStyles,
};

/// A checkbox (square) with optional checked state.
pub fn checkbox(styles: &UiStyles, assets: &UiButtonCheckboxAssets, checked: bool) -> impl Bundle {
    checkbox_sized(styles, assets, checked, styles.toggles.checkbox_size)
}

/// A checkbox (square) with explicit size.
pub fn checkbox_sized(
    _styles: &UiStyles,
    assets: &UiButtonCheckboxAssets,
    checked: bool,
    size: Vec2,
) -> impl Bundle {
    let image = if checked {
        assets.check_on.clone()
    } else {
        assets.check_off.clone()
    };

    (
        Name::new("Checkbox"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(image),
    )
}

/// A small checkbox (square) with optional checked state.
pub fn checkbox_small(
    styles: &UiStyles,
    assets: &UiButtonCheckboxAssets,
    checked: bool,
) -> impl Bundle {
    checkbox_small_sized(styles, assets, checked, styles.toggles.checkbox_small_size)
}

/// A small checkbox (square) with explicit size.
pub fn checkbox_small_sized(
    _styles: &UiStyles,
    assets: &UiButtonCheckboxAssets,
    checked: bool,
    size: Vec2,
) -> impl Bundle {
    let image = if checked {
        assets.small_check_on.clone()
    } else {
        assets.small_check_off.clone()
    };

    (
        Name::new("Checkbox Small"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(image),
    )
}

/// A small round checkbox with optional checked state.
pub fn checkbox_round(
    styles: &UiStyles,
    assets: &UiButtonCheckboxAssets,
    checked: bool,
) -> impl Bundle {
    checkbox_round_sized(styles, assets, checked, styles.toggles.checkbox_round_size)
}

/// A small round checkbox with explicit size.
pub fn checkbox_round_sized(
    _styles: &UiStyles,
    assets: &UiButtonCheckboxAssets,
    checked: bool,
    size: Vec2,
) -> impl Bundle {
    let image = if checked {
        assets.small_check_round_on.clone()
    } else {
        assets.small_check_round_off.clone()
    };

    (
        Name::new("Checkbox Round"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(image),
    )
}

/// A check-style button (image only) with optional checked state.
pub fn check_button(styles: &UiStyles, assets: &UiButtonCheckAssets, checked: bool) -> impl Bundle {
    check_button_sized(styles, assets, checked, styles.toggles.check_button_size)
}

/// A check-style button (image only) with explicit size.
pub fn check_button_sized(
    styles: &UiStyles,
    assets: &UiButtonCheckAssets,
    checked: bool,
    size: Vec2,
) -> impl Bundle {
    let image = if checked {
        assets.on.clone()
    } else {
        assets.off.clone()
    };

    (
        Name::new("Check Button"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        nine_slice_background(image, styles.toggles.check_button_border),
    )
}

/// A check-style button rendered as a small tile.
pub fn check_tile(styles: &UiStyles, assets: &UiButtonCheckAssets, checked: bool) -> impl Bundle {
    let image = if checked {
        assets.on.clone()
    } else {
        assets.off.clone()
    };

    (
        Name::new("Check Tile"),
        Node {
            width: px(styles.toggles.check_tile_size.x),
            height: px(styles.toggles.check_tile_size.y),
            ..default()
        },
        nine_slice_background(image, styles.toggles.check_tile_border),
    )
}
