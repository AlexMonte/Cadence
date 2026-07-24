//! Dialog overlay + card chrome shared by unsaved-changes and future modals.

use bevy::{picking::prelude::*, prelude::*};
use bevy_feathers::theme::ThemedText;

use crate::infrastructure::ui::theme::MusaicUiTheme;

/// Full-screen scrim hosting a centered dialog card.
pub fn spawn_dialog_overlay(
    commands: &mut Commands,
    theme: &MusaicUiTheme,
    root: impl Bundle,
    build_card: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) -> Entity {
    commands
        .spawn((
            root,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.chrome.overlay_scrim),
            ZIndex(1000),
        ))
        .with_children(|overlay| {
            spawn_dialog_card(overlay, theme, build_card);
        })
        .id()
}

pub fn spawn_dialog_card(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    content: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    parent
        .spawn((
            Node {
                min_width: Val::Px(360.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(theme.spacing.xl),
                padding: UiRect::all(Val::Px(theme.spacing.xl + theme.spacing.md)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(theme.radii.md)),
                ..default()
            },
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.border),
        ))
        .with_children(content);
}

/// Dialog action row button using theme chrome colors.
///
/// Returns the button entity so callers can attach a click observer.
pub fn spawn_dialog_action_button(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    label: &str,
    action: impl Bundle,
) -> Entity {
    parent
        .spawn((
            action,
            Node {
                height: Val::Px(36.0),
                padding: UiRect::horizontal(Val::Px(theme.spacing.lg + 2.0)),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme.chrome.button_bg),
            BorderColor::all(theme.chrome.button_border),
            Pickable::default(),
        ))
        .with_children(|btn| {
            btn.spawn((Text::new(label.to_string()), ThemedText));
        })
        .id()
}
