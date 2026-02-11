use bevy::prelude::*;

use crate::editor::ui::{
    assets::UiComponentAssets, background::nine_slice_background, styles::UiStyles,
};

/// A symmetric button component (plain image).
pub fn symetric_button(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    symetric_button_sized(styles, assets, styles.components.symetric_size)
}

/// A symmetric button component with explicit size.
pub fn symetric_button_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Symetric Button"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.button_symetric.clone()),
    )
}

/// A symmetric button component using 9-slice.
pub fn symetric_button_sliced(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    symetric_button_sliced_sized(styles, assets, styles.components.symetric_size)
}

/// A symmetric button component (sliced) with explicit size.
pub fn symetric_button_sliced_sized(
    styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Symetric Button Sliced"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        nine_slice_background(
            assets.button_symetric_sliced.clone(),
            styles.components.symetric_border,
        ),
    )
}

/// A sliced button corner (top-left).
pub fn button_slice_top_left(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    button_slice_top_left_sized(styles, assets, styles.components.slice_size)
}

pub fn button_slice_top_left_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Button Slice Top Left"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.button_sliced_top_left.clone()),
    )
}

/// A sliced button corner (top-right).
pub fn button_slice_top_right(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    button_slice_top_right_sized(styles, assets, styles.components.slice_size)
}

pub fn button_slice_top_right_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Button Slice Top Right"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.button_sliced_top_right.clone()),
    )
}

/// A sliced button corner (bottom-left).
pub fn button_slice_bottom_left(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    button_slice_bottom_left_sized(styles, assets, styles.components.slice_size)
}

pub fn button_slice_bottom_left_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Button Slice Bottom Left"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.button_sliced_bottom_left.clone()),
    )
}

/// A sliced button corner (bottom-right).
pub fn button_slice_bottom_right(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    button_slice_bottom_right_sized(styles, assets, styles.components.slice_size)
}

pub fn button_slice_bottom_right_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Button Slice Bottom Right"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.button_sliced_bottom_right.clone()),
    )
}

/// A chevron icon (left).
pub fn chevron_left(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    chevron_left_sized(styles, assets, styles.components.chevron_size)
}

/// A chevron icon (left) with explicit size.
pub fn chevron_left_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Chevron Left"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.chevron_left.clone()),
    )
}

/// A chevron icon (right).
pub fn chevron_right(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    chevron_right_sized(styles, assets, styles.components.chevron_size)
}

/// A chevron icon (right) with explicit size.
pub fn chevron_right_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Chevron Right"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.chevron_right.clone()),
    )
}

/// A switch base component.
pub fn switch_base(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    switch_base_sized(styles, assets, styles.components.switch_base_size)
}

/// A switch base component with explicit size.
pub fn switch_base_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Switch Base"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.switch_base.clone()),
    )
}

/// A switch head component.
pub fn switch_head(styles: &UiStyles, assets: &UiComponentAssets) -> impl Bundle {
    switch_head_sized(styles, assets, styles.components.switch_head_size)
}

/// A switch head component with explicit size.
pub fn switch_head_sized(
    _styles: &UiStyles,
    assets: &UiComponentAssets,
    size: Vec2,
) -> impl Bundle {
    (
        Name::new("Switch Head"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(assets.switch_head.clone()),
    )
}
