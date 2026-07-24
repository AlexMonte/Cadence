//! Minimap shell, edge-drag resize, and timeline edge handle.

use bevy::{picking::prelude::*, prelude::*};

use crate::application::pipeline::scene_sync::VisibleBoardState;
use crate::application::pipeline::ui_projection::MinimapPaint;

use super::minimap_view::{UiMinimapContent, spawn_minimap_content};
use crate::adapter::load_up::UiSpriteAssets;

use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::widgets::{PanelBackdrop, spawn_shell_panel};

pub use super::minimap_view::{
    MinimapContentCache, MinimapSlotAction, UiMinimapCanvas, sync_minimap_content,
};

#[derive(Component)]
pub struct UiShellMinimap;

#[derive(Component)]
pub struct MinimapEdgeHandle;

#[derive(Component)]
pub struct TimelineEdgeHandle;

/// Minimap overlays the board's left side (per `UI_LAYOUT.md`): drag the
/// board's left edge to the right (or press `M`) to reveal it.
pub fn spawn_minimap_panel(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    visible: &VisibleBoardState,
    paint: &MinimapPaint,
    width: f32,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
) {
    let show = width > 0.0;
    spawn_shell_panel(
        parent,
        (
            UiShellMinimap,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                bottom: px(0.0),
                width: px(width),
                display: if show { Display::Flex } else { Display::None },
                flex_direction: FlexDirection::Column,
                row_gap: px(6.0),
                padding: UiRect::all(px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            ZIndex(3),
            BackgroundColor(theme.chrome.panel_bg),
        ),
        images,
        sprites,
        PanelBackdrop::Panel,
        |panel| {
            panel.spawn((
                bevy::ui::widget::Text::new("Minimap"),
                bevy_feathers::theme::ThemedText,
            ));
            panel
                .spawn((
                    UiMinimapContent,
                    Node {
                        flex_grow: 1.0,
                        width: percent(100),
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(6.0),
                        min_height: px(0.0),
                        ..default()
                    },
                ))
                .with_children(|content| {
                    spawn_minimap_content(content, theme, images, visible, paint, sprites);
                });
        },
    );
}

/// Drag handle on the board's left seam (sits at the minimap's right edge
/// while open); pull right to reveal/widen the minimap, left to close it.
pub fn spawn_minimap_edge_handle(
    board_column: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    minimap_width: f32,
) {
    board_column
        .spawn((
            MinimapEdgeHandle,
            Node {
                position_type: PositionType::Absolute,
                left: px(minimap_width),
                top: px(0.0),
                width: px(crate::application::editor::MinimapPanelState::EDGE_HIT_WIDTH),
                height: percent(100),
                ..default()
            },
            ZIndex(4),
            BackgroundColor(theme.chrome.edge_handle_active),
            Pickable::default(),
        ))
        .observe(super::on_minimap_edge_drag_start)
        .observe(super::on_minimap_edge_drag)
        .observe(super::on_minimap_edge_drag_end);
}

/// Full-width grip on the breadcrumb bar — drag upward to reveal the piano-roll band.
pub fn spawn_timeline_edge_handle(tabs_bar: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    tabs_bar
        .spawn((
            TimelineEdgeHandle,
            Node {
                width: percent(100),
                height: px(crate::application::editor::TimelinePanelState::EDGE_HIT_HEIGHT),
                flex_shrink: 0.0,
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.chrome.edge_handle_idle),
            Pickable::default(),
        ))
        .observe(super::on_timeline_edge_drag_start)
        .observe(super::on_timeline_edge_drag)
        .observe(super::on_timeline_edge_drag_end)
        .with_children(|grip| {
            grip.spawn((
                Node {
                    width: px(36.0),
                    height: px(3.0),
                    border_radius: BorderRadius::all(px(theme.radii.sm / 2.0)),
                    ..default()
                },
                BackgroundColor(theme.chrome.edge_handle),
            ));
        });
}
