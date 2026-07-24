//! Shell layout: UI root, main row (timeline band, board region, inspector
//! column), board viewport, and edge-drag panel resizing.
//!
//! [`WorkspaceLayoutKind`] selects the document tree:
//! - Compose — board + inspector only (no timeline band entity)
//! - TimelineStacked — timeline band above board + inspector (default open)

use bevy::{
    picking::prelude::*,
    prelude::*,
    ui::widget::{Text as UiText, ViewportNode},
};
use bevy_feathers::theme::ThemedText;

use crate::{
    application::board_view_settings::BoardViewSettings,
    application::command::{EditorCommand, EditorCommandBus},
    application::editor::{
        InspectorLayout, MinimapPanelState, TimelinePanelState, WorkspaceLayoutKind,
    },
    application::pipeline::runtime::RuntimePreviewSnapshot,
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
    preview_snapshot: &RuntimePreviewSnapshot,
    board_camera: Option<Entity>,
    palette_camera: Option<Entity>,
    minimap_width: f32,
    timeline_height: f32,
    host: &mut InspectorPanelHost,
    sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
    view_settings: BoardViewSettings,
) {
    let gap = theme.spacing.panel_gap;
    root.spawn((
        UiShellMain,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(gap),
            ..default()
        },
    ))
    .with_children(|main| {
        match projection.layout_kind {
            WorkspaceLayoutKind::ComposeBoardInspector => {
                // Compose contract: no timeline band in the tree.
            }
            WorkspaceLayoutKind::TimelineStackedOverBoardInspector => {
                let height = if timeline_height > 0.0 {
                    timeline_height
                } else {
                    TimelinePanelState::DEFAULT_HEIGHT
                };
                spawn_timeline_band(main, theme, height, preview_snapshot, images, sprites);
            }
        }

        main.spawn((Node {
            width: percent(100),
            flex_grow: 1.0,
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: px(gap),
            min_height: px(0),
            ..default()
        },))
            .with_children(|row| {
                spawn_board_region(
                    row,
                    theme,
                    visible,
                    &projection.minimap_paint,
                    board_camera,
                    minimap_width,
                    sprites,
                    images,
                );
                spawn_inspector_column(
                    row,
                    theme,
                    layout,
                    &projection.inspector_paint,
                    preview_snapshot,
                    palette_camera,
                    host,
                    sprites,
                    images,
                    view_settings,
                );
            });
    });
}

fn spawn_timeline_band(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    height: f32,
    preview_snapshot: &RuntimePreviewSnapshot,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
) {
    spawn_shell_panel(
        parent,
        (
            UiShellTimeline,
            Node {
                width: percent(100),
                height: px(height),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(theme.spacing.md + 2.0)),
                row_gap: px(theme.spacing.sm),
                overflow: Overflow::clip(),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(theme.chrome.panel_bg),
        ),
        images,
        sprites,
        PanelBackdrop::Section,
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
                timeline_view::spawn_timeline_content(content, theme, preview_snapshot);
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
    // Minimap overlays the board's left side instead of occupying a sibling
    // column, so the board viewport always spans the full region.
    spawn_shell_panel(
        parent,
        (
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                height: percent(100),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme.chrome.panel_bg),
        ),
        images,
        sprites,
        PanelBackdrop::Panel,
        |board_column| {
            spawn_board_viewport(board_column, board_camera);
            minimap::spawn_minimap_panel(
                board_column,
                theme,
                visible,
                minimap_paint,
                minimap_width,
                images,
                sprites,
            );
            minimap::spawn_minimap_edge_handle(board_column, theme, minimap_width);
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
    projection: Res<'_, EditorUiProjection>,
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

    // Timeline height only applies when the shell tree includes the band.
    if matches!(
        projection.layout_kind,
        WorkspaceLayoutKind::TimelineStackedOverBoardInspector
    ) {
        let timeline_height = timeline_panel.visible_height().max(0.0);
        let show = timeline_height > 0.0;
        for mut node in panel_nodes.p1().iter_mut() {
            node.height = px(if show {
                timeline_height
            } else {
                TimelinePanelState::DEFAULT_HEIGHT
            });
            node.display = if show {
                Display::Flex
            } else {
                Display::None
            };
        }
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
    minimap.apply_drag_delta(event.delta.x);
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
    projection: Res<'_, EditorUiProjection>,
    mut bus: MessageWriter<'_, EditorCommandBus>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    // Compose has no timeline band — enter TimelineStacked so the shell can
    // rebuild with the region before height tracking continues.
    if matches!(
        projection.layout_kind,
        WorkspaceLayoutKind::ComposeBoardInspector
    ) {
        bus.write(EditorCommandBus(EditorCommand::EnterTimelineMode));
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
    timeline.apply_drag_delta(-event.delta.y);
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
