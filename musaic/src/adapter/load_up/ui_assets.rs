//! Individual UI sprites under `assets/ui/`.

use bevy::prelude::*;
use load_up::LoadAssetResource;

/// Compose shell, inspector chrome, minimap, and port compass sprites.
#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
pub struct UiSpriteAssets {
    #[dependency(path = "ui/panel_bg.png")]
    pub panel_bg: Handle<Image>,
    #[dependency(path = "ui/section_panel_bg.png")]
    pub section_panel_bg: Handle<Image>,
    #[dependency(path = "ui/menu_item_slot.png")]
    pub menu_item_slot: Handle<Image>,
    #[dependency(path = "ui/keyboard_key.png")]
    pub keyboard_key: Handle<Image>,
    #[dependency(path = "ui/accent_keyboard_key.png")]
    pub accent_keyboard_key: Handle<Image>,
    #[dependency(path = "ui/minimap_container_tile.png")]
    pub minimap_container_tile: Handle<Image>,
    #[dependency(path = "ui/minimap_output_tile.png")]
    pub minimap_output_tile: Handle<Image>,
    #[dependency(path = "ui/minimap_transforml_tile.png")]
    pub minimap_transform_tile: Handle<Image>,
    #[dependency(path = "ui/minimap_selection_marker.png")]
    pub minimap_selection_marker: Handle<Image>,
    #[dependency(path = "ui/breadcrumb_slot.png")]
    pub breadcrumb_slot: Handle<Image>,
    #[dependency(path = "ui/scroll_bar.png")]
    pub scroll_bar: Handle<Image>,
    #[dependency(path = "ui/scroll_anchor.png")]
    pub scroll_anchor: Handle<Image>,
    #[dependency(path = "ui/control_map_connection.png")]
    pub control_map_connection: Handle<Image>,
    #[dependency(path = "ui/scalar_value_connection.png")]
    pub scalar_value_connection: Handle<Image>,
    #[dependency(path = "ui/edge_disconnected_.png")]
    pub edge_disconnected: Handle<Image>,
    #[dependency(path = "ui/edge_scalar_connection.png")]
    pub edge_scalar_connection: Handle<Image>,
    #[dependency(path = "ui/edge_control_map_connection.png")]
    pub edge_control_map_connection: Handle<Image>,
    #[dependency(path = "ui/edge_direction_none.png")]
    pub edge_direction_none: Handle<Image>,
    #[dependency(path = "ui/edge_direction_north.png")]
    pub edge_direction_north: Handle<Image>,
    #[dependency(path = "ui/edge_direction_south.png")]
    pub edge_direction_south: Handle<Image>,
    #[dependency(path = "ui/edge_direction_east.png")]
    pub edge_direction_east: Handle<Image>,
    #[dependency(path = "ui/edge_direction_west.png")]
    pub edge_direction_west: Handle<Image>,
}

impl UiSpriteAssets {
    pub fn all_handles(&self) -> Vec<Handle<Image>> {
        vec![
            self.panel_bg.clone(),
            self.section_panel_bg.clone(),
            self.menu_item_slot.clone(),
            self.keyboard_key.clone(),
            self.accent_keyboard_key.clone(),
            self.minimap_container_tile.clone(),
            self.minimap_output_tile.clone(),
            self.minimap_transform_tile.clone(),
            self.minimap_selection_marker.clone(),
            self.breadcrumb_slot.clone(),
            self.scroll_bar.clone(),
            self.scroll_anchor.clone(),
            self.control_map_connection.clone(),
            self.scalar_value_connection.clone(),
            self.edge_disconnected.clone(),
            self.edge_scalar_connection.clone(),
            self.edge_control_map_connection.clone(),
            self.edge_direction_none.clone(),
            self.edge_direction_north.clone(),
            self.edge_direction_south.clone(),
            self.edge_direction_east.clone(),
            self.edge_direction_west.clone(),
        ]
    }
}
