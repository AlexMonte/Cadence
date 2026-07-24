//! In-editor top chrome (Play / Tiles) and the shell command-button channel.
//!
//! This is **not** the boot MainMenu screen — see
//! [`crate::infrastructure::ui::screens::main_menu`].
//!
//! Any UI button carrying [`MenuBarButtonAction`] or [`InspectorButtonAction`]
//! dispatches its [`EditorCommand`] on the bus when activated.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;
use bevy_ui_widgets::Activate;

use crate::application::command::{EditorCommand, EditorCommandBus};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::ui_sprites;
use crate::infrastructure::ui::widgets::{PanelBackdrop, musaic_button, spawn_shell_panel};

const TOP_MENU_HEIGHT: f32 = 36.0;

#[derive(Component)]
struct UiShellMenu;

#[derive(Component, Clone)]
pub(crate) struct MenuBarButtonAction(EditorCommand);

#[derive(Component)]
pub(crate) struct MenuTransportLabel;

/// Command chip used across inspector, breadcrumbs, minimap, and timeline UI.
#[derive(Component, Clone)]
pub(crate) struct InspectorButtonAction(pub(crate) EditorCommand);

pub(crate) fn spawn_top_menu(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
) {
    spawn_shell_panel(
        parent,
        (
            UiShellMenu,
            Node {
                width: percent(100),
                height: px(TOP_MENU_HEIGHT),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.md),
                padding: UiRect::horizontal(px(theme.spacing.md)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(theme.chrome.panel_bg),
        ),
        images,
        sprites,
        PanelBackdrop::Panel,
        |menu| {
            spawn_menu_chip(
                menu,
                images,
                sprites,
                "Play",
                Some(EditorCommand::TransportToggle),
            );
            spawn_menu_chip(
                menu,
                images,
                sprites,
                "Tiles",
                Some(EditorCommand::ToggleDrawer),
            );
        },
    );
}

fn spawn_menu_chip(
    menu: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
    label: &str,
    action: Option<EditorCommand>,
) {
    let chip_row = Node {
        min_width: px(56.0),
        height: px(28.0),
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        column_gap: px(4.0),
        padding: UiRect::horizontal(px(4.0)),
        position_type: PositionType::Relative,
        ..default()
    };
    if let Some(action) = action {
        menu.spawn(chip_row).with_children(|chip| {
            if let Some(sprites) = sprites {
                ui_sprites::spawn_menu_item_backdrop(chip, images, sprites);
            }
            if matches!(action, EditorCommand::TransportToggle) {
                chip.spawn((musaic_button(
                    Node {
                        width: percent(100),
                        height: percent(100),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    (MenuBarButtonAction(action), MenuTransportLabel),
                    label,
                ),))
                    .observe(on_menu_bar_button_activated);
            } else {
                chip.spawn((musaic_button(
                    Node {
                        width: percent(100),
                        height: percent(100),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    MenuBarButtonAction(action),
                    label,
                ),))
                    .observe(on_menu_bar_button_activated);
            }
        });
    } else {
        menu.spawn(chip_row).with_children(|chip| {
            if let Some(sprites) = sprites {
                ui_sprites::spawn_menu_item_backdrop(chip, images, sprites);
            }
            chip.spawn((UiText::new(label), ThemedText));
        });
    }
}

pub(crate) fn sync_menu_transport_label(
    transport: Res<State<crate::infrastructure::app::TransportMode>>,
    mut labels: Query<&mut Text, With<MenuTransportLabel>>,
) {
    let label = match transport.get() {
        crate::infrastructure::app::TransportMode::Playing => "Stop",
        crate::infrastructure::app::TransportMode::Stopped => "Play",
    };
    for mut text in labels.iter_mut() {
        *text = Text::new(label);
    }
}

fn on_menu_bar_button_activated(
    activate: On<'_, '_, Activate>,
    query: Query<'_, '_, &MenuBarButtonAction>,
    mut command_bus: MessageWriter<'_, EditorCommandBus>,
) {
    let Ok(MenuBarButtonAction(command)) = query.get(activate.entity) else {
        return;
    };
    command_bus.write(EditorCommandBus(command.clone()));
}

pub(crate) fn on_inspector_button_activated(
    activate: On<'_, '_, Activate>,
    query: Query<'_, '_, &InspectorButtonAction>,
    mut writer: MessageWriter<'_, EditorCommandBus>,
) {
    let Ok(action) = query.get(activate.entity) else {
        return;
    };

    writer.write(EditorCommandBus(action.0.clone()));
}
