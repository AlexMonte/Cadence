//! A complete preview cycle, with note positions and lengths scaled to its lane.

use bevy::{picking::prelude::*, prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use super::InspectorButtonAction;
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::widgets::musaic_button;
use crate::{
    application::command::EditorCommand,
    application::pipeline::runtime::TimelineEventKind,
    application::pipeline::ui_projection::{EditorUiProjection, TimelinePaint},
};

const TRACK_LABEL_WIDTH: f32 = 72.0;
const TRACK_ROW_HEIGHT: f32 = 40.0;

#[derive(Component)]
pub(crate) struct UiTimelineContent;
#[derive(Component)]
pub(crate) struct TimelinePlayhead;
#[derive(Component)]
pub(crate) struct UiTimelineEvents;
#[derive(Component)]
pub(crate) struct TimelineHeaderLabel(bool);
#[derive(Component, Clone, Copy)]
pub(crate) struct PreviewControl(i8);

impl PreviewControl {
    fn command(self, paint: &TimelinePaint) -> EditorCommand {
        let cycle = paint.window_start.max(0.0) as u32;
        let max = crate::application::editor::TimelinePanelState::MAX_PREVIEW_CYCLE;
        EditorCommand::PreviewCycle {
            cycle: match self.0 {
                -1 => Some(cycle.saturating_sub(1)),
                1 => Some(cycle.saturating_add(1).min(max)),
                _ => None,
            },
        }
    }
}

fn header_title(paint: &TimelinePaint) -> String {
    paint
        .pending_cycle
        .map(|cycle| format!("Pattern timing · edit queued for cycle {:.0}", cycle + 1.0))
        .unwrap_or_else(|| "Pattern timing".into())
}
fn cycle_label(paint: &TimelinePaint) -> String {
    format!(
        "Cycle {:.0}{}",
        paint.window_start + 1.0,
        if paint.browsing { " · Preview" } else { "" }
    )
}

pub(crate) fn spawn_timeline_content(
    band: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    paint: &TimelinePaint,
) {
    band.spawn(Node {
        width: percent(100),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        row_gap: px(6.0),
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|header| {
        header.spawn((
            TimelineHeaderLabel(true),
            UiText::new(header_title(paint)),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            ThemedText,
            TextColor(theme.chrome.text_main),
        ));
        header
            .spawn(Node {
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6.0),
                row_gap: px(6.0),
                ..default()
            })
            .with_children(|controls| {
                controls.spawn((
                    TimelineHeaderLabel(false),
                    UiText::new(cycle_label(paint)),
                    Node {
                        min_width: px(126.0),
                        ..default()
                    },
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_dim),
                ));
                for (label, control) in [
                    ("Previous cycle", PreviewControl(-1)),
                    ("Next cycle", PreviewControl(1)),
                    ("Follow playback", PreviewControl(0)),
                ] {
                    controls
                        .spawn((
                            musaic_button(
                                Node {
                                    height: px(34.0),
                                    padding: UiRect::horizontal(px(12.0)),
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(px(1.0)),
                                    border_radius: BorderRadius::all(px(theme.radii.md)),
                                    ..default()
                                },
                                (control, InspectorButtonAction(control.command(paint))),
                                label,
                            ),
                            BackgroundColor(theme.chrome.button_bg),
                            BorderColor::all(theme.chrome.button_border),
                        ))
                        .observe(super::on_inspector_button_activated);
                }
            });
    });
    band.spawn((
        UiTimelineEvents,
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(6.0),
            ..default()
        },
    ))
    .with_children(|events| spawn_timeline_events(events, theme, paint));
}

fn spawn_timeline_events(
    band: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    paint: &TimelinePaint,
) {
    band.spawn(Node {
        height: px(18.0),
        width: percent(100),
        padding: UiRect::left(px(TRACK_LABEL_WIDTH + 20.0)),
        ..default()
    })
    .with_children(|ruler| {
        ruler
            .spawn(Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Relative,
                ..default()
            })
            .with_children(|ticks| {
                let beats = paint.beats_per_cycle.max(1);
                let step = (beats / 16).max(1);
                for beat in (0..beats).step_by(step as usize) {
                    ticks.spawn((
                        UiText::new((beat + 1).to_string()),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.chrome.text_dim),
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(beat as f32 / beats as f32 * 100.0),
                            ..default()
                        },
                    ));
                }
            });
    });
    band.spawn((
        Node {
            flex_grow: 1.0,
            width: percent(100),
            min_height: px(56.0),
            position_type: PositionType::Relative,
            overflow: Overflow::clip(),
            border_radius: BorderRadius::all(px(3.0)),
            ..default()
        },
        BackgroundColor(theme.chrome.section_tint),
    ))
    .with_children(|viewport| {
        viewport
            .spawn(Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.0),
                padding: UiRect::all(px(6.0)),
                ..default()
            })
            .with_children(|tracks| spawn_timeline_tracks(tracks, theme, paint));
    });
}

fn spawn_timeline_tracks(
    tracks_root: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    paint: &TimelinePaint,
) {
    let mut tracks: std::collections::BTreeMap<&str, Vec<_>> = std::collections::BTreeMap::new();
    for event in &paint.events {
        tracks.entry(&event.output_id).or_default().push(event);
    }
    if tracks.is_empty() {
        tracks_root.spawn((
            UiText::new("Connect a pattern to an output to see its notes."),
            TextColor(theme.chrome.text_dim),
            ThemedText,
        ));
        return;
    }
    for (output, events) in tracks {
        tracks_root
            .spawn(Node {
                width: percent(100),
                height: px(TRACK_ROW_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8.0),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        width: px(TRACK_LABEL_WIDTH),
                        flex_shrink: 0.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    UiText::new(
                        paint
                            .output_names
                            .get(output)
                            .map(String::as_str)
                            .unwrap_or("Output"),
                    ),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    ThemedText,
                    TextColor(theme.chrome.text_main),
                ));
                row.spawn(Node {
                    flex_grow: 1.0,
                    height: percent(100),
                    min_width: px(0.0),
                    position_type: PositionType::Relative,
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|lane| {
                    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                        lane.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: percent(fraction * 100.0),
                                top: px(0.0),
                                bottom: px(0.0),
                                width: px(1.0),
                                ..default()
                            },
                            BackgroundColor(theme.chrome.border),
                            Pickable::IGNORE,
                        ));
                    }
                    for event in events {
                        let Some((left, width)) =
                            event_extent(paint, event.visible_start, event.visible_end)
                        else {
                            continue;
                        };
                        let (top, height, color) = match event.kind {
                            TimelineEventKind::StartVoice => {
                                (3.0, 30.0, Color::srgb(0.886, 0.933, 0.898))
                            }
                            TimelineEventKind::ControlUpdate => {
                                (27.0, 4.0, theme.semantic.atom_scalar)
                            }
                        };
                        lane.spawn((
                            musaic_button(
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: percent(left),
                                    width: percent(width),
                                    top: px(top),
                                    height: px(height),
                                    border: UiRect::horizontal(px(1.5)),
                                    border_radius: BorderRadius::all(px(3.0)),
                                    padding: UiRect::horizontal(px(4.0)),
                                    align_items: AlignItems::Center,
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                                InspectorButtonAction(EditorCommand::JumpToTimelineSource {
                                    event: event.id,
                                }),
                                if width >= 4.0 {
                                    event.label.as_str()
                                } else {
                                    ""
                                },
                            ),
                            crate::infrastructure::ui::widgets::ButtonAccessibilityLabel(format!(
                                "{} at cycle {:.3}: show source tile",
                                event.label,
                                event.visible_start + 1.0
                            )),
                            BackgroundColor(color),
                            BorderColor::all(theme.chrome.panel_inset),
                        ))
                        .observe(super::on_inspector_button_activated);
                    }
                    lane.spawn((
                        TimelinePlayhead,
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(0.0),
                            top: px(0.0),
                            bottom: px(0.0),
                            width: px(2.0),
                            ..default()
                        },
                        BackgroundColor(theme.chrome.text_main),
                        ZIndex(2),
                        Pickable::IGNORE,
                    ));
                });
            });
    }
}

/// Clip before scaling so every event is contained in the preview's exact
/// window. Durations retain their proportions; short notes get no fake length.
fn event_extent(paint: &TimelinePaint, start: f64, end: f64) -> Option<(f32, f32)> {
    let duration = paint.window_end - paint.window_start;
    if !duration.is_finite() || duration <= 0.0 || !start.is_finite() || !end.is_finite() {
        return None;
    }
    let start = start.max(paint.window_start);
    let end = end.min(paint.window_end);
    if end <= start {
        return None;
    }
    Some((
        ((start - paint.window_start) / duration * 100.0) as f32,
        ((end - start) / duration * 100.0) as f32,
    ))
}

pub(crate) fn sync_timeline_content(
    mut commands: Commands,
    theme: Res<'_, MusaicUiTheme>,
    projection: Res<'_, EditorUiProjection>,
    dirty: Res<'_, crate::application::pipeline::ui_projection::UiDirty>,
    content_roots: Query<Entity, With<UiTimelineEvents>>,
    mut labels: Query<(&TimelineHeaderLabel, &mut UiText)>,
    mut controls: Query<(&PreviewControl, &mut InspectorButtonAction)>,
    mut cache: ResMut<'_, TimelineContentCache>,
) {
    if !dirty.timeline && !cache.force_next {
        return;
    }
    cache.force_next = false;
    let paint = &projection.timeline_paint;
    for (label, mut text) in &mut labels {
        let value = if label.0 {
            header_title(paint)
        } else {
            cycle_label(paint)
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (control, mut action) in &mut controls {
        action.0 = control.command(paint);
    }
    for root in &content_roots {
        commands.entity(root).despawn_children();
        commands
            .entity(root)
            .with_children(|band| spawn_timeline_events(band, &theme, paint));
    }
}

#[derive(Resource, Default)]
pub(crate) struct TimelineContentCache {
    pub force_next: bool,
}

pub(crate) fn sync_timeline_playhead(
    clock: Res<'_, crate::application::editor::transport::TransportClock>,
    projection: Res<'_, EditorUiProjection>,
    mut playheads: Query<(&mut Node, &mut Visibility), With<TimelinePlayhead>>,
) {
    let paint = &projection.timeline_paint;
    let duration = paint.window_end - paint.window_start;
    let progress = if duration > 0.0 {
        ((clock.position.value() - paint.window_start) / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for (mut node, mut visibility) in &mut playheads {
        node.left = percent((progress * 100.0) as f32);
        *visibility = if clock.position.value() >= paint.window_start
            && clock.position.value() < paint.window_end
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_refresh_keeps_navigation_buttons_and_their_keyboard_focus() {
        let mut app = App::new();
        app.insert_resource(MusaicUiTheme::default_dark())
            .init_resource::<EditorUiProjection>()
            .init_resource::<crate::application::pipeline::ui_projection::UiDirty>()
            .init_resource::<TimelineContentCache>()
            .init_resource::<bevy::input_focus::InputFocus>()
            .add_systems(
                Startup,
                |mut commands: Commands, theme: Res<MusaicUiTheme>| {
                    commands
                        .spawn((UiTimelineContent, Node::default()))
                        .with_children(|band| {
                            spawn_timeline_content(band, &theme, &TimelinePaint::default());
                        });
                },
            )
            .add_systems(Update, sync_timeline_content);
        app.update();
        let buttons: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<PreviewControl>>()
            .iter(app.world())
            .collect();
        assert_eq!(buttons.len(), 3);
        app.world_mut()
            .resource_mut::<bevy::input_focus::InputFocus>()
            .0 = Some(buttons[1]);
        for cycle in [1.0, 8.0, 123.0] {
            app.world_mut()
                .resource_mut::<EditorUiProjection>()
                .timeline_paint
                .window_start = cycle;
            app.world_mut()
                .resource_mut::<crate::application::pipeline::ui_projection::UiDirty>()
                .timeline = true;
            app.update();
            for entity in &buttons {
                let control = app
                    .world()
                    .get::<PreviewControl>(*entity)
                    .expect("button stays alive");
                assert_eq!(
                    app.world().get::<InspectorButtonAction>(*entity).unwrap().0,
                    control.command(&app.world().resource::<EditorUiProjection>().timeline_paint)
                );
            }
            assert_eq!(
                app.world().resource::<bevy::input_focus::InputFocus>().0,
                Some(buttons[1])
            );
        }
    }

    #[test]
    fn preview_positions_are_relative_to_the_window_and_preserve_short_lengths() {
        let paint = TimelinePaint {
            window_start: 7.0,
            window_end: 8.0,
            ..default()
        };
        assert_eq!(event_extent(&paint, 7.25, 7.5), Some((25.0, 25.0)));
        assert_eq!(event_extent(&paint, 7.5, 7.5625), Some((50.0, 6.25)));
        assert_eq!(event_extent(&paint, 6.5, 7.25), Some((0.0, 25.0)));
        assert_eq!(event_extent(&paint, 7.75, 8.5), Some((75.0, 25.0)));
        assert_eq!(event_extent(&paint, 8.1, 8.3), None);
    }
}
