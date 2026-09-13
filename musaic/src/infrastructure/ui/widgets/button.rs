//! One clickable button path for Musaic UI (shell chips, dialogs, screens).
//!
//! Uses Bevy's headless button for pointer and keyboard activation while
//! retaining Musaic's own fills and text colors.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::ui::widget::Text as UiText;
use bevy::{picking::prelude::*, prelude::*};
use bevy_feathers::theme::ThemedText;
use bevy_ui_widgets::Button as CoreButton;

use crate::infrastructure::ui::theme::MusaicUiTheme;

/// Accessible, tab-focusable button, including manually constructed shell chips.
#[derive(Component, Clone, Copy, Default, Reflect)]
#[reflect(Component, Default)]
#[require(CoreButton, TabIndex)]
pub struct MusaicClickable;

/// Describes a glyph or compact preview without inserting visible label text.
#[derive(Component)]
pub struct ButtonAccessibilityLabel(pub String);

/// Label state stays on the action entity; its text is rendered as a child so
/// flex centering, padding and borders belong to the button rather than text.
#[derive(Component)]
pub struct ButtonLabel(pub String);

#[derive(Component)]
struct ButtonLabelText;

pub struct MusaicClickablePlugin;

impl Plugin for MusaicClickablePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(super::focus::plugin);
        app.register_type::<MusaicClickable>();
        app.add_systems(
            PostUpdate,
            (sync_button_labels, sync_button_accessibility_labels)
                .before(bevy::ui::UiSystems::Layout),
        );
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
        ButtonLabel(label.into()),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(MusaicUiTheme::default_dark().chrome.text_main),
        bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
        ThemedText,
        children![(ButtonLabelText, UiText::new(""), Pickable::IGNORE)],
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
            border_radius: BorderRadius::all(px(theme.radii.md)),
            ..default()
        },
        MusaicClickable,
        Pickable::default(),
        BackgroundColor(theme.chrome.button_bg),
        BorderColor::all(theme.chrome.button_border),
        overrides,
        ButtonLabel(label.into()),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(theme.chrome.text_main),
        bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
        ThemedText,
        children![(ButtonLabelText, UiText::new(""), Pickable::IGNORE)],
    )
}

fn sync_button_labels(
    buttons: Query<(&ButtonLabel, &TextFont, &TextColor), Without<ButtonLabelText>>,
    mut labels: Query<(&ChildOf, &mut Text, &mut TextFont, &mut TextColor), With<ButtonLabelText>>,
) {
    for (parent, mut text, mut font, mut color) in &mut labels {
        let Ok((label, source_font, source_color)) = buttons.get(parent.parent()) else {
            continue;
        };
        if text.0 != label.0 {
            text.0.clone_from(&label.0);
        }
        if *font != *source_font {
            *font = source_font.clone();
        }
        if *color != *source_color {
            *color = *source_color;
        }
    }
}

/// CoreButton supplies the role; explicit labels also describe icon previews.
fn sync_button_accessibility_labels(
    mut buttons: Query<
        (
            &ButtonLabel,
            Option<&ButtonAccessibilityLabel>,
            &mut bevy::a11y::AccessibilityNode,
        ),
        (
            With<MusaicClickable>,
            Or<(
                Changed<ButtonLabel>,
                Added<MusaicClickable>,
                Changed<ButtonAccessibilityLabel>,
            )>,
        ),
    >,
) {
    for (text, explicit, mut accessible) in &mut buttons {
        accessible.set_label(explicit.map(|label| &label.0).unwrap_or(&text.0).clone());
    }
}
