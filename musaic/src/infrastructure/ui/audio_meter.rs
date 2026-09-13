//! Retained delivery meter and a read-only, dismissible silence check.
use super::{
    theme::MusaicUiTheme,
    widgets::{musaic_chrome_button, spawn_dialog_overlay},
};
use crate::{
    adapter::audio::{
        AudioDeviceStatus,
        meter::{ClearMasterClip, MasterMeter},
    },
    application::{
        audio_checks,
        command::{EditorCommand, EditorCommandBus},
        editor::SelectionMode,
        pipeline::runtime::RuntimePreviewSnapshot,
        session::MusaicProject,
    },
    infrastructure::app::{AppState, MusaicSet},
};
use bevy::prelude::*;
use bevy_ui_widgets::Activate;

#[derive(Component)]
enum MeterText {
    Footer,
    Help,
}
#[derive(Component)]
struct Channel(usize);
#[derive(Component)]
struct ClipReset;
#[derive(Component)]
struct AudioHelp;
#[derive(Component)]
struct CheckList;
#[derive(Component)]
struct InspectCheck(audio_checks::AudioCheck);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (paint, sync_checks)
            .in_set(MusaicSet::RenderUi)
            .after(super::shell::rebuild::rebuild_ui)
            .run_if(in_state(AppState::Editor)),
    )
    .add_systems(
        PreUpdate,
        escape.after(bevy::input_focus::InputFocusSystems::Dispatch),
    );
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    parent.spawn(Node { align_items: AlignItems::Center, column_gap: px(7), ..default() })
        .with_children(|meter| {
            let mut accessible = accesskit::Node::new(accesskit::Role::Label);
            accessible.set_description("Master sample peaks sent to the audio device, after channel mapping and before clipping. Values are in decibels relative to full scale.");
            meter.spawn((MeterText::Footer, Text::new("Master —"), TextFont { font_size: 11.0, ..default() }, TextColor(theme.chrome.text_dim),
                bevy::a11y::AccessibilityNode(accessible), Node { width: px(180), flex_shrink: 0.0, ..default() }));
            meter.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), ..default() }).with_children(|bars| {
                for index in 0..2 {
                    bars.spawn((Node { width: px(64), height: px(4), overflow: Overflow::clip(), ..default() }, BackgroundColor(theme.chrome.border)))
                        .with_children(|track| {
                            track.spawn((Channel(index), Node { width: percent(0), height: percent(100), ..default() }, BackgroundColor(theme.chrome.accent)));
                        });
                }
            });
            meter.spawn(musaic_chrome_button(theme, "No sound?", ()))
                .insert((compact_button(theme, Display::Flex), TextFont { font_size: 11.0, ..default() }))
                .observe(open);
            meter.spawn(musaic_chrome_button(theme, "Clip · reset", ClipReset))
                .insert((compact_button(theme, Display::None), TextFont { font_size: 11.0, ..default() }))
                .observe(reset_clip);

        });
}

fn compact_button(theme: &MusaicUiTheme, display: Display) -> Node {
    Node {
        display,
        min_height: px(23),
        padding: UiRect::axes(px(7), px(3)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(theme.radii.md)),
        ..default()
    }
}

fn reset_clip(_: On<Activate>, mut messages: MessageWriter<ClearMasterClip>) {
    messages.write(ClearMasterClip);
}

fn level(value: f32) -> String {
    MasterMeter::db(value)
        .map(|db| format!("{}", db.round() as i32))
        .unwrap_or_else(|| "−∞".into())
}

fn paint(
    meter: Option<Res<MasterMeter>>,
    device: Option<Res<AudioDeviceStatus>>,
    snapshot: Res<RuntimePreviewSnapshot>,
    theme: Res<MusaicUiTheme>,
    mut labels: Query<(
        &MeterText,
        &mut Text,
        Option<&mut bevy::a11y::AccessibilityNode>,
    )>,
    mut bars: Query<(&Channel, &mut Node, &mut BackgroundColor), Without<ClipReset>>,
    mut clips: Query<(&mut Node, &mut BackgroundColor), (With<ClipReset>, Without<Channel>)>,
) {
    let receiving = meter.as_deref().is_some_and(|m| m.receiving)
        && device.as_deref().is_none_or(|d| d.error.is_none());
    let peaks = meter
        .as_deref()
        .filter(|_| receiving)
        .map(|m| [m.left, m.right])
        .unwrap_or([0.0; 2]);
    let clipped = meter.as_deref().is_some_and(|m| m.clipped);
    let footer = if receiving {
        format!("Master L {} · R {} dBFS", level(peaks[0]), level(peaks[1]))
    } else {
        "Master · no device data".into()
    };
    for (kind, mut text, accessible) in &mut labels {
        let value = match kind {
            MeterText::Footer => footer.clone(),
            MeterText::Help => {
                let device_text = match device.as_deref() {
                    Some(d) if d.error.is_some() => {
                        format!("Device unavailable · {}", d.error.as_deref().unwrap())
                    }
                    Some(d) if receiving => format!(
                        "Device receiving · {} Hz · {} channels",
                        d.sample_rate.unwrap_or_default(),
                        d.channels.unwrap_or_default()
                    ),
                    _ => "No current device data · output may be starting or unavailable".into(),
                };
                let delivery = if receiving && peaks.into_iter().any(|v| v > 0.000_001) {
                    "Samples are reaching the device. If you hear nothing, check system volume, the selected device and speakers/headphones."
                } else if receiving && snapshot.feedback.playing {
                    "The device is receiving silence. Check the route, rests, Gate and Gain tiles, and the instrument's sound."
                } else if receiving {
                    "The device is receiving silence. Press Play to check your loop."
                } else {
                    "A moving playhead cannot confirm sound delivery. Check the audio device first."
                };
                let health = device.as_deref().filter(|d| d.underrun_frames > 0 || d.rejected_commands > 0 || d.resource_limit_hits > 0)
                    .map(|d| format!("\nDelivery history · {} missing frames · {} rejected commands · {} capacity hits", d.underrun_frames, d.rejected_commands, d.resource_limit_hits)).unwrap_or_default();
                format!(
                    "{device_text}\n{}\n{footer}\n{delivery}{}{}",
                    snapshot.feedback.summary(),
                    if clipped {
                        "\nClipping detected · reduce Gain or instrument volume. Reset the indicator to check again."
                    } else {
                        ""
                    },
                    health
                )
            }
        };
        if text.0 != value {
            text.0.clone_from(&value);
            if let Some(mut accessible) = accessible {
                accessible.set_label(value);
            }
        }
    }
    for (channel, mut node, mut color) in &mut bars {
        let db = MasterMeter::db(peaks[channel.0]).unwrap_or(-60.0);
        let width = percent(((db + 60.0) / 60.0 * 100.0).clamp(0.0, 100.0));
        if node.width != width {
            node.width = width;
        }
        let next = if peaks[channel.0] > 1.0 {
            theme.chrome.danger
        } else {
            theme.chrome.accent
        };
        if color.0 != next {
            color.0 = next;
        }
    }
    for (mut node, mut color) in &mut clips {
        let display = if clipped {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        if color.0 != theme.chrome.diagnostics_bg {
            color.0 = theme.chrome.diagnostics_bg;
        }
    }
}

fn open(
    _: On<Activate>,
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
    existing: Query<(), With<AudioHelp>>,
) {
    if !existing.is_empty() {
        return;
    }
    spawn_dialog_overlay(
        &mut commands,
        &theme,
        (AudioHelp, DespawnOnExit(AppState::Editor)),
        |card| {
            card.spawn((
                Text::new("Audio output"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            card.spawn((
                MeterText::Help,
                Node {
                    max_width: px(580),
                    ..default()
                },
                Text::new("Checking audio…"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            card.spawn((Node { max_width: px(580), ..default() }, Text::new("Project checks · these tiles may be intentional or unused. Select a check to inspect its source."), TextFont { font_size: 13.0, ..default() }, TextColor(theme.chrome.text_dim)));
            card.spawn((
                CheckList,
                Node {
                    max_width: px(580),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    max_height: px(180),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ));
            card.spawn((Node { max_width: px(580), ..default() }, Text::new("0 dBFS is full scale. The bars show sample peaks with a short decay; Clip stays on until reset. They include auditions and effect tails. This meter does not measure speaker volume."), TextFont { font_size: 12.0, ..default() }, TextColor(theme.chrome.text_dim)));
            card.spawn(musaic_chrome_button(&theme, "Close", ()))
                .observe(close);
        },
    );
}

fn sync_checks(
    mut commands: Commands,
    project: Res<MusaicProject>,
    theme: Res<MusaicUiTheme>,
    lists: Query<(Entity, Ref<CheckList>)>,
) {
    for (entity, list) in &lists {
        if !list.is_added() && !project.is_changed() {
            continue;
        }
        let checks = audio_checks::inspect(&project);
        commands.entity(entity).despawn_children().with_children(|list| {
            if checks.outputs == 0 {
                list.spawn((Text::new("No output tiles · connect a pattern and instrument to an output."), TextFont { font_size: 13.0, ..default() }, TextColor(theme.chrome.text_main)));
            } else if checks.issues.is_empty() {
                list.spawn((Text::new("No disconnected outputs, closed Gate tiles, zero levels or unavailable instrument samples found. Also inspect connected control values and rests."), TextFont { font_size: 13.0, ..default() }, TextColor(theme.chrome.text_main)));
            }
            for check in checks.issues {
                list.spawn(musaic_chrome_button(&theme, check.detail.clone(), InspectCheck(check))).observe(inspect);
            }
        });
    }
}
fn inspect(
    event: On<Activate>,
    checks: Query<&InspectCheck>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut commands: Commands,
    dialogs: Query<Entity, With<AudioHelp>>,
) {
    let Ok(InspectCheck(check)) = checks.get(event.entity) else {
        return;
    };
    bus.write(EditorCommandBus(EditorCommand::NavigateToSurface {
        surface: check.surface,
    }));
    bus.write(EditorCommandBus(EditorCommand::SelectNode {
        node: check.node.clone(),
        mode: SelectionMode::Replace,
    }));
    for entity in &dialogs {
        commands.entity(entity).despawn();
    }
}
fn close(_: On<Activate>, mut commands: Commands, dialogs: Query<Entity, With<AudioHelp>>) {
    for entity in &dialogs {
        commands.entity(entity).despawn();
    }
}
fn escape(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    dialogs: Query<Entity, With<AudioHelp>>,
) {
    if !dialogs.is_empty() && keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        for entity in &dialogs {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clip_control_is_retained_resets_by_message_and_device_error_never_claims_signal() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<MasterMeter>()
            .init_resource::<AudioDeviceStatus>()
            .init_resource::<RuntimePreviewSnapshot>()
            .init_resource::<MusaicUiTheme>()
            .add_message::<ClearMasterClip>()
            .add_systems(
                Startup,
                |mut commands: Commands, theme: Res<MusaicUiTheme>| {
                    commands
                        .spawn_empty()
                        .with_children(|parent| spawn(parent, &theme));
                },
            )
            .add_systems(Update, paint);
        app.update();
        let clip = app
            .world_mut()
            .query_filtered::<Entity, With<ClipReset>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(clip).unwrap().display,
            Display::None
        );
        {
            let mut meter = app.world_mut().resource_mut::<MasterMeter>();
            meter.receiving = true;
            meter.left = 2.0;
            meter.right = 0.5;
            meter.clipped = true;
        }
        app.update();
        assert_eq!(
            app.world().get::<Node>(clip).unwrap().display,
            Display::Flex
        );
        let levels = app
            .world_mut()
            .query::<(&MeterText, &Text)>()
            .iter(app.world())
            .find(|(kind, _)| matches!(kind, MeterText::Footer))
            .unwrap()
            .1
            .0
            .clone();
        assert_eq!(levels, "Master L 6 · R -6 dBFS");
        app.world_mut().trigger(Activate { entity: clip });
        assert_eq!(app.world().resource::<Messages<ClearMasterClip>>().len(), 1);
        app.world_mut().resource_mut::<MasterMeter>().clipped = false;
        app.world_mut().resource_mut::<AudioDeviceStatus>().error = Some("Device stopped".into());
        app.update();
        assert_eq!(
            app.world().get::<Node>(clip).unwrap().display,
            Display::None
        );
        let levels = app
            .world_mut()
            .query::<(&MeterText, &Text)>()
            .iter(app.world())
            .find(|(kind, _)| matches!(kind, MeterText::Footer))
            .unwrap()
            .1
            .0
            .clone();
        assert_eq!(levels, "Master · no device data");
        for (_, node, _) in app
            .world_mut()
            .query::<(&Channel, &Node, &BackgroundColor)>()
            .iter(app.world())
        {
            assert_eq!(node.width, percent(0));
        }
    }
}
