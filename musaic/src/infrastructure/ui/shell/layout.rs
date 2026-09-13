//! Shell layout: a left board/preview workspace beside the full-height inspector,
//! with breadcrumbs above the board and edge-drag panel resizing.
//!
//! [`WorkspaceLayoutKind`] selects the document tree:
//! - Compose — board + inspector only (no timeline band entity)
//! - TimelineStacked — preview below the board, within the left workspace

use bevy::{
    picking::prelude::*,
    prelude::*,
    ui::widget::{Text as UiText, ViewportNode},
};
use bevy_feathers::theme::ThemedText;

use crate::{
    application::command::{EditorCommand, EditorCommandBus},
    application::editor::{InspectorLayout, MinimapPanelState, TimelinePanelState},
    application::pipeline::scene_sync::VisibleBoardState,
    application::pipeline::ui_projection::{EditorUiProjection, MinimapPaint},
};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::board_camera_nav::UiBoardViewport;
use crate::infrastructure::ui::inspector::spawn_inspector_column;
use crate::infrastructure::ui::theme::{InspectorPanelHost, MusaicUiTheme};
use crate::infrastructure::ui::widgets::{PanelBackdrop, spawn_shell_panel};
use crate::infrastructure::ui::{minimap, timeline_view};

#[derive(Component)]
pub(crate) struct MusaicUiRoot;

#[derive(Component)]
struct UiShellMain;

#[derive(Component)]
pub(crate) struct UiShellTimeline;

pub(crate) fn teardown_editor_ui(mut commands: Commands, roots: Query<Entity, With<MusaicUiRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn spawn_main_row(
    root: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    visible: &VisibleBoardState,
    projection: &EditorUiProjection,
    layout: &InspectorLayout,
    board_camera: Option<Entity>,
    minimap_width: f32,
    timeline_height: f32,
    host: &mut InspectorPanelHost,
    sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
) {
    root.spawn((
        UiShellMain,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            flex_basis: px(0),
            min_height: px(0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            ..default()
        },
    ))
    .with_children(|main| {
        // The board and its preview share one column. The inspector is a sibling
        // of that whole column, so its bottom stays aligned with the workspace.
        main.spawn(Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            height: percent(100),
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|workspace| {
            super::breadcrumbs::spawn_board_breadcrumbs(
                workspace,
                theme,
                &projection.breadcrumbs,
                projection.active_surface,
            );
            spawn_board_region(
                workspace,
                theme,
                visible,
                &projection.minimap_paint,
                board_camera,
                minimap_width,
                sprites,
                images,
            );
            // This seam remains reachable when the preview is closed.
            minimap::spawn_timeline_edge_handle(workspace, theme);
            // Keep the band and its grip alive throughout opening/closing drags.
            spawn_timeline_band(
                workspace,
                theme,
                timeline_height,
                projection,
                images,
                sprites,
            );
        });
        spawn_inspector_column(
            main,
            theme,
            layout,
            &projection.inspector_paint,
            host,
            sprites,
            images,
            projection.view_settings,
        );
    });
}

fn spawn_timeline_band(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    height: f32,
    projection: &EditorUiProjection,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
) {
    spawn_shell_panel(
        parent,
        (
            UiShellTimeline,
            super::super::keyboard_regions::KeyboardRegion(
                super::super::keyboard_regions::RegionKind::Timing,
            ),
            bevy::input_focus::tab_navigation::TabIndex(-1),
            Node {
                width: percent(100),
                height: px(height),
                flex_shrink: 0.0,
                display: if height > 0.0 {
                    Display::Flex
                } else {
                    Display::None
                },
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(22.0), px(12.0)),
                border: UiRect::top(px(1.0)),
                row_gap: px(8.0),
                overflow: Overflow::clip(),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(theme.chrome.window_bg),
            BorderColor::all(theme.chrome.border),
        ),
        theme,
        images,
        sprites,
        PanelBackdrop::None,
        |band| {
            band.spawn((
                timeline_view::UiTimelineContent,
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.spacing.sm),
                    min_height: px(0.0),
                    ..default()
                },
            ))
            .with_children(|content| {
                timeline_view::spawn_timeline_content(content, theme, &projection.timeline_paint);
            });
        },
    );
}

fn spawn_board_region(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    visible: &VisibleBoardState,
    minimap_paint: &MinimapPaint,
    board_camera: Option<Entity>,
    minimap_width: f32,
    sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
) {
    spawn_shell_panel(
        parent,
        (
            super::super::keyboard_regions::KeyboardRegion(
                super::super::keyboard_regions::RegionKind::Board,
            ),
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                min_height: px(0),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(8.0),
                padding: UiRect {
                    left: px(22.0),
                    right: px(22.0),
                    top: px(6.0),
                    bottom: px(18.0),
                },
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme.chrome.window_bg),
        ),
        theme,
        images,
        sprites,
        PanelBackdrop::None,
        |board_area| {
            board_area
                .spawn((
                    Node {
                        width: percent(100),
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_width: px(0),
                        min_height: px(0),
                        border: UiRect::all(px(1.0)),
                        position_type: PositionType::Relative,
                        padding: UiRect {
                            left: px(super::super::board_rulers::LEFT),
                            top: px(super::super::board_rulers::TOP),
                            ..default()
                        },
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BorderColor::all(theme.chrome.border),
                ))
                .with_children(|frame| {
                    // The marker stays on the image itself, inside the inset border,
                    // so pointer-to-camera coordinates use the real render rectangle.
                    spawn_board_viewport(frame, board_camera);
                    super::super::board_rulers::spawn(frame, theme);
                    minimap::spawn_minimap_panel(
                        frame,
                        theme,
                        visible,
                        minimap_paint,
                        minimap_width,
                        images,
                        sprites,
                    );
                    minimap::spawn_minimap_edge_handle(frame, theme, minimap_width);
                });
            board_area.spawn((
                super::super::board::detail::DetailLegend,
                UiText::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme.chrome.text_dim),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
        },
    );
}

fn spawn_board_viewport(column: &mut ChildSpawnerCommands<'_>, board_camera: Option<Entity>) {
    match board_camera {
        Some(camera) => {
            column.spawn((
                UiBoardViewport,
                Node {
                    flex_grow: 1.0,
                    min_height: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ViewportNode::new(camera),
            ));
        }
        None => {
            column.spawn((
                Node {
                    flex_grow: 1.0,
                    min_height: px(0),
                    width: percent(100),
                    height: percent(100),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                UiText::new("Board camera missing"),
                ThemedText,
            ));
        }
    }
}

pub(crate) fn sync_shell_panel_sizes(
    minimap_panel: Res<'_, MinimapPanelState>,
    timeline_panel: Res<'_, TimelinePanelState>,
    mut panel_nodes: ParamSet<(
        Query<'_, '_, &mut Node, With<minimap::UiShellMinimap>>,
        Query<'_, '_, &mut Node, With<UiShellTimeline>>,
        Query<'_, '_, &mut Node, With<minimap::MinimapEdgeHandle>>,
    )>,
) {
    let minimap_width = minimap_panel.visible_width();
    for mut node in panel_nodes.p0().iter_mut() {
        node.width = px(minimap_width);
        node.display = if minimap_width > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
    }
    // Keep the drag handle on the minimap's right edge (board's left seam
    // when closed).
    for mut node in panel_nodes.p2().iter_mut() {
        node.left = px(minimap_width);
    }

    let height = timeline_panel.visible_height();
    for mut node in panel_nodes.p1().iter_mut() {
        node.height = px(height);
        node.display = if height > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(crate) fn on_minimap_edge_drag_start(
    mut event: On<'_, '_, Pointer<DragStart>>,
    mut minimap: ResMut<'_, MinimapPanelState>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    minimap.begin_drag();
    event.propagate(false);
}

pub(crate) fn on_minimap_edge_drag(
    mut event: On<'_, '_, Pointer<Drag>>,
    mut minimap: ResMut<'_, MinimapPanelState>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    // Minimap overlays the board's left side: drag right widens it, drag left closes it.
    minimap.apply_drag_distance(event.distance.x);
    event.propagate(false);
}

pub(crate) fn on_minimap_edge_drag_end(
    mut event: On<'_, '_, Pointer<DragEnd>>,
    mut minimap: ResMut<'_, MinimapPanelState>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    minimap.finish_drag();
    event.propagate(false);
}

pub(crate) fn on_timeline_edge_drag_start(
    mut event: On<'_, '_, Pointer<DragStart>>,
    mut timeline: ResMut<'_, TimelinePanelState>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    timeline.begin_drag();
    event.propagate(false);
}

pub(crate) fn on_timeline_edge_drag(
    mut event: On<'_, '_, Pointer<Drag>>,
    mut timeline: ResMut<'_, TimelinePanelState>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    timeline.apply_drag_distance(-event.distance.y);
    event.propagate(false);
}

pub(crate) fn on_timeline_edge_drag_end(
    mut event: On<'_, '_, Pointer<DragEnd>>,
    mut timeline: ResMut<'_, TimelinePanelState>,
    mut bus: MessageWriter<'_, EditorCommandBus>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    timeline.finish_drag();
    if timeline.open {
        bus.write(EditorCommandBus(EditorCommand::EnterTimelineMode));
    } else {
        bus.write(EditorCommandBus(EditorCommand::EnterCompose));
    }
    event.propagate(false);
}
