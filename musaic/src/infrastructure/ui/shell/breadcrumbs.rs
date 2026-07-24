//! Bottom bar: surface breadcrumb trail plus the timeline edge grip.

use bevy::{picking::prelude::Pickable, prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;
use tessera::prelude::NodeId;

use crate::{
    application::command::EditorCommand,
    application::editor::TimelinePanelState,
    domain::board::{BoardSurfaceId, BoardSurfaceKind},
    domain::document::{DocumentQueries, PlacementAddress},
};

use super::menu::{InspectorButtonAction, on_inspector_button_activated};
use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::controls::{MusaicClickable, musaic_clickable};
use crate::infrastructure::ui::minimap;
use crate::infrastructure::ui::tile_shell::{PANEL_BG, PanelBackdrop, spawn_shell_panel};
use crate::infrastructure::ui::ui_sprites;

const BOTTOM_TAB_HEIGHT: f32 = 32.0;

#[derive(Component)]
struct UiShellTabs;

#[derive(Debug, Clone)]
pub(crate) struct BreadcrumbEntry {
    label: String,
    surface: BoardSurfaceId,
}

pub(crate) fn spawn_bottom_tabs(
    parent: &mut ChildSpawnerCommands<'_>,
    breadcrumbs: &[BreadcrumbEntry],
    active_surface: Option<BoardSurfaceId>,
    sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
) {
    let grip_height = TimelinePanelState::EDGE_HIT_HEIGHT;
    let bar_height = BOTTOM_TAB_HEIGHT + grip_height;

    spawn_shell_panel(
        parent,
        (
            UiShellTabs,
            Node {
                width: percent(100),
                height: px(bar_height),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(0.0),
                padding: UiRect::ZERO,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ),
        images,
        sprites,
        PanelBackdrop::Panel,
        |tabs| {
            minimap::spawn_timeline_edge_handle(tabs);
            tabs.spawn((Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(4),
                padding: UiRect::horizontal(px(8)),
                ..default()
            },))
                .with_children(|row| {
                    for (index, crumb) in breadcrumbs.iter().enumerate() {
                        if index > 0 {
                            row.spawn((UiText::new("|"), ThemedText));
                        }
                        let selected = active_surface == Some(crumb.surface);
                        spawn_breadcrumb_tab(
                            row,
                            images,
                            sprites,
                            &crumb.label,
                            selected,
                            crumb.surface,
                        );
                    }
                });
        },
    );
}

/// Readable label on the near-white `breadcrumb_slot.png` chip.
const ACTIVE_CRUMB_TEXT: Color = Color::srgb(0.13, 0.14, 0.18);

fn spawn_breadcrumb_tab(
    tabs: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
    label: &str,
    selected: bool,
    surface: BoardSurfaceId,
) {
    let chip = Node {
        min_width: px(56.0),
        height: px(26.0),
        flex_shrink: 0.0,
        display: Display::Flex,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        padding: UiRect::horizontal(px(8.0)),
        position_type: PositionType::Relative,
        ..default()
    };

    // `breadcrumb_slot.png` marks the *active* tab only; its light fill needs
    // dark label text (ThemedText would render white-on-white).
    if selected && let Some(sprites) = sprites {
        tabs.spawn(chip).with_children(|chip_root| {
            ui_sprites::spawn_breadcrumb_chip_background(chip_root, images, sprites);
            chip_root
                .spawn((
                    Node {
                        width: percent(100),
                        height: percent(100),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    MusaicClickable,
                    Pickable::default(),
                    InspectorButtonAction(EditorCommand::NavigateToSurface { surface }),
                    UiText::new(label.to_string()),
                    TextColor(ACTIVE_CRUMB_TEXT),
                ))
                .observe(on_inspector_button_activated);
        });
        return;
    }

    tabs.spawn((
        musaic_clickable(
            chip,
            InspectorButtonAction(EditorCommand::NavigateToSurface { surface }),
            label.to_string(),
        ),
        if selected {
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14))
        } else {
            BackgroundColor(Color::NONE)
        },
    ))
    .observe(on_inspector_button_activated);
}

pub(crate) fn breadcrumb_entries(
    queries: &DocumentQueries<'_>,
    active_surface: Option<BoardSurfaceId>,
) -> Vec<BreadcrumbEntry> {
    let Some(mut surface) = active_surface else {
        return Vec::new();
    };

    let mut labels = Vec::new();

    loop {
        match queries.surface_kind(surface) {
            Some(BoardSurfaceKind::RootBoard) => {
                labels.push(BreadcrumbEntry {
                    label: "Home".to_string(),
                    surface,
                });
                break;
            }
            Some(BoardSurfaceKind::ContainerStack { container }) => {
                let container_node = NodeId::new(container.0.clone());
                if let Some(location) = queries.location_of(&container_node) {
                    let label = match location.address {
                        PlacementAddress::BoardSlot(slot) => {
                            format!("Container {}:{}", slot.x, slot.y)
                        }
                        PlacementAddress::StackIndex(index) => {
                            format!("Container @{}", index.0)
                        }
                    };
                    labels.push(BreadcrumbEntry { label, surface });
                    surface = location.surface;
                } else {
                    labels.push(BreadcrumbEntry {
                        label: "Container".to_string(),
                        surface,
                    });
                    break;
                }
            }
            None => {
                labels.push(BreadcrumbEntry {
                    label: "Unknown".to_string(),
                    surface,
                });
                break;
            }
        }
    }

    labels.reverse();
    labels
}
