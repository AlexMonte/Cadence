//! Cursor interaction state machine ([`seldom_state`]) — pointer semantics only.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use seldom_state::prelude::*;
use seldom_state::set::StateSet;

use crate::application::editor::MusaicEditorSet;
use crate::domain::document::TileSpawnKind;

use super::cursor_logic::{CursorInteractionPhase, pan_dragging, panning_active};
use super::session::EditorSession;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorInteraction {
    phase: CursorInteractionPhase,
}

impl CursorInteraction {
    pub fn phase(&self) -> CursorInteractionPhase {
        self.phase
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorInteractionChanged {
    pub from: CursorInteractionPhase,
    pub to: CursorInteractionPhase,
}

/// Raycast hover over a pickable board target (filled by infrastructure each frame).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct BoardCursorHover {
    pub pickable: bool,
}

/// Pointer sample for board placement (filled by infrastructure each frame).
#[derive(Resource, Default, Debug, Clone)]
pub struct BoardPlacementPointer {
    pub window_cursor: Option<Vec2>,
    pub viewport_cursor: Option<Vec2>,
    pub cursor_world: Option<Vec3>,
}

#[derive(Debug, Clone)]
pub struct DrawerTilePressed {
    pub tile: TileSpawnKind,
    pub start_screen: Vec2,
}

/// Written by the drawer UI on pointer-down.
#[derive(Resource, Default)]
pub struct DrawerTilePressQueue {
    pub pending: Option<DrawerTilePressed>,
}

/// Viewport pan/orbit latch (written by infrastructure camera nav each frame).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct BoardViewportPointerInput {
    pub viewport_pan_active: bool,
    pub middle_orbit_active: bool,
}

#[derive(Resource, Default)]
struct CursorPanLatch {
    pan_dragging: bool,
}

#[derive(Component)]
struct CursorStateMachineRoot;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct CursorIdle;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct CursorPanning;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct CursorHoveringTile;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct CursorDragging;

#[derive(Resource, Debug, Clone, Copy)]
struct CursorStateMachineEntity(Entity);

/// Registers the cursor interaction state machine. Called by the editor plugin.
pub fn register_cursor_interaction(app: &mut App) {
    app.init_resource::<CursorInteraction>()
        .init_resource::<BoardCursorHover>()
        .init_resource::<BoardPlacementPointer>()
        .init_resource::<BoardViewportPointerInput>()
        .init_resource::<DrawerTilePressQueue>()
        .init_resource::<CursorPanLatch>()
        .add_message::<CursorInteractionChanged>()
        .add_plugins(StateMachinePlugin::default().schedule(Update))
        .add_systems(Startup, spawn_cursor_state_machine)
        .add_systems(
            Update,
            (
                latch_pan_dragging.before(StateSet::Transition),
                sync_cursor_interaction
                    .after(StateSet::Transition)
                    .in_set(MusaicEditorSet::MutateState),
            ),
        );
}

fn spawn_cursor_state_machine(mut commands: Commands) {
    let entity = commands
        .spawn((
            CursorStateMachineRoot,
            CursorIdle,
            StateMachine::default()
                .trans::<CursorIdle, _>(enter_panning, CursorPanning)
                .trans::<CursorIdle, _>(enter_dragging, CursorDragging)
                .trans::<CursorIdle, _>(enter_hovering_tile, CursorHoveringTile)
                .trans::<CursorPanning, _>(exit_panning, CursorIdle)
                .trans::<CursorPanning, _>(enter_dragging_from_pan, CursorDragging)
                .trans::<CursorHoveringTile, _>(enter_panning, CursorPanning)
                .trans::<CursorHoveringTile, _>(enter_dragging, CursorDragging)
                .trans::<CursorHoveringTile, _>(exit_hovering_tile, CursorIdle)
                .trans::<CursorDragging, _>(exit_dragging, CursorIdle)
                .trans::<CursorDragging, _>(enter_panning, CursorPanning),
        ))
        .id();
    commands.insert_resource(CursorStateMachineEntity(entity));
}

fn enter_panning(pointer: Res<BoardViewportPointerInput>) -> bool {
    panning_active(pointer.viewport_pan_active, pointer.middle_orbit_active)
}

fn exit_panning(pointer: Res<BoardViewportPointerInput>) -> bool {
    !panning_active(pointer.viewport_pan_active, pointer.middle_orbit_active)
}

fn enter_dragging(session: Res<EditorSession>) -> bool {
    session.is_placing_from_drawer()
}

fn enter_dragging_from_pan(session: Res<EditorSession>) -> bool {
    session.is_placing_from_drawer()
}

fn exit_dragging(session: Res<EditorSession>) -> bool {
    !session.is_placing_from_drawer()
}

fn enter_hovering_tile(
    hover: Res<BoardCursorHover>,
    pointer: Res<BoardViewportPointerInput>,
    session: Res<EditorSession>,
) -> bool {
    !session.is_placing_from_drawer()
        && !panning_active(pointer.viewport_pan_active, pointer.middle_orbit_active)
        && hover.pickable
}

fn exit_hovering_tile(
    hover: Res<BoardCursorHover>,
    pointer: Res<BoardViewportPointerInput>,
    session: Res<EditorSession>,
) -> bool {
    session.is_placing_from_drawer()
        || panning_active(pointer.viewport_pan_active, pointer.middle_orbit_active)
        || !hover.pickable
}

fn latch_pan_dragging(
    pointer: Res<BoardViewportPointerInput>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut latch: ResMut<CursorPanLatch>,
    mut last_cursor: Local<Option<Vec2>>,
) {
    let cursor_moved = windows
        .iter()
        .next()
        .and_then(|w| w.cursor_position())
        .map(|cursor| {
            last_cursor
                .map(|prev| cursor.distance(prev) > 0.5)
                .unwrap_or(false)
        })
        .unwrap_or(false);

    if let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) {
        *last_cursor = Some(cursor);
    } else {
        *last_cursor = None;
    }

    latch.pan_dragging = pan_dragging(pointer.viewport_pan_active, cursor_moved);
}

fn sync_cursor_interaction(
    machine: Res<CursorStateMachineEntity>,
    states: Query<
        (
            Has<CursorIdle>,
            Has<CursorPanning>,
            Has<CursorHoveringTile>,
            Has<CursorDragging>,
        ),
        With<CursorStateMachineRoot>,
    >,
    mut cursor: ResMut<CursorInteraction>,
    mut changed: MessageWriter<CursorInteractionChanged>,
) {
    let Ok((idle, panning, hovering, dragging)) = states.get(machine.0) else {
        return;
    };

    let next = if panning {
        CursorInteractionPhase::Panning
    } else if dragging {
        CursorInteractionPhase::Dragging
    } else if hovering {
        CursorInteractionPhase::HoveringTile
    } else if idle {
        CursorInteractionPhase::Idle
    } else {
        CursorInteractionPhase::Idle
    };

    if cursor.phase != next {
        changed.write(CursorInteractionChanged {
            from: cursor.phase,
            to: next,
        });
        cursor.phase = next;
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::application::editor::EditorPlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(EditorPlugin);
        app
    }

    #[test]
    fn cursor_interaction_starts_idle() {
        let mut app = test_app();
        app.update();

        assert_eq!(
            app.world().resource::<CursorInteraction>().phase(),
            CursorInteractionPhase::Idle
        );
    }
}
