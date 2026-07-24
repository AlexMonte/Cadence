//! Tile inspect panel: description, port compass, connect action.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::{
    controls::{ButtonProps, button},
    theme::ThemedText,
};
use bevy_ui_widgets::observe;
use tessera::prelude::NodeId;

use crate::application::command::EditorCommand;
use crate::application::editor::{connection_endpoint_view, tile_inspect_description};
use crate::application::pipeline::scene_sync::VisibleBoardState;
use crate::domain::document::DocumentQueries;

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::port_glyphs::spawn_inspector_port_compass;
use crate::infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated};

pub(crate) fn spawn_tile_inspect_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    queries: &DocumentQueries<'_>,
    node: &NodeId,
    visible: &VisibleBoardState,
    images: &Assets<Image>,
    ui_sprites: Option<&UiSpriteAssets>,
) {
    panel.spawn((
        UiText::new(tile_inspect_description(queries, node)),
        ThemedText,
    ));

    let slot = queries
        .location_of(node)
        .and_then(|location| match location.address {
            crate::domain::document::PlacementAddress::BoardSlot(slot) => Some(slot),
            _ => None,
        })
        .or_else(|| {
            if let Some(crate::application::pipeline::scene_sync::RenderBoardFocus::Tile {
                address: crate::domain::document::PlacementAddress::BoardSlot(slot),
                ..
            }) = visible.focus.as_ref()
            {
                Some(*slot)
            } else {
                None
            }
        });

    if let Some(slot) = slot {
        if let Some(view) = connection_endpoint_view(queries, node, Some(slot), None) {
            spawn_inspector_port_compass(panel, images, ui_sprites, node, &view);
        }
    }

    panel.spawn((
        button(
            ButtonProps::default(),
            InspectorButtonAction(EditorCommand::StartConnection {
                source: node.clone(),
            }),
            Spawn((UiText::new("Connect"), ThemedText)),
        ),
        observe(on_inspector_button_activated),
    ));
}
