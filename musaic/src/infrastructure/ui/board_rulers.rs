//! Retained coordinate labels around the actual board viewport.
//! Reads the smoothed camera; never changes camera or document state.
use super::{board::Board3dCamera, board_camera_nav::UiBoardViewport, theme::MusaicUiTheme};
use crate::{
    application::pipeline::{
        scene_sync::VisibleBoardState,
        selected_tile::{grid_column, grid_row},
    },
    domain::board::{SurfaceLayoutKind, board_height, board_width},
    infrastructure::app::AppState,
};
use bevy::prelude::*;

pub(super) const LEFT: f32 = 42.0;
pub(super) const TOP: f32 = 24.0;
const LABELS: usize = 64;
#[derive(Component)]
struct RulerRoot;
#[derive(Component)]
struct Tick {
    column: bool,
    index: usize,
}
#[derive(Component)]
struct Caption;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        sync.after(super::camera_rig::CameraRigSet::Smooth)
            .run_if(in_state(AppState::Editor)),
    );
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    for column in [true, false] {
        let node = if column {
            Node {
                position_type: PositionType::Absolute,
                left: px(LEFT),
                right: px(0),
                top: px(0),
                height: px(TOP),
                overflow: Overflow::clip(),
                ..default()
            }
        } else {
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(TOP),
                bottom: px(0),
                width: px(LEFT),
                overflow: Overflow::clip(),
                ..default()
            }
        };
        parent
            .spawn((RulerRoot, node, Pickable::IGNORE))
            .with_children(|strip| {
                if column {
                    strip.spawn((
                        Caption,
                        Text::new(""),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.chrome.text_dim),
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(3),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                }
                for index in 0..LABELS {
                    strip.spawn((
                        Name::new(if column {
                            "Board column coordinate"
                        } else {
                            "Board row coordinate"
                        }),
                        Tick { column, index },
                        Text::new(""),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.chrome.text_dim),
                        TextLayout::new_with_justify(Justify::Center),
                        Node {
                            position_type: PositionType::Absolute,
                            width: px(if column { 64.0 } else { LEFT - 4.0 }),
                            height: px(18),
                            ..default()
                        },
                        Visibility::Hidden,
                        Pickable::IGNORE,
                    ));
                }
            });
    }
}

#[derive(Debug, PartialEq)]
struct Label {
    index: i32,
    pixel: f32,
}
/// Place only fully visible, evenly spaced cell-center labels. Signed indices
/// are aligned to a power-of-two stride, so panning does not reshuffle the ticks.
fn labels(start: f64, span: f64, pixels: f32, column: bool) -> Vec<Label> {
    if !start.is_finite() || !span.is_finite() || span <= 0.0 || pixels <= 0.0 {
        return vec![];
    }
    let spacing = if column { 68.0 } else { 24.0 };
    let required = (span * spacing / pixels as f64)
        .max(span / (LABELS - 1) as f64)
        .max(1.0);
    let stride = 2.0_f64.powf(required.log2().ceil()).min(i32::MAX as f64);
    let first = (start / stride).ceil() * stride;
    let inset = if column { 32.0 } else { 9.0 };
    (0..LABELS)
        .filter_map(|offset| {
            let index = first + offset as f64 * stride;
            if index < i32::MIN as f64 || index > i32::MAX as f64 {
                return None;
            }
            let pixel = ((index - start) / span) as f32 * pixels;
            (pixel >= inset && pixel <= pixels - inset).then_some(Label {
                index: index as i32,
                pixel,
            })
        })
        .collect()
}

fn sync(
    visible: Res<VisibleBoardState>,
    cameras: Query<(&Transform, &Projection), With<Board3dCamera>>,
    viewports: Query<(Entity, &ComputedNode), With<UiBoardViewport>>,
    mut ticks: Query<(&Tick, &mut Text, &mut Node, &mut Visibility)>,
    mut captions: Query<&mut Text, (With<Caption>, Without<Tick>)>,
    mut previous: Local<Option<(Entity, Vec2, Vec3, Quat, f32, SurfaceLayoutKind)>>,
) {
    let Ok((camera, Projection::Orthographic(projection))) = cameras.single() else {
        return;
    };
    let Ok((entity, viewport)) = viewports.single() else {
        return;
    };
    let size = viewport.size() * viewport.inverse_scale_factor();
    if size.min_element() <= 1.0 {
        return;
    }
    let height = match projection.scaling_mode {
        bevy::camera::ScalingMode::FixedVertical { viewport_height } => {
            viewport_height * projection.scale
        }
        _ => return,
    };
    let key = (
        entity,
        size,
        camera.translation,
        camera.rotation,
        height,
        visible.layout,
    );
    if previous.as_ref() == Some(&key) {
        return;
    }
    *previous = Some(key);
    let upright = camera.right().dot(Vec3::X) > 0.99999 && camera.up().dot(Vec3::NEG_Z) > 0.99999;
    let root = visible.layout == SurfaceLayoutKind::Board;
    let width = height * size.x / size.y;
    let columns = if root && upright {
        labels(
            (camera.translation.x - width * 0.5 + board_width() * 0.5 - 0.5) as f64,
            width as f64,
            size.x,
            true,
        )
    } else {
        vec![]
    };
    let rows = if root && upright {
        labels(
            (camera.translation.z - height * 0.5 + board_height() * 0.5 - 0.5) as f64,
            height as f64,
            size.y,
            false,
        )
    } else {
        vec![]
    };
    for mut text in &mut captions {
        let caption = if !root {
            "Pattern order"
        } else if !upright {
            "Rotated view · Fit whole board restores coordinate rulers"
        } else {
            ""
        };
        if text.0 != caption {
            text.0 = caption.into();
        }
    }
    for (tick, mut text, mut node, mut visibility) in &mut ticks {
        let label = if tick.column {
            columns.get(tick.index)
        } else {
            rows.get(tick.index)
        };
        *visibility = if label.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let Some(label) = label else {
            continue;
        };
        let value = if tick.column {
            grid_column(label.index)
        } else {
            grid_row(label.index)
        };
        if text.0 != value {
            text.0 = value;
        }
        node.left = px(if tick.column { label.pixel - 32.0 } else { 0.0 });
        node.top = px(if tick.column { 3.0 } else { label.pixel - 9.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ticks_follow_cell_centers_and_use_stable_strides_when_panning() {
        let before = labels(-3.5, 12.0, 960.0, true);
        assert!(before.iter().any(|l| l.index == 0 && l.pixel == 280.0));
        let after = labels(-2.5, 12.0, 960.0, true);
        for a in &after {
            if let Some(b) = before.iter().find(|b| b.index == a.index) {
                assert!((b.pixel - a.pixel - 80.0).abs() < 0.001);
            }
        }
    }
    #[test]
    fn distant_views_are_bounded_and_do_not_overlap_labels() {
        for span in [9.0, 24.0, 200.0, 10_000.0] {
            for column in [true, false] {
                let ticks = labels(-span * 0.5, span, 900.0, column);
                assert!(ticks.len() <= LABELS);
                assert!(ticks.len() >= 2);
                for pair in ticks.windows(2) {
                    assert!(pair[1].pixel - pair[0].pixel >= if column { 68.0 } else { 24.0 });
                }
            }
        }
        assert!(labels(0.0, 0.0, 100.0, true).is_empty());
    }
}
