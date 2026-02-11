//! Cursor sprite lifecycle, pointer context tracking, and visual state resolution.

use bevy::camera::visibility::RenderLayers;
use bevy::ecs::query::QueryFilter;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::NodeId;
use crate::editor::camera::{CURSOR_RENDER_LAYER, CursorCamera, UiCamera};
use crate::editor::canvas::CanvasNode;
use crate::editor::gestures::PointerState;
use crate::editor::menus::ProjectHubNode;
use crate::editor::screens::editor::InEditor;
use crate::editor::ui::interactions::InteractiveChild;
use crate::editor::ux::NodeCloseButton;
use crate::shared::resources::LoadResource;

const CURSOR_Z: f32 = 101.0;
const CURSOR_TILE_SIZE: UVec2 = UVec2::new(16, 16);
const CURSOR_TILE_COLUMNS: u32 = 20;
const CURSOR_TILE_ROWS: u32 = 11;
const CURSOR_TILE_COUNT: usize = (CURSOR_TILE_COLUMNS * CURSOR_TILE_ROWS) as usize;

const CURSOR_INDEX_DEFAULT: usize = 168;
const CURSOR_INDEX_HOVER_NODE: usize = 175;
const CURSOR_INDEX_HOVER_ACTION: usize = 175;
const CURSOR_INDEX_MARQUEE: usize = 125;
const CURSOR_INDEX_DRAGGING_NODE: usize = 178;
const CURSOR_INDEX_PANNING: usize = 3;
#[cfg(test)]
const CURSOR_NONVISIBLE_RESERVED_RANGE: core::ops::RangeInclusive<usize> = 169..=174;

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<CursorAssets>();
    app.init_resource::<CursorVisualMap>();
    app.add_systems(
        OnEnter(InEditor),
        (spawn_cursor_camera, spawn_cursor).chain(),
    )
    .add_systems(
        Update,
        (
            resolve_cursor_visual_state,
            apply_cursor_frame_and_hotspot,
            update_cursor_sprite_position,
        )
            .chain()
            .run_if(in_state(InEditor)),
    );
}

fn spawn_cursor_camera(mut commands: Commands, existing_camera: Query<Entity, With<CursorCamera>>) {
    if !existing_camera.is_empty() {
        return;
    }

    commands.spawn((
        Name::new("Cursor Camera"),
        CursorCamera,
        DespawnOnExit(InEditor),
    ));
}

#[derive(Resource, Asset, Reflect, Clone)]
#[reflect(Resource)]
pub struct CursorAssets {
    #[dependency]
    pub cursor_tileset: Handle<Image>,
}

impl FromWorld for CursorAssets {
    fn from_world(world: &mut World) -> CursorAssets {
        let assets = world.resource::<AssetServer>();
        let cursor_tileset = assets.load("images/cursor.png");
        Self { cursor_tileset }
    }
}
fn spawn_cursor(
    mut commands: Commands,
    cursor_assets: Res<CursorAssets>,
    visual_map: Res<CursorVisualMap>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    existing_cursor: Query<Entity, With<Cursor>>,
) {
    if !existing_cursor.is_empty() {
        return;
    }

    let texture = cursor_assets.cursor_tileset.clone();
    let texture_atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        CURSOR_TILE_SIZE,
        CURSOR_TILE_COLUMNS,
        CURSOR_TILE_ROWS,
        None,
        None,
    ));

    let default_frame = visual_map.frame(CursorVisualState::Default);
    commands.spawn((
        Cursor,
        Name::new("Cursor"),
        CursorVisualStateCache(CursorVisualState::Default),
        CursorHotspot(default_frame.hotspot_world),
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: texture_atlas_layout,
                index: default_frame.atlas_index,
            },
        ),
        Transform::from_translation(Vec3::new(0.0, 0.0, CURSOR_Z)),
        Visibility::Visible,
        Pickable::IGNORE,
        RenderLayers::layer(CURSOR_RENDER_LAYER as usize),
        DespawnOnExit(InEditor),
    ));
}

#[derive(Component, Clone, PartialEq, Debug, Reflect)]
#[reflect(Component)]
#[require(
    PointerPosition::default(),
    PointerTarget::default(),
    Sprite::default()
)]
/// Tag component to identify the main cursor entity.
pub struct Cursor;

#[derive(Component, Clone, PartialEq, Debug, Reflect, Default)]
#[reflect(Component)]
/// Model for pointer appearance (e.g., custom cursor image)
pub struct PointerModel {
    pub image: Option<Handle<Image>>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
struct CursorVisualStateCache(CursorVisualState);

#[derive(Component, Clone, Copy, PartialEq, Debug)]
struct CursorHotspot(Vec2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CursorVisualState {
    Default,
    HoverNode,
    HoverAction,
    Marquee,
    DraggingNode,
    Panning,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct CursorFrame {
    atlas_index: usize,
    hotspot_world: Vec2,
}

#[derive(Resource, Clone)]
struct CursorVisualMap {
    default: CursorFrame,
    hover_node: CursorFrame,
    hover_action: CursorFrame,
    marquee: CursorFrame,
    dragging_node: CursorFrame,
    panning: CursorFrame,
}

impl Default for CursorVisualMap {
    fn default() -> Self {
        Self {
            default: CursorFrame {
                atlas_index: CURSOR_INDEX_DEFAULT,
                hotspot_world: Vec2::new(4.0, -2.0),
            },
            hover_node: CursorFrame {
                atlas_index: CURSOR_INDEX_HOVER_NODE,
                hotspot_world: Vec2::new(4.0, -2.0),
            },
            hover_action: CursorFrame {
                atlas_index: CURSOR_INDEX_HOVER_ACTION,
                hotspot_world: Vec2::new(4.0, -2.0),
            },
            marquee: CursorFrame {
                atlas_index: CURSOR_INDEX_MARQUEE,
                hotspot_world: Vec2::ZERO,
            },
            dragging_node: CursorFrame {
                atlas_index: CURSOR_INDEX_DRAGGING_NODE,
                hotspot_world: Vec2::ZERO,
            },
            panning: CursorFrame {
                atlas_index: CURSOR_INDEX_PANNING,
                hotspot_world: Vec2::ZERO,
            },
        }
    }
}

impl CursorVisualMap {
    fn frame(&self, state: CursorVisualState) -> CursorFrame {
        match state {
            CursorVisualState::Default => self.default,
            CursorVisualState::HoverNode => self.hover_node,
            CursorVisualState::HoverAction => self.hover_action,
            CursorVisualState::Marquee => self.marquee,
            CursorVisualState::DraggingNode => self.dragging_node,
            CursorVisualState::Panning => self.panning,
        }
    }
}

#[derive(Component, Clone, PartialEq, Debug, Reflect)]
#[reflect(Component)]
pub struct PointerPosition {
    viewport_pos: Vec2,
    world_pos: Vec2,
    down_viewport_pos: Option<Vec2>,
    down_world_pos: Option<Vec2>,
    /// Distance cursor has moved since pointer down (used for drag vs click disambiguation)
    pub movement_distance: f32,
}

impl Default for PointerPosition {
    fn default() -> Self {
        Self {
            viewport_pos: Vec2::ZERO,
            world_pos: Vec2::ZERO,
            down_viewport_pos: None,
            down_world_pos: None,
            movement_distance: 0.0,
        }
    }
}

impl PointerPosition {
    pub fn new(viewport_pos: Vec2, world_pos: Vec2) -> Self {
        Self {
            viewport_pos,
            world_pos,
            down_viewport_pos: None,
            down_world_pos: None,
            movement_distance: 0.0,
        }
    }
    /// Begin a new press ("pending" gesture). Also resets movement accumulation.
    pub fn begin_press(&mut self, screen_pos: Vec2, world_pos: Vec2) {
        self.down_viewport_pos = Some(screen_pos);
        self.down_world_pos = Some(world_pos);
        self.viewport_pos = screen_pos;
        self.world_pos = world_pos;
        self.movement_distance = 0.0;
    }

    /// End the current press (clears pending context).
    pub fn end_press(&mut self) {
        self.down_viewport_pos = None;
        self.down_world_pos = None;
        self.movement_distance = 0.0;
    }

    /// Update cursor position and recalculate movement distance.
    pub fn update_cursor(&mut self, screen_pos: Vec2, world_pos: Vec2) {
        if let Some(down_pos) = self.down_viewport_pos {
            self.movement_distance = down_pos.distance(screen_pos);
            self.viewport_pos = screen_pos;
            self.world_pos = world_pos;
        }
    }

    /// Check if cursor has moved beyond drag threshold.
    /// Prefer this for promotion checks.
    pub fn has_exceeded_drag_threshold(&self, threshold_pixels: f32) -> bool {
        self.movement_distance >= threshold_pixels
    }

    pub fn world_pos(&self) -> Vec2 {
        self.world_pos
    }

    pub fn down_world_pos(&self) -> Option<Vec2> {
        self.down_world_pos
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Reflect, Default)]
pub struct Modifiers {
    pub alt: bool,
    pub shift: bool,
    pub ctrl: bool,
}

impl Modifiers {
    pub fn new(alt: bool, shift: bool, ctrl: bool) -> Self {
        Self { alt, shift, ctrl }
    }
}

#[derive(Component, Clone, PartialEq, Debug, Reflect, Default)]
#[reflect(Component)]
pub struct PointerTarget {
    pub down_target: Option<PointerDownTarget>,
    pub down_button: Option<PointerButton>,
    pub modifiers: Modifiers,
}

impl PointerTarget {
    pub fn new(
        down_target: Option<PointerDownTarget>,
        down_button: Option<PointerButton>,
        modifiers: Modifiers,
    ) -> Self {
        Self {
            down_target,
            down_button,
            modifiers,
        }
    }
    /// Begin a new press ("pending" gesture).
    pub fn begin_press(
        &mut self,
        target: PointerDownTarget,
        button: PointerButton,
        modifiers: Modifiers,
    ) {
        self.down_target = Some(target);
        self.down_button = Some(button);
        self.modifiers = modifiers;
    }

    /// End the current press (clears pending context).
    pub fn end_press(&mut self) {
        self.down_target = None;
        self.down_button = None;
        self.modifiers = Modifiers::default();
    }

    pub fn down_target(&self) -> Option<PointerDownTarget> {
        self.down_target
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum PointerDownTarget {
    Canvas,
    Ui,
    NodeRoot(Entity, NodeId),
    Widget(Entity, NodeId),
}

fn resolve_cursor_visual_state(
    pointer_state: Res<State<PointerState>>,
    pointer_target: Single<&PointerTarget, With<Cursor>>,
    hovered_nodes: Query<&Hovered, With<CanvasNode>>,
    hovered_buttons: Query<&Hovered, With<Button>>,
    hovered_widget_children: Query<&Hovered, With<InteractiveChild>>,
    hovered_hub_nodes: Query<&Hovered, With<ProjectHubNode>>,
    hovered_close_buttons: Query<&Hovered, With<NodeCloseButton>>,
    mut cursor: Single<&mut CursorVisualStateCache, With<Cursor>>,
) {
    let has_hover_action = any_hovered(&hovered_widget_children)
        || any_hovered(&hovered_buttons)
        || any_hovered(&hovered_hub_nodes)
        || any_hovered(&hovered_close_buttons);
    let has_hover_node = any_hovered(&hovered_nodes);
    let down_target = pointer_target.down_target();
    let state = resolve_state(
        pointer_state.get().clone(),
        down_target,
        has_hover_action,
        has_hover_node,
    );
    cursor.0 = state;
}

fn apply_cursor_frame_and_hotspot(
    visual_map: Res<CursorVisualMap>,
    cursor: Single<(&CursorVisualStateCache, &mut CursorHotspot, &mut Sprite), With<Cursor>>,
) {
    let (state, mut hotspot, mut sprite) = cursor.into_inner();
    let frame = visual_map.frame(state.0);
    let atlas_index = if frame.atlas_index < CURSOR_TILE_COUNT {
        frame.atlas_index
    } else {
        visual_map.default.atlas_index
    };

    if let Some(atlas) = sprite.texture_atlas.as_mut() {
        atlas.index = atlas_index;
    }
    hotspot.0 = frame.hotspot_world;
}

fn update_cursor_sprite_position(
    window: Single<&Window, With<PrimaryWindow>>,
    cursor_camera: Option<Single<(&Camera, &GlobalTransform), With<CursorCamera>>>,
    ui_camera: Option<Single<(&Camera, &GlobalTransform), With<UiCamera>>>,
    cursor: Single<(&mut Transform, &CursorHotspot, &mut Visibility), With<Cursor>>,
) {
    let Some(cursor_viewport_pos) = window.cursor_position() else {
        let (_, _, mut visibility) = cursor.into_inner();
        *visibility = Visibility::Hidden;
        return;
    };

    let cursor_ui_pos = match (cursor_camera, ui_camera) {
        (Some(cursor_camera), _) => {
            let (camera, camera_transform) = cursor_camera.into_inner();
            camera
                .viewport_to_world_2d(camera_transform, cursor_viewport_pos)
                .ok()
        }
        (None, Some(ui_camera)) => {
            let (camera, camera_transform) = ui_camera.into_inner();
            camera
                .viewport_to_world_2d(camera_transform, cursor_viewport_pos)
                .ok()
        }
        (None, None) => {
            let half_w = window.width() * 0.5;
            let half_h = window.height() * 0.5;
            Some(Vec2::new(
                cursor_viewport_pos.x - half_w,
                half_h - cursor_viewport_pos.y,
            ))
        }
    };

    let Some(cursor_ui_pos) = cursor_ui_pos else {
        let (_, _, mut visibility) = cursor.into_inner();
        *visibility = Visibility::Hidden;
        return;
    };

    let (mut transform, hotspot, mut visibility) = cursor.into_inner();
    *visibility = Visibility::Visible;
    transform.translation = Vec3::new(
        cursor_ui_pos.x + hotspot.0.x,
        cursor_ui_pos.y + hotspot.0.y,
        CURSOR_Z,
    );
}

fn any_hovered<F: QueryFilter>(query: &Query<&Hovered, F>) -> bool {
    query.iter().any(|hovered| hovered.0)
}

fn resolve_state(
    pointer_state: PointerState,
    down_target: Option<PointerDownTarget>,
    has_hover_action: bool,
    has_hover_node: bool,
) -> CursorVisualState {
    match pointer_state {
        PointerState::PanningCanvas => CursorVisualState::Panning,
        PointerState::DraggingNode => CursorVisualState::DraggingNode,
        PointerState::MarqueeSelecting => CursorVisualState::Marquee,
        PointerState::InteractingWithWidget => CursorVisualState::HoverAction,
        PointerState::Armed => match down_target {
            Some(PointerDownTarget::Widget(..) | PointerDownTarget::Ui) => {
                CursorVisualState::HoverAction
            }
            Some(PointerDownTarget::NodeRoot(..)) => CursorVisualState::HoverNode,
            Some(PointerDownTarget::Canvas) | None => CursorVisualState::Default,
        },
        _ if has_hover_action => CursorVisualState::HoverAction,
        _ if has_hover_node => CursorVisualState::HoverNode,
        _ => CursorVisualState::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_state_priority_panning_overrides_hover() {
        let state = resolve_state(PointerState::PanningCanvas, None, true, true);
        assert_eq!(state, CursorVisualState::Panning);
    }

    #[test]
    fn cursor_state_priority_drag_overrides_hover() {
        let state = resolve_state(PointerState::DraggingNode, None, true, true);
        assert_eq!(state, CursorVisualState::DraggingNode);
    }

    #[test]
    fn cursor_state_defaults_when_idle_and_no_hover() {
        let state = resolve_state(PointerState::Idle, None, false, false);
        assert_eq!(state, CursorVisualState::Default);
    }

    #[test]
    fn cursor_state_hover_action_beats_node_hover() {
        let state = resolve_state(PointerState::Idle, None, true, true);
        assert_eq!(state, CursorVisualState::HoverAction);
    }

    #[test]
    fn cursor_state_armed_widget_uses_action() {
        let state = resolve_state(
            PointerState::Armed,
            Some(PointerDownTarget::Widget(
                Entity::PLACEHOLDER,
                NodeId::new(),
            )),
            false,
            false,
        );
        assert_eq!(state, CursorVisualState::HoverAction);
    }

    #[test]
    fn cursor_state_armed_ui_uses_action() {
        let state = resolve_state(
            PointerState::Armed,
            Some(PointerDownTarget::Ui),
            false,
            false,
        );
        assert_eq!(state, CursorVisualState::HoverAction);
    }

    #[test]
    fn cursor_state_interacting_widget_uses_action() {
        let state = resolve_state(PointerState::InteractingWithWidget, None, false, false);
        assert_eq!(state, CursorVisualState::HoverAction);
    }

    #[test]
    fn cursor_map_has_expected_hotspots() {
        let map = CursorVisualMap::default();
        assert_eq!(
            map.frame(CursorVisualState::Default).hotspot_world,
            Vec2::new(4.0, -2.0)
        );
        assert_eq!(
            map.frame(CursorVisualState::Marquee).hotspot_world,
            Vec2::ZERO
        );
    }

    #[test]
    fn cursor_indices_use_valid_tiles() {
        let map = CursorVisualMap::default();
        let indices = [
            map.frame(CursorVisualState::Default).atlas_index,
            map.frame(CursorVisualState::HoverNode).atlas_index,
            map.frame(CursorVisualState::HoverAction).atlas_index,
            map.frame(CursorVisualState::Marquee).atlas_index,
            map.frame(CursorVisualState::DraggingNode).atlas_index,
            map.frame(CursorVisualState::Panning).atlas_index,
        ];

        for index in indices {
            assert!(
                index < CURSOR_TILE_COUNT,
                "cursor atlas index out of range: {index}"
            );
            assert!(
                !CURSOR_NONVISIBLE_RESERVED_RANGE.contains(&index),
                "cursor atlas index {index} maps to a reserved/blank cursor tile range"
            );
        }
    }
}
