use bevy::prelude::*;

use crate::editor::ui::styles::UiStyles;

/// A simple icon image.
pub fn icon(styles: &UiStyles, image: Handle<Image>) -> impl Bundle {
    icon_sized(image, styles.icons.size)
}

/// An icon image with explicit size.
pub fn icon_sized(image: Handle<Image>, size: Vec2) -> impl Bundle {
    (
        Name::new("Icon"),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        ImageNode::new(image),
    )
}

/// A simple icon button (image-only).
pub fn icon_button(styles: &UiStyles, image: Handle<Image>) -> impl Bundle {
    icon_button_sized(image, styles.icons.size)
}

/// An icon button with explicit size.
pub fn icon_button_sized(image: Handle<Image>, size: Vec2) -> impl Bundle {
    (
        Name::new("Icon Button"),
        Button,
        Interaction::None,
        Node {
            width: px(size.x),
            height: px(size.y),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        ImageNode::new(image),
    )
}
