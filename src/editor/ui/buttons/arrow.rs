use bevy::{ecs::system::IntoObserverSystem, prelude::*};

use crate::editor::ui::styles::{ButtonImages, UiStyles};

use super::button_base;

/// A small arrow button with text and an action defined as an [`Observer`].
pub fn button_arrow<E, B, M, I>(
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
    button_arrow_sized(text, styles, images, action, styles.buttons.arrow.size)
}

/// An arrow button with explicit size.
pub fn button_arrow_sized<E, B, M, I>(
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
    let style = styles.buttons.arrow.clone().with_size(size);
    let images: ButtonImages = images.into();

    button_base(text, action, style.node(), Some(images), style)
}
