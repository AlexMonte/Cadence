//! Piano-roll timeline band with drag-to-reveal height and drag-to-pan tracks.

use bevy::{picking::prelude::*, prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use crate::{
    application::command::EditorCommand,
    application::pipeline::runtime::{RuntimePreviewSnapshot, TimelineEventKind},
};

use super::{
    InspectorButtonAction, artist_palette::ArtistPalette, controls::musaic_clickable,
    tile_shell::PANEL_INSET_BG,
};

const TRACK_LABEL_WIDTH: f32 = 72.0;
const TRACK_ROW_HEIGHT: f32 = 28.0;
const EVENT_MIN_WIDTH: f32 = 18.0;
const EVENT_MAX_WIDTH: f32 = 96.0;
const TIMELINE_CONTENT_WIDTH: f32 = 720.0;
const CYCLE_SCALE: f32 = 48.0;

#[derive(Component)]
pub struct UiTimelineContent;

#[derive(Component)]
pub struct UiTimelineViewport;

#[derive(Component)]
pub struct UiTimelineScroll;

#[derive(Component)]
pub struct TimelinePanHandle;

#[derive(Component)]
pub struct TimelinePlayhead;

const PLAYHEAD_COLOR: Color = Color::srgba(1.0, 0.85, 0.2, 0.9);

#[derive(Resource, Default)]
pub struct TimelinePanState {
    pub offset_x: f32,
    drag_anchor: Option<f32>,
}

pub fn spawn_timeline_content(
    band: &mut ChildSpawnerCommands<'_>,
    preview_snapshot: &RuntimePreviewSnapshot,
) {
    band.spawn((UiText::new("Piano roll"), ThemedText));
    band.spawn((
        UiText::new(format!(
            "{} note starts · {} control updates · drag tracks to pan",
            preview_snapshot.starts().count(),
            preview_snapshot.control_updates().count()
        )),
        ThemedText,
    ));

    band.spawn((
        UiTimelineViewport,
        Node {
            flex_grow: 1.0,
            width: percent(100),
            min_height: px(56.0),
            position_type: PositionType::Relative,
            overflow: Overflow::clip(),
            border_radius: BorderRadius::all(px(6.0)),
            ..default()
        },
        BackgroundColor(PANEL_INSET_BG),
        Pickable::default(),
    ))
    .observe(on_timeline_pan_drag_start)
    .observe(on_timeline_pan_drag)
    .observe(on_timeline_pan_drag_end)
    .with_children(|viewport| {
        viewport.spawn((
            TimelinePlayhead,
            Node {
                position_type: PositionType::Absolute,
                top: px(0.0),
                bottom: px(0.0),
                width: px(2.0),
                left: px(0.0),
                ..default()
            },
            BackgroundColor(PLAYHEAD_COLOR),
            GlobalZIndex(5),
        ));
        viewport
            .spawn((
                TimelinePanHandle,
                UiTimelineScroll,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0.0),
                    bottom: px(0.0),
                    left: px(0.0),
                    width: px(TIMELINE_CONTENT_WIDTH),
                    height: percent(100),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4.0),
                    padding: UiRect::all(px(6.0)),
                    ..default()
                },
            ))
            .with_children(|scroll| spawn_timeline_tracks(scroll, preview_snapshot));
    });
}

fn spawn_timeline_tracks(
    scroll: &mut ChildSpawnerCommands<'_>,
    preview_snapshot: &RuntimePreviewSnapshot,
) {
    let mut tracks: Vec<(String, Vec<_>)> = Vec::new();
    for event in preview_snapshot.events() {
        let entry = tracks
            .iter_mut()
            .find(|(label, _)| label == &event.output_id)
            .map(|(_, events)| events);
        if let Some(events) = entry {
            events.push(event);
        } else {
            tracks.push((event.output_id.clone(), vec![event]));
        }
    }

    if tracks.is_empty() {
        scroll.spawn((
            UiText::new("Run preview to populate the piano roll."),
            ThemedText,
        ));
        return;
    }

    tracks.sort_by(|(a, _), (b, _)| a.cmp(b));

    for (label, events) in tracks {
        let mut sorted = events;
        sorted.sort_by(|a, b| {
            a.visible_start
                .value()
                .partial_cmp(&b.visible_start.value())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scroll
            .spawn((Node {
                width: percent(100),
                height: px(TRACK_ROW_HEIGHT),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                flex_shrink: 0.0,
                ..default()
            },))
            .with_children(|row| {
                row.spawn((
                    Node {
                        width: px(TRACK_LABEL_WIDTH),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    UiText::new(track_label(&label)),
                    ThemedText,
                ));

                row.spawn((Node {
                    flex_grow: 1.0,
                    height: percent(100),
                    position_type: PositionType::Relative,
                    min_width: px(0.0),
                    ..default()
                },))
                    .with_children(|lane| {
                        for (index, event) in sorted.iter().enumerate() {
                            let left = (event.visible_start.value() as f32) * CYCLE_SCALE;
                            let span = ((event.visible_end.value() - event.visible_start.value())
                                .max(0.05) as f32)
                                * CYCLE_SCALE;
                            let width = span.clamp(EVENT_MIN_WIDTH, EVENT_MAX_WIDTH);
                            let color = match event.kind {
                                TimelineEventKind::StartVoice => ArtistPalette::ATOM_NOTE,
                                TimelineEventKind::ControlUpdate => ArtistPalette::ATOM_SCALAR,
                            };

                            lane.spawn((
                                musaic_clickable(
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: px(left),
                                        top: px(4.0),
                                        width: px(width),
                                        height: px(TRACK_ROW_HEIGHT - 8.0),
                                        border_radius: BorderRadius::all(px(3.0)),
                                        ..default()
                                    },
                                    InspectorButtonAction(EditorCommand::JumpToTimelineSource {
                                        event: event.id,
                                    }),
                                    "",
                                ),
                                BackgroundColor(color),
                            ))
                            .observe(super::on_inspector_button_activated);

                            if index + 1 < sorted.len() {
                                let next_left =
                                    (sorted[index + 1].visible_start.value() as f32) * CYCLE_SCALE;
                                let arrow_x = left + width + 2.0;
                                if next_left > arrow_x {
                                    lane.spawn((
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: px(arrow_x),
                                            top: px(9.0),
                                            width: px((next_left - arrow_x).min(12.0)),
                                            height: px(2.0),
                                            ..default()
                                        },
                                        BackgroundColor(ArtistPalette::FLOW_CONTROL),
                                    ));
                                }
                            }
                        }
                    });
            });
    }
}

fn track_label(output_id: &str) -> String {
    match output_id {
        "main" | "root" => "Home".to_string(),
        other if other.len() <= 10 => other.to_string(),
        other => other.chars().take(8).collect(),
    }
}

pub fn on_timeline_pan_drag_start(
    mut event: On<'_, '_, Pointer<DragStart>>,
    mut pan: ResMut<'_, TimelinePanState>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    pan.drag_anchor = Some(pan.offset_x);
    event.propagate(false);
}

pub fn on_timeline_pan_drag(
    mut event: On<'_, '_, Pointer<Drag>>,
    mut pan: ResMut<'_, TimelinePanState>,
) {
    let Some(anchor) = pan.drag_anchor else {
        return;
    };
    pan.offset_x = (anchor - event.delta.x).clamp(0.0, TIMELINE_CONTENT_WIDTH);
    event.propagate(false);
}

pub fn on_timeline_pan_drag_end(
    mut event: On<'_, '_, Pointer<DragEnd>>,
    mut pan: ResMut<'_, TimelinePanState>,
) {
    pan.drag_anchor = None;
    event.propagate(false);
}

pub fn apply_timeline_pan(scroll: &mut Node, pan: &TimelinePanState) {
    scroll.left = px(-pan.offset_x);
}

pub fn sync_timeline_content(
    mut commands: Commands,
    preview: Res<'_, RuntimePreviewSnapshot>,
    dirty: Res<'_, crate::application::pipeline::ui_projection::UiDirty>,
    content_roots: Query<Entity, With<UiTimelineContent>>,
    mut cache: ResMut<'_, TimelineContentCache>,
) {
    if !dirty.timeline && !cache.force_next {
        return;
    }
    cache.force_next = false;

    for root in content_roots.iter() {
        commands.entity(root).despawn_children();
        commands.entity(root).with_children(|band| {
            spawn_timeline_content(band, &preview);
        });
    }
}

#[derive(Resource, Default)]
pub struct TimelineContentCache {
    /// Set true after full shell respawn so content rehydrates once.
    pub force_next: bool,
}

pub fn sync_timeline_playhead(
    clock: Res<'_, crate::application::editor::transport::TransportClock>,
    pan: Res<'_, TimelinePanState>,
    mut playheads: Query<&mut Node, With<TimelinePlayhead>>,
) {
    let x = (clock.position.value() as f32 * CYCLE_SCALE - pan.offset_x).max(0.0);
    for mut node in playheads.iter_mut() {
        node.left = px(x);
    }
}

pub fn sync_timeline_pan_offset(
    pan: Res<'_, TimelinePanState>,
    mut scroll_nodes: Query<&mut Node, With<UiTimelineScroll>>,
) {
    for mut node in &mut scroll_nodes {
        apply_timeline_pan(&mut node, &pan);
    }
}
