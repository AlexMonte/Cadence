use bevy::{ecs::system::IntoObserverSystem, prelude::*};

use crate::editor::ui::styles::{ButtonImages, UiStyles};

use super::button_base;

/// A check-style button with text and an action defined as an [`Observer`].
pub fn check_button<E, B, M, I>(
    text: impl Into<String>,
    styles: &UiStyles,
    images: impl Into<ButtonImages>,
    action: I,
) -> impl Bundle
where
    E: EntityEvent,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{
    check_button_sized(text, styles, images, action, styles.buttons.check.size)
}

/// A check-style button with explicit size.
pub fn check_button_sized<E, B, M, I>(
    text: impl Into<String>,
    styles: &UiStyles,
    images: impl Into<ButtonImages>,
    action: I,
    size: Vec2,
) -> impl Bundle
where
    E: EntityEvent,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{
    let style = styles.buttons.check.clone().with_size(size);
    let images: ButtonImages = images.into();

    button_base(text, action, style.node(), Some(images), style)
}
