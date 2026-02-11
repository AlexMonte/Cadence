//! Selection semantic reducer and selection-visual maintenance for canvas nodes.

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::EditorSystemSet;
use crate::core::NodeId;
use crate::editor::camera::CANVAS_RENDER_LAYER;
use crate::editor::canvas::{CanvasNode, NodeStackIntent};
use crate::editor::input::Modifiers;
use crate::editor::state::EditorReady;

const CORNER_LENGTH: f32 = 14.0;
const CORNER_THICKNESS: f32 = 2.5;
const CORNER_MARGIN: f32 = 5.0;
const GLOW_PADDING: Vec2 = Vec2::new(36.0, 26.0);
const SELECTION_PULSE_FREQ: f32 = 2.2;
const PRIMARY_SCALE_AMPLITUDE: f32 = 0.035;
const SECONDARY_SCALE_AMPLITUDE: f32 = 0.02;
const GLOW_Z: f32 = 0.01;
const CORNER_Z: f32 = 0.03;
const VISUAL_ROOT_Z: f32 = 0.15;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<SelectionIntent>();
    app.init_resource::<SelectionState>();
    app.init_resource::<SelectionVisualAssets>();
    app.add_systems(
        Update,
        apply_selection_intents
            .run_if(in_state(EditorReady))
            .in_set(EditorSystemSet::SelectionResolution),
    );
    app.add_systems(
        Update,
        (
            ensure_selection_visual_children,
            update_selection_visual_state,
            animate_selection_pulse,
        )
            .chain()
            .run_if(in_state(EditorReady))
            .in_set(EditorSystemSet::RenderSelection),
    );
}

#[derive(Message, Debug, Clone, Copy)]
pub enum SelectionIntent {
    ClickNode {
        node_id: NodeId,
        modifiers: Modifiers,
    },
    ClickCanvas {
        modifiers: Modifiers,
    },
    Marquee {
        rect_world: Rect,
        modifiers: Modifiers,
    },
}

#[derive(Resource, Debug, Default)]
pub struct SelectionState {
    pub selected: Vec<NodeId>,
    pub primary: Option<NodeId>,
}

impl SelectionState {
    pub fn is_selected(&self, node_id: NodeId) -> bool {
        self.selected.contains(&node_id)
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.primary = None;
    }

    pub fn select_single(&mut self, node_id: NodeId) {
        self.selected.clear();
        self.selected.push(node_id);
        self.primary = Some(node_id);
    }

    pub fn add(&mut self, node_id: NodeId) {
        if !self.is_selected(node_id) {
            self.selected.push(node_id);
            if self.primary.is_none() {
                self.primary = Some(node_id);
            }
        }
    }

    pub fn toggle(&mut self, node_id: NodeId) {
        if let Some(index) = self.selected.iter().position(|id| *id == node_id) {
            self.selected.remove(index);
            if self.primary == Some(node_id) {
                self.primary = self.selected.first().copied();
            }
        } else {
            self.selected.push(node_id);
            if self.primary.is_none() {
                self.primary = Some(node_id);
            }
        }
    }

    pub fn set_from_iter<I: IntoIterator<Item = NodeId>>(&mut self, ids: I) {
        self.selected = ids.into_iter().collect();
        self.primary = self.selected.first().copied();
    }
}

pub fn apply_selection_intents(
    mut intents: MessageReader<SelectionIntent>,
    mut selection: ResMut<SelectionState>,
    nodes: Query<(&CanvasNode, &Transform)>,
    mut stack_intents: MessageWriter<NodeStackIntent>,
) {
    for intent in intents.read() {
        match *intent {
            SelectionIntent::ClickNode { node_id, modifiers } => {
                apply_click_node(&mut selection, node_id, modifiers);
                if !modifiers.ctrl && !modifiers.shift {
                    stack_intents.write(NodeStackIntent::BringToFront {
                        node_ids: vec![node_id],
                    });
                }
            }
            SelectionIntent::ClickCanvas { modifiers } => {
                apply_click_canvas(&mut selection, modifiers);
            }
            SelectionIntent::Marquee {
                rect_world,
                modifiers,
            } => {
                let hits = nodes_in_rect(rect_world, nodes.iter());
                apply_multi_select(&mut selection, hits, modifiers);
            }
        }
    }
}

fn apply_click_node(selection: &mut SelectionState, node_id: NodeId, modifiers: Modifiers) {
    if modifiers.ctrl {
        selection.toggle(node_id);
    } else if modifiers.shift {
        selection.add(node_id);
    } else {
        selection.select_single(node_id);
    }
}

fn apply_click_canvas(selection: &mut SelectionState, modifiers: Modifiers) {
    if modifiers.shift || modifiers.ctrl {
        return;
    }
    selection.clear();
}

fn apply_multi_select(selection: &mut SelectionState, hits: Vec<NodeId>, modifiers: Modifiers) {
    if modifiers.ctrl {
        for node_id in hits {
            selection.toggle(node_id);
        }
    } else if modifiers.shift {
        for node_id in hits {
            selection.add(node_id);
        }
    } else {
        selection.set_from_iter(hits);
    }
}

fn nodes_in_rect<'a>(
    rect_world: Rect,
    nodes: impl Iterator<Item = (&'a CanvasNode, &'a Transform)>,
) -> Vec<NodeId> {
    nodes
        .filter_map(|(node, transform)| {
            let node_rect = Rect::from_center_size(
                transform.translation.truncate(),
                Vec2::new(node.width, node.height),
            );
            if rects_intersect(rect_world, node_rect) {
                Some(node.node_id)
            } else {
                None
            }
        })
        .collect()
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.min.x <= b.max.x && a.max.x >= b.min.x && a.min.y <= b.max.y && a.max.y >= b.min.y
}

#[derive(Resource, Clone)]
pub struct SelectionVisualAssets {
    glow_texture: Handle<Image>,
}

impl FromWorld for SelectionVisualAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            glow_texture: assets.load("images/ui/fx/light_blue.png"),
        }
    }
}

#[derive(Component)]
struct SelectionVisualLink;

#[derive(Component)]
pub struct SelectionVisualRoot {
    node_id: NodeId,
}

#[derive(Component)]
pub struct SelectionCorner;

#[derive(Component)]
pub struct SelectionGlow;

#[derive(Component)]
struct SelectionPulsePhase(f32);

fn ensure_selection_visual_children(
    mut commands: Commands,
    nodes_without_visuals: Query<(Entity, &CanvasNode), Without<SelectionVisualLink>>,
    visual_assets: Res<SelectionVisualAssets>,
    asset_server: Res<AssetServer>,
) {
    let glow_failed = asset_server
        .load_state(&visual_assets.glow_texture)
        .is_failed();

    for (node_entity, node) in &nodes_without_visuals {
        let phase = phase_from_node(node.node_id);
        let root = commands
            .spawn((
                Name::new("SelectionVisualRoot"),
                SelectionVisualRoot {
                    node_id: node.node_id,
                },
                SelectionPulsePhase(phase),
                Transform::from_translation(Vec3::new(0.0, 0.0, VISUAL_ROOT_Z)),
                GlobalTransform::default(),
                Visibility::Hidden,
                InheritedVisibility::default(),
                ViewVisibility::default(),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();

        commands.entity(root).with_children(|children| {
            let mut glow = if glow_failed {
                Sprite::from_color(
                    Color::Srgba(Srgba::new(0.12, 0.82, 1.0, 0.18)),
                    Vec2::new(node.width + GLOW_PADDING.x, node.height + GLOW_PADDING.y),
                )
            } else {
                Sprite::from_image(visual_assets.glow_texture.clone())
            };
            glow.custom_size = Some(Vec2::new(
                node.width + GLOW_PADDING.x,
                node.height + GLOW_PADDING.y,
            ));
            glow.color = Color::Srgba(Srgba::new(0.12, 0.82, 1.0, 0.18));

            children.spawn((
                Name::new("SelectionGlow"),
                glow,
                Transform::from_xyz(0.0, 0.0, GLOW_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
                SelectionGlow,
            ));

            for (sign_x, sign_y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner_x = sign_x * ((node.width * 0.5) + CORNER_MARGIN);
                let corner_y = sign_y * ((node.height * 0.5) + CORNER_MARGIN);

                children.spawn((
                    Name::new("SelectionCornerH"),
                    Sprite::from_color(
                        Color::Srgba(Srgba::new(0.22, 0.75, 0.95, 0.72)),
                        Vec2::new(CORNER_LENGTH, CORNER_THICKNESS),
                    ),
                    Transform::from_xyz(
                        corner_x - (sign_x * CORNER_LENGTH * 0.5),
                        corner_y,
                        CORNER_Z,
                    ),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::IGNORE,
                    SelectionCorner,
                ));

                children.spawn((
                    Name::new("SelectionCornerV"),
                    Sprite::from_color(
                        Color::Srgba(Srgba::new(0.22, 0.75, 0.95, 0.72)),
                        Vec2::new(CORNER_THICKNESS, CORNER_LENGTH),
                    ),
                    Transform::from_xyz(
                        corner_x,
                        corner_y - (sign_y * CORNER_LENGTH * 0.5),
                        CORNER_Z,
                    ),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::IGNORE,
                    SelectionCorner,
                ));
            }
        });

        commands
            .entity(node_entity)
            .add_child(root)
            .insert(SelectionVisualLink);
    }
}

fn update_selection_visual_state(
    selection: Res<SelectionState>,
    mut roots: Query<(
        &SelectionVisualRoot,
        &Children,
        &mut Visibility,
        &mut Transform,
    )>,
    mut corners: Query<&mut Sprite, (With<SelectionCorner>, Without<SelectionGlow>)>,
    mut glows: Query<&mut Sprite, (With<SelectionGlow>, Without<SelectionCorner>)>,
) {
    for (root, children, mut visibility, mut transform) in &mut roots {
        let is_selected = selection.is_selected(root.node_id);
        if !is_selected {
            *visibility = Visibility::Hidden;
            transform.scale = Vec3::ONE;
            continue;
        }

        *visibility = Visibility::Visible;
        transform.scale = Vec3::ONE;

        let is_primary = selection.primary == Some(root.node_id);
        let corner_color = if is_primary {
            Color::Srgba(Srgba::new(0.16, 0.96, 1.0, 0.95))
        } else {
            Color::Srgba(Srgba::new(0.22, 0.75, 0.95, 0.72))
        };
        let glow_color = if is_primary {
            Color::Srgba(Srgba::new(0.14, 0.86, 1.0, 0.30))
        } else {
            Color::Srgba(Srgba::new(0.14, 0.76, 0.95, 0.18))
        };

        for child in children.iter() {
            if let Ok(mut corner_sprite) = corners.get_mut(child) {
                corner_sprite.color = corner_color;
            }
            if let Ok(mut glow_sprite) = glows.get_mut(child) {
                glow_sprite.color = glow_color;
            }
        }
    }
}

fn animate_selection_pulse(
    time: Res<Time>,
    selection: Res<SelectionState>,
    mut roots: Query<(
        &SelectionVisualRoot,
        &SelectionPulsePhase,
        &Children,
        &mut Transform,
        &Visibility,
    )>,
    mut glows: Query<&mut Sprite, (With<SelectionGlow>, Without<SelectionCorner>)>,
) {
    let t = time.elapsed_secs();
    for (root, phase, children, mut transform, visibility) in &mut roots {
        if *visibility == Visibility::Hidden || !selection.is_selected(root.node_id) {
            continue;
        }

        let is_primary = selection.primary == Some(root.node_id);
        let pulse = ((t * SELECTION_PULSE_FREQ) + phase.0).sin() * 0.5 + 0.5;
        let amplitude = if is_primary {
            PRIMARY_SCALE_AMPLITUDE
        } else {
            SECONDARY_SCALE_AMPLITUDE
        };
        transform.scale = Vec3::splat(1.0 + (pulse * amplitude));

        let (glow_base, glow_range) = if is_primary {
            (0.24, 0.10)
        } else {
            (0.16, 0.06)
        };
        let glow_alpha = glow_base + (pulse * glow_range);
        for child in children.iter() {
            if let Ok(mut glow_sprite) = glows.get_mut(child) {
                glow_sprite.color = Color::Srgba(Srgba::new(0.14, 0.86, 1.0, glow_alpha));
            }
        }
    }
}

fn phase_from_node(node_id: NodeId) -> f32 {
    let bits = node_id.as_uuid().as_u128() as u64;
    let unit = (bits & 0xffff) as f32 / 65535.0;
    unit * std::f32::consts::TAU
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Project;
    use crate::editor::AppState;

    // Invariants:
    // - SelectionState preserves primary/selected consistency and uniqueness behavior.
    // - rect intersection and marquee hit selection are correct for overlap boundaries.
    // - plain click emits BringToFront, modifier clicks do not.
    // - canvas click clears only without additive/toggle modifiers.

    #[test]
    fn selection_state_core_invariants() {
        let id_a = NodeId::new();
        let id_b = NodeId::new();
        let mut selection = SelectionState::default();

        selection.clear();
        assert!(selection.selected.is_empty());
        assert_eq!(selection.primary, None);

        selection.select_single(id_a);
        assert_eq!(selection.selected, vec![id_a]);
        assert_eq!(selection.primary, Some(id_a));

        selection.add(id_b);
        assert!(selection.is_selected(id_a));
        assert!(selection.is_selected(id_b));
        assert_eq!(selection.primary, Some(id_a));

        selection.toggle(id_a);
        assert!(!selection.is_selected(id_a));
        assert!(selection.is_selected(id_b));
        assert_eq!(selection.primary, Some(id_b));

        selection.set_from_iter([id_a, id_b]);
        assert_eq!(selection.selected.len(), 2);
        assert_eq!(selection.primary, Some(id_a));
    }

    #[test]
    fn rect_intersection_boundaries_are_inclusive() {
        let a = Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let b = Rect::from_corners(Vec2::new(10.0, 10.0), Vec2::new(15.0, 15.0));
        let c = Rect::from_corners(Vec2::new(11.0, 11.0), Vec2::new(12.0, 12.0));

        assert!(
            rects_intersect(a, b),
            "touching edges should count as intersecting"
        );
        assert!(
            !rects_intersect(a, c),
            "separated rects should not intersect"
        );
    }

    #[test]
    fn apply_selection_intents_emits_stack_only_for_plain_click_and_canvas_clear_rules() {
        let mut app = App::new();
        app.add_message::<SelectionIntent>();
        app.add_message::<NodeStackIntent>();
        app.init_resource::<SelectionState>();
        app.insert_resource(AppState::new(Project::new("sel-test".to_string())));
        app.add_systems(Update, apply_selection_intents);

        let id_a = NodeId::new();
        app.world_mut().spawn((
            CanvasNode {
                node_id: id_a,
                width: 20.0,
                height: 20.0,
            },
            Transform::default(),
        ));

        app.world_mut().write_message(SelectionIntent::ClickNode {
            node_id: id_a,
            modifiers: Modifiers::default(),
        });
        app.update();

        {
            let selection = app.world().resource::<SelectionState>();
            assert_eq!(selection.selected, vec![id_a]);
            assert_eq!(selection.primary, Some(id_a));
        }

        let mut stack_cursor = app
            .world()
            .resource::<Messages<NodeStackIntent>>()
            .get_cursor();
        let stack_events: Vec<_> = stack_cursor
            .read(app.world().resource::<Messages<NodeStackIntent>>())
            .cloned()
            .collect();
        assert_eq!(stack_events.len(), 1);

        app.world_mut().write_message(SelectionIntent::ClickNode {
            node_id: id_a,
            modifiers: Modifiers {
                shift: true,
                ..Default::default()
            },
        });
        app.update();

        let stack_events_after_shift: Vec<_> = stack_cursor
            .read(app.world().resource::<Messages<NodeStackIntent>>())
            .cloned()
            .collect();
        assert!(
            stack_events_after_shift.is_empty(),
            "shift click must not emit bring-to-front"
        );

        app.world_mut().write_message(SelectionIntent::ClickCanvas {
            modifiers: Modifiers::default(),
        });
        app.update();
        {
            let selection = app.world().resource::<SelectionState>();
            assert!(selection.selected.is_empty());
            assert_eq!(selection.primary, None);
        }

        app.world_mut().write_message(SelectionIntent::ClickNode {
            node_id: id_a,
            modifiers: Modifiers::default(),
        });
        app.update();
        app.world_mut().write_message(SelectionIntent::ClickCanvas {
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        });
        app.update();
        {
            let selection = app.world().resource::<SelectionState>();
            assert_eq!(selection.selected, vec![id_a]);
            assert_eq!(selection.primary, Some(id_a));
        }

        let id_b = NodeId::new();
        app.world_mut().spawn((
            CanvasNode {
                node_id: id_b,
                width: 20.0,
                height: 20.0,
            },
            Transform::from_xyz(90.0, 90.0, 0.0),
        ));
        app.world_mut().write_message(SelectionIntent::Marquee {
            rect_world: Rect::from_corners(Vec2::new(-30.0, -30.0), Vec2::new(30.0, 30.0)),
            modifiers: Modifiers::default(),
        });
        app.update();
        {
            let selection = app.world().resource::<SelectionState>();
            assert_eq!(selection.selected, vec![id_a]);
        }
    }
}
