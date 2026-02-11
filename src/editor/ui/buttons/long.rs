use bevy::{ecs::system::IntoObserverSystem, prelude::*};

use crate::editor::ui::styles::{ButtonImages, UiStyles};

use super::button_base;

/// A medium-sized long button with text and an action defined as an [`Observer`].
pub fn button_long<E, B, M, I>(
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
    button_long_sized(text, styles, images, action, styles.buttons.long.size)
}

/// A long button with explicit size.
pub fn button_long_sized<E, B, M, I>(
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
    let style = styles.buttons.long.clone().with_size(size);
    let images: ButtonImages = images.into();

    button_base(text, action, style.node(), Some(images), style)
}
