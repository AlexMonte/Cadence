use bevy::{ecs::spawn::SpawnWith, prelude::*};

use crate::editor::ui::{
    assets::UiRangeAssets, background::nine_slice_background, styles::UiStyles,
};

/// A range slider track (empty).
pub fn range_track(styles: &UiStyles, assets: &UiRangeAssets) -> impl Bundle {
    range_track_sized(styles, assets, styles.range.track_size)
}

/// A range slider track with explicit size.
pub fn range_track_sized(styles: &UiStyles, assets: &UiRangeAssets, size: Vec2) -> impl Bundle {
    (
        Name::new("Range Track"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        nine_slice_background(assets.bar_empty.clone(), styles.range.border),
    )
}

/// A range slider fill (full bar).
pub fn range_fill(styles: &UiStyles, assets: &UiRangeAssets) -> impl Bundle {
    range_fill_sized(styles, assets, styles.range.track_size)
}

/// A range slider fill with explicit size.
pub fn range_fill_sized(styles: &UiStyles, assets: &UiRangeAssets, size: Vec2) -> impl Bundle {
    (
        Name::new("Range Fill"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        nine_slice_background(assets.bar_full.clone(), styles.range.border),
    )
}

/// A range slider grabber.
pub fn range_grabber(styles: &UiStyles, assets: &UiRangeAssets, active: bool) -> impl Bundle {
    range_grabber_sized(styles, assets, active, styles.range.grabber_size)
}

/// A range slider grabber with explicit size.
pub fn range_grabber_sized(
    _styles: &UiStyles,
    assets: &UiRangeAssets,
    active: bool,
    size: Vec2,
) -> impl Bundle {
    let image = if active {
        assets.grabber_on.clone()
    } else {
        assets.grabber_off.clone()
    };

    (
        Name::new("Range Grabber"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(image),
    )
}

/// A composite range slider (track + fill + grabber).
pub fn range_slider(
    styles: &UiStyles,
    assets: &UiRangeAssets,
    fill_percent: f32,
    grabber_on: bool,
) -> impl Bundle {
    range_slider_sized(
        styles,
        assets,
        styles.range.track_size,
        fill_percent,
        grabber_on,
    )
}

/// A composite range slider with explicit size.
pub fn range_slider_sized(
    styles: &UiStyles,
    assets: &UiRangeAssets,
    size: Vec2,
    fill_percent: f32,
    grabber_on: bool,
) -> impl Bundle {
    let fill = fill_percent.clamp(0.0, 1.0);
    let grabber_size = styles.range.grabber_size;
    let grabber_half = grabber_size.x / 2.0;
    let grabber_left = (size.x * fill - grabber_half).clamp(0.0, size.x - grabber_size.x);
    let grabber_top = (size.y - grabber_size.y) * 0.5;
    let grabber_image = if grabber_on {
        assets.grabber_on.clone()
    } else {
        assets.grabber_off.clone()
    };
    let border = styles.range.border;
    let bar_empty = assets.bar_empty.clone();
    let bar_full = assets.bar_full.clone();

    (
        Name::new("Range Slider"),
        Node {
            width: px(size.x),
            height: px(size.y),
            position_type: PositionType::Relative,
            ..default()
        },
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            parent.spawn((
                Name::new("Range Track"),
                Node {
                    width: px(size.x),
                    height: px(size.y),
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    ..default()
                },
                nine_slice_background(bar_empty.clone(), border),
            ));

            parent.spawn((
                Name::new("Range Fill"),
                Node {
                    width: px(size.x * fill),
                    height: px(size.y),
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    ..default()
                },
                nine_slice_background(bar_full.clone(), border),
            ));

            parent.spawn((
                Name::new("Range Grabber"),
                Node {
                    width: px(grabber_size.x),
                    height: px(grabber_size.y),
                    position_type: PositionType::Absolute,
                    left: px(grabber_left),
                    top: px(grabber_top),
                    ..default()
                },
                ImageNode::new(grabber_image.clone()),
            ));
        })),
    )
}
