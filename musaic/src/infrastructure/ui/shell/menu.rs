//! In-editor top chrome (Play / Tiles) and the shell command-button channel.
//!
//! This is **not** the boot MainMenu screen — see
//! [`crate::infrastructure::ui::screens::main_menu`].
//!
//! Any UI button carrying [`MenuBarButtonAction`] or [`InspectorButtonAction`]
//! dispatches its [`EditorCommand`] on the bus when activated.

use crate::infrastructure::ui::widgets::exact_number::{self, ExactNumber};
use bevy::input_focus::InputFocus;
use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_ui_widgets::{Activate, ValueChange};
use tessera::prelude::Rational;

use crate::application::command::{EditorCommand, EditorCommandBus};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::widgets::{PanelBackdrop, musaic_button, spawn_shell_panel};

const TOP_MENU_HEIGHT: f32 = 68.0;

#[derive(Component)]
struct UiShellMenu;

#[derive(Component)]
pub(crate) struct MenuProjectLabel;

#[derive(Component)]
pub(crate) struct MenuClockLabel;

#[derive(Component)]
pub(crate) struct MenuAudioLabel;

#[derive(Component, Clone, Copy)]
pub(crate) enum MenuParameter {
    Tempo,
    BeatsPerCycle,
}

#[derive(Component, Clone)]
pub(crate) struct MenuBarButtonAction(EditorCommand);

#[derive(Component)]
pub(crate) struct MenuTransportLabel;

#[derive(Component)]
pub(crate) struct MenuTransportStatus;

/// Command chip used across inspector, breadcrumbs, minimap, and timeline UI.
#[derive(Component, Clone)]
pub(crate) struct InspectorButtonAction(pub(crate) EditorCommand);

pub(crate) fn spawn_top_menu(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
) {
    spawn_shell_panel(
        parent,
        (
            UiShellMenu,
            super::super::keyboard_regions::KeyboardRegion(
                super::super::keyboard_regions::RegionKind::Toolbar,
            ),
            bevy::input_focus::tab_navigation::TabIndex(-1),
            Node {
                width: percent(100),
                min_height: px(TOP_MENU_HEIGHT),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(18.0),
                row_gap: px(8.0),
                padding: UiRect::axes(px(22.0), px(12.0)),
                border: UiRect::bottom(px(1.0)),
                ..default()
            },
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.border),
        ),
        theme,
        images,
        sprites,
        PanelBackdrop::None,
        |menu| {
            menu.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(14.0),
                min_width: px(0),
                ..default()
            })
            .with_children(|project| {
                project
                    .spawn(Node {
                        width: px(19.0),
                        height: px(19.0),
                        flex_shrink: 0.0,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(3.0),
                        row_gap: px(3.0),
                        ..default()
                    })
                    .with_children(|mark| {
                        for index in 0..4 {
                            mark.spawn((
                                Node {
                                    width: px(8.0),
                                    height: px(8.0),
                                    border_radius: BorderRadius::all(px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(theme.chrome.accent.with_alpha(if index == 1 {
                                    0.45
                                } else {
                                    1.0
                                })),
                            ));
                        }
                    });
                project.spawn((
                    UiText::new("Musaic"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_main),
                ));
                super::dropdown::spawn(project, theme);
            });
            menu.spawn((
                MenuProjectLabel,
                UiText::new("Untitled"),
                TextLayout::new_with_no_wrap(),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(90),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ));
            menu.spawn(Node {
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(8.0),
                row_gap: px(8.0),
                ..default()
            })
            .with_children(|transport| {
                for (label, command) in [
                    ("Play", EditorCommand::TransportToggle),
                    ("Stop", EditorCommand::TransportStop),
                    ("Panic", EditorCommand::TransportPanic),
                ] {
                    spawn_menu_chip(transport, theme, label, command);
                }
                transport.spawn((
                    MenuTransportStatus,
                    UiText::new("Stopped"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_dim),
                    Node {
                        min_width: px(48),
                        ..default()
                    },
                ));
                transport.spawn((
                    Node {
                        width: px(1.0),
                        height: px(22.0),
                        margin: UiRect::horizontal(px(3.0)),
                        ..default()
                    },
                    BackgroundColor(theme.chrome.border),
                ));
                spawn_transport_parameter(transport, theme, MenuParameter::Tempo, "BPM");
                spawn_transport_parameter(
                    transport,
                    theme,
                    MenuParameter::BeatsPerCycle,
                    "beats / cycle",
                );
            });
        },
    );
}

fn spawn_transport_parameter(
    menu: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    parameter: MenuParameter,
    label: &str,
) {
    let (value, accessible_label) = match parameter {
        MenuParameter::Tempo => (120, "Tempo in beats per minute"),
        MenuParameter::BeatsPerCycle => (4, "Beats per cycle"),
    };
    menu.spawn(Node {
        flex_shrink: 0.0,
        align_items: AlignItems::Center,
        column_gap: px(3),
        ..default()
    })
    .with_children(|control| {
        let field = exact_number::spawn(control, Rational::new(value, 1), parameter);
        let mut accessible = accesskit::Node::new(accesskit::Role::TextInput);
        accessible.set_label(accessible_label);
        control
            .commands()
            .entity(field)
            .insert((
                Name::new(accessible_label),
                bevy::a11y::AccessibilityNode(accessible),
                Node {
                    min_width: px(30),
                    max_width: px(76),
                    min_height: px(18),
                    padding: UiRect::axes(px(5), px(0)),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
            ))
            .observe(on_transport_parameter_changed);
        control.spawn((
            UiText::new(label),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(theme.chrome.text_dim),
        ));
    });
}

fn parameter_command(
    parameter: MenuParameter,
    value: Rational,
) -> Result<EditorCommand, &'static str> {
    let number = value.numerator as f64 / value.denominator as f64;
    match parameter {
        MenuParameter::Tempo if number.is_finite() && (1.0..=999.0).contains(&number) => {
            Ok(EditorCommand::SetBpm {
                bpm: (number * 10.0).round() / 10.0,
            })
        }
        MenuParameter::Tempo => Err("BPM must be 1–999"),
        MenuParameter::BeatsPerCycle
            if value.denominator == 1 && (1..=64).contains(&value.numerator) =>
        {
            Ok(EditorCommand::SetBeatsPerCycle {
                beats: value.numerator as u32,
            })
        }
        MenuParameter::BeatsPerCycle => Err("Use 1–64 whole beats"),
    }
}

fn on_transport_parameter_changed(
    event: On<ValueChange<Rational>>,
    parameters: Query<&MenuParameter>,
    mut fields: Query<&mut Text, With<ExactNumber>>,
    mut focus: ResMut<InputFocus>,
    mut commands: MessageWriter<EditorCommandBus>,
) {
    let Ok(parameter) = parameters.get(event.source) else {
        return;
    };
    match parameter_command(*parameter, event.value) {
        Ok(command) => {
            commands.write(EditorCommandBus(command));
        }
        Err(error) => {
            if let Ok(mut text) = fields.get_mut(event.source) {
                text.0 = error.into();
                focus.set(event.source);
            }
        }
    }
}

fn spawn_menu_chip(
    menu: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    label: &str,
    action: EditorCommand,
) {
    let is_transport = matches!(action, EditorCommand::TransportToggle);
    let mut button = menu.spawn(musaic_button(
        Node {
            min_width: px(44.0),
            height: px(34.0),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(10.0), px(5.0)),
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(6.0)),
            ..default()
        },
        MenuBarButtonAction(action),
        label,
    ));
    button.insert((
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(theme.chrome.text_main),
        BackgroundColor(if is_transport {
            theme.chrome.crumb_selected_bg
        } else {
            theme.chrome.button_bg
        }),
        BorderColor::all(theme.chrome.button_border),
    ));
    if is_transport {
        button.insert(MenuTransportLabel);
    }
    button.observe(on_menu_bar_button_activated);
}

pub(crate) fn sync_menu_transport_label(
    transport: Res<State<crate::infrastructure::app::TransportMode>>,
    project: Res<crate::application::session::MusaicProject>,
    clock: Res<crate::application::editor::transport::TransportClock>,
    audio: Option<Res<crate::adapter::audio::AudioDeviceStatus>>,
    #[cfg(not(target_arch = "wasm32"))] recovery: Option<
        Res<crate::adapter::persistence::recovery::RecoveryStatus>,
    >,
    focus: Res<InputFocus>,
    mut labels: ParamSet<(
        Query<&mut crate::infrastructure::ui::widgets::ButtonLabel, With<MenuTransportLabel>>,
        Query<&mut Text, With<MenuProjectLabel>>,
        Query<&mut Text, With<MenuClockLabel>>,
        Query<&mut Text, With<MenuAudioLabel>>,
        Query<(Entity, &MenuParameter, &mut ExactNumber, &mut Text)>,
        Query<&mut Text, With<MenuTransportStatus>>,
    )>,
) {
    let label = match transport.get() {
        crate::infrastructure::app::TransportMode::Playing => "Pause",
        crate::infrastructure::app::TransportMode::Paused => "Resume",
        crate::infrastructure::app::TransportMode::Stopped => "Play",
    };
    for mut text in labels.p0().iter_mut() {
        if text.0 != label {
            text.0 = label.into();
        }
    }
    let project_label = format!(
        "{}{}",
        project.metadata.display_name,
        if project.metadata.dirty {
            " · Unsaved"
        } else {
            ""
        }
    );
    #[cfg(not(target_arch = "wasm32"))]
    let project_label = if recovery
        .as_deref()
        .is_some_and(|status| status.error.is_some())
    {
        "Recovery failed · Save project".into()
    } else {
        project_label
    };
    for mut text in labels.p1().iter_mut() {
        if text.0 != project_label {
            text.0.clone_from(&project_label);
        }
    }
    let clock_label = format!("Cycle {:.1}", clock.position.value() + 1.0);
    for mut text in labels.p2().iter_mut() {
        if text.0 != clock_label {
            text.0.clone_from(&clock_label);
        }
    }
    let audio_label = match audio.as_deref() {
        Some(status) if status.error.is_some() => "Audio unavailable".to_owned(),
        Some(status) if status.rejected_commands > 0 || status.resource_limit_hits > 0 => {
            "Audio at capacity".to_owned()
        }
        Some(status) if status.late_commands > 0 => "Audio timing late".to_owned(),
        Some(status) => status
            .sample_rate
            .map(|rate| format!("Audio · {:.1} kHz", rate as f64 / 1000.0))
            .unwrap_or_else(|| "Audio starting".into()),
        None => "Audio unavailable".to_owned(),
    };
    for mut text in labels.p3().iter_mut() {
        if text.0 != audio_label {
            text.0.clone_from(&audio_label);
        }
    }
    for (entity, parameter, mut field, mut text) in &mut labels.p4() {
        let (value, display) = match parameter {
            MenuParameter::Tempo => (
                Rational::new((clock.bpm * 10.0).round() as i64, 10),
                if clock.bpm.fract().abs() < 0.0001 {
                    format!("{:.0}", clock.bpm)
                } else {
                    format!("{:.1}", clock.bpm)
                },
            ),
            MenuParameter::BeatsPerCycle => (
                Rational::new(i64::from(clock.beats_per_cycle), 1),
                clock.beats_per_cycle.to_string(),
            ),
        };
        let focused = focus.0 == Some(entity);
        exact_number::update(&mut field, &mut text, value, focused);
        if !focused && text.0 != display {
            text.0 = display;
        }
    }
    let state = match transport.get() {
        crate::infrastructure::app::TransportMode::Playing => "Playing",
        crate::infrastructure::app::TransportMode::Paused => "Paused",
        crate::infrastructure::app::TransportMode::Stopped => "Stopped",
    };
    for mut text in &mut labels.p5() {
        if text.0 != state {
            text.0 = state.into();
        }
    }
}

fn on_menu_bar_button_activated(
    activate: On<'_, '_, Activate>,
    query: Query<'_, '_, &MenuBarButtonAction>,
    mut command_bus: MessageWriter<'_, EditorCommandBus>,
) {
    let Ok(MenuBarButtonAction(command)) = query.get(activate.entity) else {
        return;
    };
    command_bus.write(EditorCommandBus(command.clone()));
}

pub(crate) fn on_inspector_button_activated(
    activate: On<'_, '_, Activate>,
    query: Query<'_, '_, &InspectorButtonAction>,
    mut writer: MessageWriter<'_, EditorCommandBus>,
) {
    let Ok(action) = query.get(activate.entity) else {
        return;
    };

    writer.write(EditorCommandBus(action.0.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_entry_accepts_decimal_precision_and_rejects_invalid_ranges() {
        assert_eq!(
            parameter_command(MenuParameter::Tempo, Rational::new(241, 2)),
            Ok(EditorCommand::SetBpm { bpm: 120.5 })
        );
        for value in [
            Rational::new(0, 1),
            Rational::new(-2, 1),
            Rational::new(1000, 1),
        ] {
            assert!(parameter_command(MenuParameter::Tempo, value).is_err());
        }
    }

    #[test]
    fn cycle_entry_rejects_fractional_beats_instead_of_silently_rounding() {
        assert_eq!(
            parameter_command(MenuParameter::BeatsPerCycle, Rational::new(7, 1)),
            Ok(EditorCommand::SetBeatsPerCycle { beats: 7 })
        );
        for value in [
            Rational::new(7, 2),
            Rational::new(0, 1),
            Rational::new(65, 1),
        ] {
            assert!(parameter_command(MenuParameter::BeatsPerCycle, value).is_err());
        }
    }
}
