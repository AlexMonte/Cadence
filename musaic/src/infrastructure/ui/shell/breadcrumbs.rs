//! Bottom bar: surface breadcrumb trail plus the timeline edge grip.
//!
//! Labels come from [`BreadcrumbPaint`] — this module paints only.

use bevy::{picking::prelude::Pickable, prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use crate::{
    application::command::EditorCommand,
    application::editor::TimelinePanelState,
    application::pipeline::ui_projection::BreadcrumbPaint,
    domain::board::BoardSurfaceId,
};

use super::menu::{InspectorButtonAction, on_inspector_button_activated};
use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::minimap;
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::ui_sprites;
use crate::infrastructure::ui::widgets::{
    MusaicClickable, PanelBackdrop, musaic_button, spawn_shell_panel,
};

const BOTTOM_TAB_HEIGHT: f32 = 32.0;

#[derive(Component)]
struct UiShellTabs;

pub(crate) fn spawn_bottom_tabs(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    breadcrumbs: &BreadcrumbPaint,
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
            BackgroundColor(theme.chrome.panel_bg),
        ),
        images,
        sprites,
        PanelBackdrop::Panel,
        |tabs| {
            minimap::spawn_timeline_edge_handle(tabs, theme);
            tabs.spawn((Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.xs),
                padding: UiRect::horizontal(px(theme.spacing.md)),
                ..default()
            },))
                .with_children(|row| {
                    for (index, (label, surface)) in breadcrumbs.entries.iter().enumerate() {
                        if index > 0 {
                            row.spawn((UiText::new("|"), ThemedText));
                        }
                        let selected = active_surface == Some(*surface);
                        spawn_breadcrumb_tab(
                            row,
                            theme,
                            images,
                            sprites,
                            label,
                            selected,
                            *surface,
                        );
                    }
                });
        },
    );
}

fn spawn_breadcrumb_tab(
    tabs: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
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
        padding: UiRect::horizontal(px(theme.spacing.md)),
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
                    TextColor(theme.chrome.crumb_active_text),
                ))
                .observe(on_inspector_button_activated);
        });
        return;
    }

    tabs.spawn((
        musaic_button(
            chip,
            InspectorButtonAction(EditorCommand::NavigateToSurface { surface }),
            label.to_string(),
        ),
        if selected {
            BackgroundColor(theme.chrome.crumb_selected_bg)
        } else {
            BackgroundColor(Color::NONE)
        },
    ))
    .observe(on_inspector_button_activated);
}
