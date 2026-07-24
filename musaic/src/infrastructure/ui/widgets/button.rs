//! One clickable button path for Musaic UI (shell chips, dialogs, screens).
//!
//! Emits [`Activate`] on primary click — same contract as the former
//! `musaic_clickable` helper, without Feathers `button()` theme fills over
//! custom chrome.

use bevy::ui::widget::Text as UiText;
use bevy::{picking::prelude::*, prelude::*};
use bevy_feathers::theme::ThemedText;
use bevy_ui_widgets::Activate;

use crate::infrastructure::ui::theme::MusaicUiTheme;

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

/// Transparent hit area + label. Merge `overrides` with actions
/// (e.g. inspector / menu command carriers).
pub fn musaic_button<B: Bundle>(
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

/// Chrome-styled button (background + border from theme).
pub fn musaic_chrome_button<B: Bundle>(
    theme: &MusaicUiTheme,
    label: impl Into<String>,
    overrides: B,
) -> impl Bundle {
    (
        Node {
            min_width: Val::Px(120.0),
            height: Val::Px(36.0),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(theme.spacing.lg)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        MusaicClickable,
        Pickable::default(),
        BackgroundColor(theme.chrome.button_bg),
        BorderColor::all(theme.chrome.button_border),
        overrides,
        UiText::new(label.into()),
        ThemedText,
    )
}

/// Backward-compatible alias while call sites migrate.
#[inline]
pub fn musaic_clickable<B: Bundle>(
    layout: Node,
    overrides: B,
    label: impl Into<String>,
) -> impl Bundle {
    musaic_button(layout, overrides, label)
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
