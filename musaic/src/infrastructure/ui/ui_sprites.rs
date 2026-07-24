//! UI sprites from [`UiSpriteAssets`] (one PNG per widget, nearest + integer scale).

use bevy::{
    prelude::*,
    sprite::{BorderRect, SliceScaleMode, TextureSlicer},
    ui::widget::{ImageNode, NodeImageMode},
};

use crate::adapter::load_up::{UiSpriteAssets, pixel_ui_display_size};

use crate::application::editor::PortSlotState;
use crate::application::pipeline::scene_sync::VisibleNodeKind;

use super::tile_shell::{PANEL_BG, PANEL_INSET_BG, PanelBackdrop};

/// Border inset (texels) for 32×32 `panel_bg` / `section_panel_bg` nine-slice sheets.
pub const PANEL_NINE_SLICE_BORDER: f32 = 4.0;

/// 9-slice config for shell panel chrome (corners stay fixed; center stretches).
pub fn panel_nine_slice_slicer() -> TextureSlicer {
    TextureSlicer {
        border: BorderRect::all(PANEL_NINE_SLICE_BORDER),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    }
}

/// Full-bleed panel backdrop that scales with [`NodeImageMode::Sliced`].
pub fn panel_backdrop_image(handle: &Handle<Image>) -> ImageNode {
    ImageNode::new(handle.clone()).with_mode(NodeImageMode::Sliced(panel_nine_slice_slicer()))
}

/// Multiply tint for `section_panel_bg.png`: its center texel is near-white,
/// which would render themed (light) text unreadable — darken it into a panel
/// surface while keeping the nine-slice border art.
const SECTION_BACKDROP_TINT: Color = Color::srgb(0.19, 0.20, 0.25);

/// Build an [`ImageNode`] for fixed-size UI sprites (icons, glyphs).
pub fn image_node(handle: &Handle<Image>) -> ImageNode {
    ImageNode::new(handle.clone()).with_mode(NodeImageMode::Auto)
}

pub fn port_slot_image<'a>(assets: &'a UiSpriteAssets, state: PortSlotState) -> &'a Handle<Image> {
    match state {
        PortSlotState::Input => &assets.scalar_value_connection,
        PortSlotState::Output => &assets.control_map_connection,
        PortSlotState::None => &assets.edge_direction_none,
    }
}

/// Layout size for a port compass button (integer multiple of source texels).
pub fn port_glyph_layout_size(
    images: &Assets<Image>,
    assets: &UiSpriteAssets,
    state: PortSlotState,
) -> Vec2 {
    let handle = port_slot_image(assets, state);
    pixel_ui_display_size(images, handle).unwrap_or_else(|| {
        let fallback = port_glyph_fallback_px(state);
        Vec2::splat(fallback)
    })
}

fn port_glyph_fallback_px(state: PortSlotState) -> f32 {
    match state {
        PortSlotState::Input | PortSlotState::Output => 24.0,
        PortSlotState::None => 32.0,
    }
}

/// Spawn a pixel-art UI image with explicit integer-scaled width/height.
pub fn spawn_scaled_sprite(
    parent: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    handle: &Handle<Image>,
    extra_node: Node,
) {
    let Some(size) = pixel_ui_display_size(images, handle) else {
        return;
    };
    parent.spawn((image_node(handle), extra_node.with_size(size.x, size.y)));
}

pub fn spawn_panel_backdrop(
    parent: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    sprites: Option<&UiSpriteAssets>,
    backdrop: PanelBackdrop,
) {
    let (handle, fallback, tint) = match backdrop {
        PanelBackdrop::Panel => (sprites.map(|a| a.panel_bg.clone()), PANEL_BG, None),
        PanelBackdrop::Section => (
            sprites.map(|a| a.section_panel_bg.clone()),
            PANEL_INSET_BG,
            Some(SECTION_BACKDROP_TINT),
        ),
        PanelBackdrop::None => return,
    };
    let Some(handle) = handle else {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(fallback),
            Pickable::IGNORE,
        ));
        return;
    };
    if pixel_ui_display_size(images, &handle).is_none() {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(fallback),
            Pickable::IGNORE,
        ));
        return;
    };
    let mut image = panel_backdrop_image(&handle);
    if let Some(tint) = tint {
        image.color = tint;
    }
    parent.spawn((
        image,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            right: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            width: percent(100),
            height: percent(100),
            ..default()
        },
        Pickable::IGNORE,
    ));
}

pub fn spawn_menu_item_backdrop(
    parent: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    sprites: &UiSpriteAssets,
) -> bool {
    let Some(size) = pixel_ui_display_size(images, &sprites.menu_item_slot) else {
        return false;
    };
    parent.spawn((
        image_node(&sprites.menu_item_slot),
        Node {
            position_type: PositionType::Absolute,
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        Pickable::IGNORE,
    ));
    true
}

pub fn minimap_tile_image<'a>(
    assets: &'a UiSpriteAssets,
    kind: VisibleNodeKind,
) -> &'a Handle<Image> {
    match kind {
        VisibleNodeKind::Container | VisibleNodeKind::Atom => &assets.minimap_container_tile,
        VisibleNodeKind::Output => &assets.minimap_output_tile,
        VisibleNodeKind::Tile | VisibleNodeKind::TrickInstance => &assets.minimap_transform_tile,
    }
}

pub fn spawn_breadcrumb_strip(
    parent: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    assets: &UiSpriteAssets,
) {
    spawn_scaled_sprite(
        parent,
        images,
        &assets.scroll_bar,
        Node {
            margin: UiRect::top(px(2.0)),
            flex_shrink: 0.0,
            ..default()
        },
    );
}

pub fn spawn_breadcrumb_chip_background(
    parent: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    assets: &UiSpriteAssets,
) -> bool {
    let Some(size) = pixel_ui_display_size(images, &assets.breadcrumb_slot) else {
        return false;
    };
    parent.spawn((
        panel_backdrop_image(&assets.breadcrumb_slot),
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            right: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            min_width: px(size.x),
            height: px(size.y),
            ..default()
        },
        Pickable::IGNORE,
    ));
    true
}

trait PixelNodeSize {
    fn with_size(self, width: f32, height: f32) -> Self;
}

impl PixelNodeSize for Node {
    fn with_size(mut self, width: f32, height: f32) -> Self {
        self.width = px(width);
        self.height = px(height);
        self
    }
}

fn px(v: f32) -> Val {
    Val::Px(v)
}
