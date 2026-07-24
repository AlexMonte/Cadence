//! Pointer-hit targets without feathers `button()` (avoids theme fills over custom chrome).

use bevy::ui::widget::Text as UiText;
use bevy::{picking::prelude::*, prelude::*};
use bevy_feathers::theme::ThemedText;
use bevy_ui_widgets::Activate;

/// Marker for a layout node that emits [`Activate`] on primary click.
#[derive(Component, Clone, Copy, Default, Reflect)]
#[reflect(Component, Default)]
pub struct MusaicClickable;

pub struct MusaicClickablePlugin;

impl Plugin for MusaicClickablePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_musaic_clickable_pointer);
    }
}

/// Transparent hit area + label. Merge `overrides` with actions (e.g. [`crate::infrastructure::ui::InspectorButtonAction`]).
pub fn musaic_clickable<B: Bundle>(
    layout: Node,
    overrides: B,
    label: impl Into<String>,
) -> impl Bundle {
    (
        layout,
        MusaicClickable,
        Pickable::default(),
        overrides,
        UiText::new(label.into()),
        ThemedText,
    )
}

fn on_musaic_clickable_pointer(
    click: On<Pointer<Click>>,
    clickables: Query<(), With<MusaicClickable>>,
    mut commands: Commands,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let target = click.original_event_target();
    if clickables.get(target).is_ok() {
        commands.trigger(Activate { entity: target });
    }
}
