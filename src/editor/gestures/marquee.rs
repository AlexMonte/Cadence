//! Marquee drag gesture lifecycle and selection-intent emission.

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::EditorSystemSet;
use crate::editor::camera::CANVAS_RENDER_LAYER;
use crate::editor::gestures::PointerState;
use crate::editor::input::{Cursor, Modifiers, PointerPosition, PointerTarget};
use crate::editor::semantics::SelectionIntent;
use crate::editor::state::EditorInteractive;

const MARQUEE_Z: f32 = 0.9;
const MARQUEE_FILL: Color = Color::Srgba(Srgba::new(0.2, 0.6, 1.0, 0.2));

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<MarqueeState>();
    app.add_systems(
        OnEnter(PointerState::MarqueeSelecting),
        start_marquee.run_if(in_state(EditorInteractive)),
    );
    app.add_systems(
        Update,
        update_marquee
            .run_if(in_state(PointerState::MarqueeSelecting))
            .run_if(in_state(EditorInteractive))
            .in_set(EditorSystemSet::DragComputation),
    );
    app.add_systems(OnExit(PointerState::MarqueeSelecting), finish_marquee);
}

#[derive(Resource, Debug, Default)]
pub struct MarqueeState {
    start_world: Option<Vec2>,
    current_world: Option<Vec2>,
    modifiers: Modifiers,
}

#[derive(Component)]
pub struct MarqueeVisual;

impl MarqueeState {
    fn rect_world(&self) -> Option<Rect> {
        Some(marquee_rect_from_points(
            self.start_world?,
            self.current_world?,
        ))
    }

    fn reset(&mut self) {
        self.start_world = None;
        self.current_world = None;
        self.modifiers = Modifiers::default();
    }
}

fn start_marquee(
    pointer: Single<(&PointerPosition, &PointerTarget), With<Cursor>>,
    mut state: ResMut<MarqueeState>,
    mut commands: Commands,
) {
    let (pos, target) = pointer.into_inner();
    let Some(start) = pos.down_world_pos() else {
        state.reset();
        return;
    };

    let current = pos.world_pos();
    state.start_world = Some(start);
    state.current_world = Some(current);
    state.modifiers = target.modifiers();

    if let Some(rect) = state.rect_world() {
        let (size, center) = rect_size_center(rect);
        commands.spawn((
            Sprite {
                color: MARQUEE_FILL,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_translation(Vec3::new(center.x, center.y, MARQUEE_Z)),
            RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
            MarqueeVisual,
            Name::new("MarqueeVisual"),
        ));
    }
}

fn update_marquee(
    pointer: Single<&PointerPosition, With<Cursor>>,
    mut state: ResMut<MarqueeState>,
    mut visuals: Query<(&mut Sprite, &mut Transform), With<MarqueeVisual>>,
) {
    if state.start_world.is_none() {
        return;
    }

    state.current_world = Some(pointer.world_pos());
    let Some(rect) = state.rect_world() else {
        return;
    };
    let (size, center) = rect_size_center(rect);

    let Ok((mut sprite, mut transform)) = visuals.single_mut() else {
        return;
    };
    sprite.custom_size = Some(size);
    transform.translation = Vec3::new(center.x, center.y, MARQUEE_Z);
}

fn finish_marquee(
    mut state: ResMut<MarqueeState>,
    mut intents: MessageWriter<SelectionIntent>,
    mut commands: Commands,
    visuals: Query<Entity, With<MarqueeVisual>>,
) {
    if let Some(rect_world) = state.rect_world() {
        intents.write(SelectionIntent::Marquee {
            rect_world,
            modifiers: state.modifiers,
        });
    }

    for entity in &visuals {
        commands.entity(entity).try_despawn();
    }

    state.reset();
}

fn marquee_rect_from_points(a: Vec2, b: Vec2) -> Rect {
    Rect::from_corners(a, b)
}

fn rect_size_center(rect: Rect) -> (Vec2, Vec2) {
    let size = rect.max - rect.min;
    let center = rect.min + size / 2.0;
    (size, center)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invariants:
    // - marquee rect ordering is min/max stable regardless of drag direction.
    // - rect_size_center computes positive size and midpoint correctly.
    // - finish emits one marquee intent for complete state.
    // - finish always resets state and despawns visuals.

    #[test]
    fn marquee_rect_from_points_orders_min_max() {
        let rect = marquee_rect_from_points(Vec2::new(5.0, 1.0), Vec2::new(-1.0, 10.0));
        assert_eq!(rect.min, Vec2::new(-1.0, 1.0));
        assert_eq!(rect.max, Vec2::new(5.0, 10.0));
    }

    #[test]
    fn rect_size_center_handles_reverse_drag() {
        let rect = marquee_rect_from_points(Vec2::new(20.0, 5.0), Vec2::new(-4.0, 15.0));
        let (size, center) = rect_size_center(rect);
        assert_eq!(size, Vec2::new(24.0, 10.0));
        assert_eq!(center, Vec2::new(8.0, 10.0));
    }

    #[test]
    fn finish_marquee_emits_once_and_cleans_up() {
        let mut app = App::new();
        app.add_message::<SelectionIntent>();
        app.init_resource::<MarqueeState>();
        app.add_systems(Update, finish_marquee);

        app.world_mut().spawn((MarqueeVisual, Transform::default()));
        {
            let mut state = app.world_mut().resource_mut::<MarqueeState>();
            state.start_world = Some(Vec2::new(-1.0, -1.0));
            state.current_world = Some(Vec2::new(3.0, 2.0));
            state.modifiers = Modifiers {
                shift: true,
                ..Default::default()
            };
        }

        app.update();

        let mut cursor = app
            .world()
            .resource::<Messages<SelectionIntent>>()
            .get_cursor();
        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<SelectionIntent>>())
            .copied()
            .collect();
        assert_eq!(intents.len(), 1);
        match intents[0] {
            SelectionIntent::Marquee {
                rect_world,
                modifiers,
            } => {
                assert_eq!(rect_world.min, Vec2::new(-1.0, -1.0));
                assert_eq!(rect_world.max, Vec2::new(3.0, 2.0));
                assert!(modifiers.shift);
            }
            _ => panic!("expected marquee intent"),
        }

        let state = app.world().resource::<MarqueeState>();
        assert!(state.start_world.is_none());
        assert!(state.current_world.is_none());
        assert_eq!(state.modifiers, Modifiers::default());

        let visual_count = {
            let world = app.world_mut();
            let mut visuals = world.query_filtered::<Entity, With<MarqueeVisual>>();
            visuals.iter(world).count()
        };
        assert_eq!(visual_count, 0);
    }

    #[test]
    fn finish_marquee_with_incomplete_state_emits_nothing() {
        let mut app = App::new();
        app.add_message::<SelectionIntent>();
        app.init_resource::<MarqueeState>();
        app.add_systems(Update, finish_marquee);
        {
            let mut state = app.world_mut().resource_mut::<MarqueeState>();
            state.start_world = Some(Vec2::ZERO);
            state.current_world = None;
        }
        app.update();

        let mut cursor = app
            .world()
            .resource::<Messages<SelectionIntent>>()
            .get_cursor();
        let intents: Vec<_> = cursor
            .read(app.world().resource::<Messages<SelectionIntent>>())
            .copied()
            .collect();
        assert!(intents.is_empty());
    }
}
