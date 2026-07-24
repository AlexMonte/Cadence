//! Shell layout: UI root, main row (timeline band, board region, inspector
//! column), board viewport, and edge-drag panel resizing.

use bevy::{
    picking::prelude::*,
    prelude::*,
    ui::widget::{Text as UiText, ViewportNode},
};
use bevy_feathers::theme::ThemedText;

use crate::{
    application::board_view_settings::BoardViewSettings,
    application::editor::{
        EditorAttention, EditorSession, InspectorLayout, MinimapPanelState, TimelinePanelState,
    },
    application::pipeline::runtime::RuntimePreviewSnapshot,
    application::pipeline::scene_sync::VisibleBoardState,
    domain::document::DocumentQueries,
};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::board_camera_nav::UiBoardViewport;
use crate::infrastructure::ui::inspector::{InspectorPanelHost, spawn_inspector_column};
use crate::infrastructure::ui::tile_shell::{PANEL_BG, PanelBackdrop, spawn_shell_panel};
use crate::infrastructure::ui::{minimap, timeline_view};

pub(crate) const PANEL_GAP: f32 = 12.0;
pub(crate) const SHELL_PADDING: f32 = 10.0;

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
    queries: &DocumentQueries<'_>,
    visible: &VisibleBoardState,
    layout: &InspectorLayout,
    attention: &EditorAttention,
    session: &EditorSession,
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
    root.spawn((
        UiShellMain,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(PANEL_GAP),
            ..default()
        },
    ))
    .with_children(|main| {
        spawn_timeline_band(
            main,
            timeline_height.max(0.0),
            preview_snapshot,
            images,
            sprites,
        );

        main.spawn((Node {
            width: percent(100),
            flex_grow: 1.0,
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: px(PANEL_GAP),
            min_height: px(0),
            ..default()
        },))
            .with_children(|row| {
                spawn_board_region(
                    row,
                    queries,
                    visible,
                    attention,
                    board_camera,
                    minimap_width,
                    sprites,
                    images,
                );
                spawn_inspector_column(
                    row,
                    queries,
                    layout,
                    session,
                    preview_snapshot,
                    visible,
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
    height: f32,
    preview_snapshot: &RuntimePreviewSnapshot,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
) {
    let backdrop = if height > 0.0 {
        PanelBackdrop::Section
    } else {
        PanelBackdrop::None
    };
    spawn_shell_panel(
        parent,
        (
            UiShellTimeline,
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
                padding: UiRect::all(px(10.0)),
                row_gap: px(6.0),
                overflow: Overflow::clip(),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ),
        images,
        sprites,
        backdrop,
        |band| {
            band.spawn((
                timeline_view::UiTimelineContent,
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6.0),
                    min_height: px(0.0),
                    ..default()
                },
            ))
            .with_children(|content| {
                timeline_view::spawn_timeline_content(content, preview_snapshot);
            });
        },
    );
}

fn spawn_board_region(
    parent: &mut ChildSpawnerCommands<'_>,
    queries: &DocumentQueries<'_>,
    visible: &VisibleBoardState,
    attention: &EditorAttention,
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
            BackgroundColor(PANEL_BG),
        ),
        images,
        sprites,
        PanelBackdrop::Panel,
        |board_column| {
            spawn_board_viewport(board_column, board_camera);
            minimap::spawn_minimap_panel(
                board_column,
                visible,
                queries,
                attention,
                minimap_width,
                images,
                sprites,
            );
            minimap::spawn_minimap_edge_handle(board_column, minimap_width);
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

    let timeline_height = timeline_panel.visible_height();
    for mut node in panel_nodes.p1().iter_mut() {
        node.height = px(timeline_height);
        node.display = if timeline_height > 0.0 {
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
    timeline.apply_drag_delta(-event.delta.y);
    event.propagate(false);
}

pub(crate) fn on_timeline_edge_drag_end(
    mut event: On<'_, '_, Pointer<DragEnd>>,
    mut timeline: ResMut<'_, TimelinePanelState>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    timeline.finish_drag();
    event.propagate(false);
}
