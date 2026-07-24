//! Tile inspect panel: description, port compass, connect action.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use tessera::prelude::NodeId;

use crate::application::command::EditorCommand;
use crate::application::editor::ConnectionEndpointView;

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::port_glyphs::spawn_inspector_port_compass;
use crate::infrastructure::ui::widgets::musaic_button;
use crate::infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated};

pub(crate) fn spawn_tile_inspect_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    description: &str,
    ports: Option<&ConnectionEndpointView>,
    images: &Assets<Image>,
    ui_sprites: Option<&UiSpriteAssets>,
) {
    panel.spawn((UiText::new(description.to_string()), ThemedText));

    if let Some(view) = ports {
        spawn_inspector_port_compass(panel, images, ui_sprites, node, view);
    }

    panel
        .spawn(musaic_button(
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            InspectorButtonAction(EditorCommand::StartConnection {
                source: node.clone(),
            }),
            "Connect",
        ))
        .observe(on_inspector_button_activated);
}
