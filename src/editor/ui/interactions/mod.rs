//! Interaction state handling and visual palette syncing for UI widgets.

use bevy::prelude::*;

#[cfg(feature = "ui_audio")]
pub mod audio;

pub(super) fn plugin(app: &mut App) {
    #[cfg(feature = "ui_reflect")]
    app.register_type::<InteractionPalette>();
    app.add_systems(Update, apply_interaction_palette);
}

/// Marker component for interactive children (sliders, toggles, buttons, etc) within a node.
/// When a click/drag occurs on an entity with this component, it can be routed to the
/// interactive handler while simultaneously selecting the owner node.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component, Debug)]
pub struct InteractiveChild;

#[derive(Debug, Clone, PartialEq, Reflect)]
#[reflect(Debug, Clone, PartialEq)]
pub enum PaletteField {
    Image(Handle<Image>),
    Color(Color),
}

/// Palette for widget interactions. Add this to an entity that supports
/// [`Interaction`]s, such as a button, to change its [`BackgroundColor`] based
/// on the current interaction state.
#[derive(Component, Debug, Reflect)]
#[reflect(Component, Debug)]
#[require(Interaction)]
pub struct InteractionPalette {
    pub none: PaletteField,
    pub hovered: PaletteField,
    pub pressed: PaletteField,
    pub active: PaletteField,
}

fn apply_interaction_palette(
    mut palette_query: Query<
        (
            &Interaction,
            &InteractionPalette,
            &mut BackgroundColor,
            Option<&mut ImageNode>,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, palette, mut background, image) in &mut palette_query {
        let field = palette_field_for_interaction(interaction, palette);
        if let Some(mut img) = image {
            if let PaletteField::Image(image) = field {
                img.image = image.clone();
            }
        } else {
            *background = BackgroundColor(match field {
                PaletteField::Color(color) => *color,
                PaletteField::Image(_) => Color::BLACK,
            });
        }
    }
}

fn palette_field_for_interaction<'a>(
    interaction: &Interaction,
    palette: &'a InteractionPalette,
) -> &'a PaletteField {
    match interaction {
        Interaction::None => &palette.none,
        Interaction::Hovered => &palette.hovered,
        Interaction::Pressed => &palette.pressed,
    }
}
