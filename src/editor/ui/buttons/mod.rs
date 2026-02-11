//! Reusable button primitives and typed button constructors for editor UI.

use crate::editor::ui::{
    background::nine_slice_background,
    interactions::{InteractiveChild, PaletteField},
};

use super::{
    interactions::InteractionPalette,
    styles::{ButtonImages, ButtonStyle, UiStyles},
};
use bevy::{
    ecs::{spawn::SpawnWith, system::IntoObserverSystem},
    prelude::*,
};

pub mod arrow;
pub mod big;
pub mod check;
pub mod checkbox;
pub mod long;
pub mod normal;
pub mod skewed;
pub mod tab;
pub mod tabs;

pub trait ButtonAsset<T: Asset> {
    fn none(&self) -> Handle<Image>;
    fn hovered(&self) -> Handle<Image>;
    fn pressed(&self) -> Handle<Image>;
}

/// A simple button with text and an action defined as an [`Observer`]. The button's layout is provided by `button_bundle`.
pub fn button_base<E, B, M, I>(
    text: impl Into<String>,
    action: I,
    button_bundle: impl Bundle,
    images: Option<ButtonImages>,
    style: ButtonStyle,
) -> impl Bundle
where
    E: EntityEvent,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{
    let text = text.into();
    let action = IntoObserverSystem::into_system(action);
    let text_font = style.text.text_font();
    let text_color = style.text.color;
    let border = style.border;
    let colors = style.colors.clone();

    (
        Name::new("Button"),
        GlobalTransform::default(),
        Node::default(),
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            let mut button = parent.spawn((
                Name::new("Button Inner"),
                Button,
                Pickable::default(),
                InteractiveChild,
                children![(
                    Name::new("Button Text"),
                    Text(text.clone()),
                    text_font.clone(),
                    TextColor(text_color),
                    // Don't bubble picking events from the text up to the button.
                    Pickable::IGNORE,
                )],
            ));

            if let Some(button_images) = images {
                button.insert((
                    nine_slice_background(button_images.none.clone(), border),
                    InteractionPalette {
                        none: PaletteField::Image(button_images.none.clone()),
                        hovered: PaletteField::Image(button_images.hovered.clone()),
                        pressed: PaletteField::Image(button_images.pressed.clone()),
                        active: PaletteField::Color(colors.active),
                    },
                ));
            } else {
                button.insert((
                    BackgroundColor(colors.none),
                    InteractionPalette {
                        none: PaletteField::Color(colors.none),
                        hovered: PaletteField::Color(colors.hovered),
                        pressed: PaletteField::Color(colors.pressed),
                        active: PaletteField::Color(colors.active),
                    },
                ));
            }

            button.insert(button_bundle).observe(action);
        })),
    )
}

/// A small square button with text and an action defined as an [`Observer`].
pub fn button_small<E, B, M, I>(
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
    let style = styles.buttons.small.clone();
    let images: ButtonImages = images.into();
    button_base(text, action, style.node(), Some(images), style)
}
