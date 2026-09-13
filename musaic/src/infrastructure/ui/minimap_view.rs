//! Minimap: 3×3 color-coded pixels per board cell plus flow-line overlay.

use bevy::{picking::prelude::*, prelude::*};
use bevy_ui_widgets::Activate;

use crate::{
    application::command::EditorCommand,
    application::editor::PickHit,
    application::editor::interaction::{BoardPickEvent, BoardPickHit, BoardPickTargetKind},
    application::pipeline::scene_sync::{
        RenderBoardFocus, VisibleAtomCompound, VisibleBoardNode, VisibleBoardState, VisibleNodeKind,
    },
    application::pipeline::ui_projection::MinimapPaint,
    domain::board::{BoardSlot, VIEWPORT_COLUMNS, VIEWPORT_ROWS},
    domain::document::PlacementAddress,
};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::theme::{MusaicUiTheme, SemanticColors};
use crate::infrastructure::ui::ui_sprites;
use crate::infrastructure::ui::widgets::{MusaicClickable, musaic_button};

use super::InspectorButtonAction;
use super::camera_rig::CameraRequest;

const FLOW_LINE_STEPS: usize = 12;

/// Slot padding around the occupied bounding box.
const WINDOW_PADDING: i32 = 2;
/// Largest window rendered as UI cells (the board itself is unbounded).
const WINDOW_MAX_SPAN: i32 = 24;
/// Window span shown for an empty board, centered on the world origin slots.
const WINDOW_DEFAULT_SPAN: i32 = 12;

/// The slot region the minimap displays. The root board is unbounded, so the
/// minimap frames the occupied bounding box (plus focus) rather than a fixed
/// grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimapWindow {
    pub min: BoardSlot,
    pub cols: i32,
    pub rows: i32,
}

impl MinimapWindow {
    pub fn from_visible(visible: &VisibleBoardState) -> Self {
        let mut slots: Vec<BoardSlot> = Vec::new();
        for node in &visible.nodes {
            if !hidden_atom(node) {
                slots.push(display_slot(visible, node.address));
            }
        }
        for compound in &visible.atom_compounds {
            slots.push(compound_grid_slot(visible, compound.slot));
        }
        if let Some(RenderBoardFocus::BoardSlot(slot)) = visible.focus {
            slots.push(slot);
        }

        if slots.is_empty() {
            // World origin sits between slots (16, 16) under the current
            // slot→world mapping; frame that neighbourhood by default.
            let center = BoardSlot::new(VIEWPORT_COLUMNS / 2, VIEWPORT_ROWS / 2);
            return Self {
                min: BoardSlot::new(
                    center.x - WINDOW_DEFAULT_SPAN / 2,
                    center.y - WINDOW_DEFAULT_SPAN / 2,
                ),
                cols: WINDOW_DEFAULT_SPAN,
                rows: WINDOW_DEFAULT_SPAN,
            };
        }

        let min_x = slots.iter().map(|slot| slot.x).min().unwrap() - WINDOW_PADDING;
        let max_x = slots.iter().map(|slot| slot.x).max().unwrap() + WINDOW_PADDING;
        let min_y = slots.iter().map(|slot| slot.y).min().unwrap() - WINDOW_PADDING;
        let max_y = slots.iter().map(|slot| slot.y).max().unwrap() + WINDOW_PADDING;

        let span = |min: i32, max: i32| (max - min + 1).max(WINDOW_DEFAULT_SPAN);
        let clamp_span = |span: i32| span.min(WINDOW_MAX_SPAN);
        let cols = clamp_span(span(min_x, max_x));
        let rows = clamp_span(span(min_y, max_y));
        // Center the (possibly clamped) window on the content bounding box.
        let center_x = (min_x + max_x) / 2;
        let center_y = (min_y + max_y) / 2;
        Self {
            min: BoardSlot::new(center_x - cols / 2, center_y - rows / 2),
            cols,
            rows,
        }
    }

    pub fn slots(&self) -> impl Iterator<Item = BoardSlot> + '_ {
        let min = self.min;
        let cols = self.cols;
        (0..self.rows)
            .flat_map(move |row| (0..cols).map(move |col| BoardSlot::new(min.x + col, min.y + row)))
    }

    fn center_percent(&self, slot: BoardSlot) -> (f32, f32) {
        let x = (slot.x - self.min.x) as f32 + 0.5;
        let y = (slot.y - self.min.y) as f32 + 0.5;
        (x / self.cols as f32 * 100.0, y / self.rows as f32 * 100.0)
    }
}

fn hidden_atom(node: &VisibleBoardNode) -> bool {
    node.kind == VisibleNodeKind::Atom
        && matches!(
            node.surface_content,
            crate::application::pipeline::scene_sync::TileSurfaceContent::Empty
        )
}

fn display_slot(visible: &VisibleBoardState, address: PlacementAddress) -> BoardSlot {
    match visible.display_address(address) {
        PlacementAddress::BoardSlot(slot) => slot,
        PlacementAddress::StackIndex(index) => {
            crate::domain::board::geometry::stack_display_slot(index.0)
        }
    }
}

#[derive(Component)]
pub struct UiMinimapContent;

#[derive(Component)]
pub struct UiMinimapCanvas;

#[derive(Component, Clone, Copy)]
pub struct MinimapSlotAction {
    pub slot: BoardSlot,
}

pub fn visible_node_color(semantic: &SemanticColors, node: &VisibleBoardNode) -> Color {
    match node.kind {
        VisibleNodeKind::Tile => semantic.transform_generic,
        VisibleNodeKind::Container => semantic.container,
        VisibleNodeKind::Output => semantic.output,
        VisibleNodeKind::Atom => node
            .atom
            .clone()
            .map(|atom| semantic.atom_value_color(atom))
            .unwrap_or(semantic.atom_note),
        VisibleNodeKind::TrickInstance => semantic.trick_unset,
    }
}

pub fn compound_slot_color(semantic: &SemanticColors, compound: &VisibleAtomCompound) -> Color {
    compound
        .primary_atom
        .clone()
        .map(|atom| semantic.atom_value_color(atom))
        .unwrap_or(semantic.atom_note)
}

pub fn flow_color(semantic: &SemanticColors, scalar: bool) -> Color {
    if scalar {
        semantic.flow_scalar
    } else {
        semantic.flow_control
    }
}

pub fn spawn_minimap_content(
    panel: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    images: &Assets<Image>,
    visible: &VisibleBoardState,
    paint: &MinimapPaint,
    sprites: Option<&UiSpriteAssets>,
) {
    panel
        .spawn((
            UiMinimapCanvas,
            Node {
                flex_grow: 1.0,
                width: percent(100),
                min_height: px(96.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(theme.radii.md)),
                ..default()
            },
            BackgroundColor(theme.chrome.panel_inset),
        ))
        .with_children(|canvas| {
            let window = MinimapWindow::from_visible(visible);
            spawn_flow_lines(canvas, theme, visible, &window);
            spawn_pixel_grid(canvas, theme, images, visible, sprites, &window);
        });

    for (label, surface) in &paint.surface_buttons {
        panel
            .spawn((
                musaic_button(
                    Node {
                        min_height: px(28.0),
                        padding: UiRect::horizontal(px(theme.spacing.md)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    InspectorButtonAction(EditorCommand::NavigateToSurface { surface: *surface }),
                    label.clone(),
                ),
                BackgroundColor(theme.chrome.button_bg),
            ))
            .observe(super::on_inspector_button_activated);
    }
}

fn spawn_pixel_grid(
    canvas: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    images: &Assets<Image>,
    visible: &VisibleBoardState,
    sprites: Option<&UiSpriteAssets>,
    window: &MinimapWindow,
) {
    // One flat colored cell per windowed slot — the board itself is
    // unbounded, so the minimap frames the occupied region instead of trying
    // to render every slot.
    canvas
        .spawn((
            Node {
                width: percent(100),
                aspect_ratio: Some(window.cols as f32 / window.rows as f32),
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(window.cols as u16, 1.0),
                grid_template_rows: RepeatedGridTrack::flex(window.rows as u16, 1.0),
                column_gap: px(1.0),
                row_gap: px(1.0),
                padding: UiRect::all(px(3.0)),
                ..default()
            },
            ZIndex(2),
        ))
        .with_children(|grid| {
            for slot in window.slots() {
                let node = visible.nodes.iter().find(|entry| {
                    !hidden_atom(entry) && display_slot(visible, entry.address) == slot
                });
                let compound_entry = visible
                    .atom_compounds
                    .iter()
                    .find(|c| compound_grid_slot(visible, c.slot) == slot);
                let focused = matches!(
                    visible.focus,
                    Some(RenderBoardFocus::BoardSlot(focus_slot)) if focus_slot == slot
                );
                let fill = if focused {
                    theme.semantic.focus_accent
                } else if let Some(node) = node {
                    visible_node_color(&theme.semantic, node)
                } else if let Some(compound) = compound_entry {
                    compound_slot_color(&theme.semantic, compound)
                } else {
                    theme.semantic.empty_cell
                };

                grid.spawn((
                    Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    MusaicClickable,
                    Pickable::default(),
                    MinimapSlotAction { slot },
                    BackgroundColor(fill),
                ))
                .observe(on_minimap_slot_activated)
                .with_children(|cell| {
                    if focused && let Some(sprites) = sprites {
                        ui_sprites::spawn_scaled_sprite(
                            cell,
                            images,
                            &sprites.minimap_selection_marker,
                            Node {
                                position_type: PositionType::Absolute,
                                width: percent(100),
                                height: percent(100),
                                ..default()
                            },
                        );
                    }
                });
            }
        });
}

fn spawn_flow_lines(
    canvas: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    visible: &VisibleBoardState,
    window: &MinimapWindow,
) {
    canvas
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ZIndex(3),
            Pickable::IGNORE,
        ))
        .with_children(|layer| {
            for connection in &visible.connections {
                let scalar = connection.from_slot.x == connection.to_slot.x;
                let color = flow_color(&theme.semantic, scalar);
                let (from_x, from_y) = window.center_percent(connection.from_slot);
                let (to_x, to_y) = window.center_percent(connection.to_slot);
                for step in 0..=FLOW_LINE_STEPS {
                    let t = step as f32 / FLOW_LINE_STEPS as f32;
                    let x = from_x + (to_x - from_x) * t;
                    let y = from_y + (to_y - from_y) * t;
                    layer.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(x),
                            top: percent(y),
                            width: px(3.0),
                            height: px(3.0),
                            margin: UiRect::axes(px(-1.5), px(-1.5)),
                            ..default()
                        },
                        BackgroundColor(color),
                    ));
                }
                spawn_flow_arrow(
                    layer,
                    connection.from_slot,
                    connection.to_slot,
                    color,
                    window,
                );
            }
        });
}

fn spawn_flow_arrow(
    layer: &mut ChildSpawnerCommands<'_>,
    from: BoardSlot,
    to: BoardSlot,
    color: Color,
    window: &MinimapWindow,
) {
    let (from_x, from_y) = window.center_percent(from);
    let (to_x, to_y) = window.center_percent(to);
    let tip_x = from_x + (to_x - from_x) * 0.88;
    let tip_y = from_y + (to_y - from_y) * 0.88;
    layer.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(tip_x),
            top: percent(tip_y),
            width: px(4.0),
            height: px(4.0),
            margin: UiRect::axes(px(-2.0), px(-2.0)),
            ..default()
        },
        BackgroundColor(color),
    ));
}

pub fn on_minimap_slot_activated(
    activate: On<'_, '_, Activate>,
    actions: Query<'_, '_, &MinimapSlotAction>,
    visible: Res<'_, VisibleBoardState>,
    mut pick_events: MessageWriter<'_, BoardPickEvent>,
    mut camera_requests: MessageWriter<'_, CameraRequest>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    let Some(surface) = visible.active_surface else {
        return;
    };
    // VisibleBoardState::pick_at owns empty-slot / stack-insert classification.
    let pick_slot = if visible.layout == crate::domain::board::SurfaceLayoutKind::Stack {
        let Some(index) = crate::domain::board::geometry::stack_index_at_display_slot(action.slot)
        else {
            return;
        };
        BoardSlot::new(index as i32, 0)
    } else {
        action.slot
    };
    let Some(hit) = visible.pick_at(pick_slot) else {
        return;
    };
    let address = match &hit {
        PickHit::EmptySlot { slot, .. } => PlacementAddress::BoardSlot(*slot),
        PickHit::StackInsert { index, .. } => PlacementAddress::StackIndex(*index),
        PickHit::Tile { node } => match visible.address_of(node) {
            Some(address) => address,
            None => return,
        },
        PickHit::AtomCompound { primary_node, .. } => match visible.address_of(primary_node) {
            Some(address) => address,
            None => return,
        },
        PickHit::Port { .. } => return,
    };
    let kind = match hit {
        PickHit::EmptySlot { slot, .. } => BoardPickTargetKind::Slot { slot },
        PickHit::StackInsert { index, .. } => BoardPickTargetKind::StackInsert { index },
        PickHit::Tile { node } => BoardPickTargetKind::BoardTile { tile_id: node },
        PickHit::AtomCompound { primary_node, .. } => {
            BoardPickTargetKind::AtomCompound { primary_node }
        }
        PickHit::Port { .. } => return,
    };
    pick_events.write(BoardPickEvent::Hit(BoardPickHit {
        surface_id: surface,
        kind,
        world_position: Vec3::ZERO,
        distance: 0.0,
    }));
    camera_requests.write(CameraRequest::FocusAddress(address));
}

pub fn sync_minimap_content(
    mut commands: Commands,
    theme: Res<'_, MusaicUiTheme>,
    images: Res<'_, Assets<Image>>,
    visible: Res<'_, VisibleBoardState>,
    projection: Res<'_, crate::application::pipeline::ui_projection::EditorUiProjection>,
    ui_sprites: Option<Res<'_, UiSpriteAssets>>,
    dirty: Res<'_, crate::application::pipeline::ui_projection::UiDirty>,
    content_roots: Query<Entity, With<UiMinimapContent>>,
    mut cache: ResMut<'_, MinimapContentCache>,
) {
    if !dirty.minimap && !cache.force_next {
        return;
    }
    cache.force_next = false;

    for root in content_roots.iter() {
        commands.entity(root).despawn_children();
        commands.entity(root).with_children(|panel| {
            spawn_minimap_content(
                panel,
                &theme,
                &images,
                &visible,
                &projection.minimap_paint,
                ui_sprites.as_deref(),
            );
        });
    }
}

#[derive(Resource, Default)]
pub struct MinimapContentCache {
    /// Set true after full shell respawn so content rehydrates once.
    pub force_next: bool,
}

fn compound_grid_slot(visible: &VisibleBoardState, slot: BoardSlot) -> BoardSlot {
    if visible.layout == crate::domain::board::SurfaceLayoutKind::Stack {
        crate::domain::board::geometry::stack_display_slot(slot.x as usize)
    } else {
        slot
    }
}
