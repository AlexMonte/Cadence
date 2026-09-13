//! Editor authoring session — modal statechart for arm / place / connect.
//!
//! **Single writer of [`EditorMode`]:** session systems in `MutateState` plus
//! the command dispatcher (`ArmPlacementTool`, `CancelPlacement`,
//! `StartConnection`, `AbortConnection`, post-accept settle). The cursor FSM
//! reads mode only.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use seldom_state::set::StateSet;
use tessera::prelude::NodeId;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::domain::board::BoardSurfaceId;
use crate::domain::document::{PlacementAddress, TileSpawnKind};

use crate::application::editor::{MusaicEditorSet, PickHit, transaction::PlacementTarget};
use crate::application::pipeline::scene_sync::VisibleBoardState;
use crate::domain::board::geometry::slot_at_world_position;

use super::cursor::BoardPlacementPointer;
use super::cursor::{
    BoardTilePressQueue, BoardTilePressed, DrawerTilePressQueue, DrawerTilePressed,
};
use super::cursor_logic::CursorInteractionPhase;
use super::cursor_logic::should_block_board_pick;

/// Hover target while placing from the drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementHover {
    pub surface: BoardSurfaceId,
    pub address: PlacementAddress,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacementSession {
    pub tile: TileSpawnKind,
    pub source: Option<NodeId>,
    pub hover: Option<PlacementHover>,
    pub last_hover: Option<PlacementHover>,
}

/// Mutually exclusive authoring modes. Illegal overlaps (armed+placing,
/// connect-tool without draft) are unrepresentable.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum EditorMode {
    #[default]
    Idle,
    Armed {
        tile: TileSpawnKind,
    },
    Placing(PlacementSession),
    Connecting {
        source: NodeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransitionError {
    /// Requested transition is illegal from the current mode.
    Illegal {
        from: &'static str,
        action: &'static str,
    },
}

impl SessionTransitionError {
    fn illegal(from: &'static str, action: &'static str) -> Self {
        Self::Illegal { from, action }
    }
}

impl std::fmt::Display for SessionTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Illegal { from, action } => {
                write!(f, "illegal session transition: {action} from {from}")
            }
        }
    }
}

/// Authoring context separate from pointer/cursor semantics.
#[derive(Resource, Default, Debug, Clone)]
pub struct EditorSession {
    pub mode: EditorMode,
    /// Pre-mode latch while a drawer tile is pressed (not an authoring mode).
    pending_drawer_press: Option<DrawerTilePressed>,
    pending_board_press: Option<BoardTilePressed>,
    /// Address of the most recently committed drawer placement (camera settle).
    pub last_placement_address: Option<PlacementAddress>,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSessionChanged;

impl EditorSession {
    pub fn mode_name(mode: &EditorMode) -> &'static str {
        match mode {
            EditorMode::Idle => "Idle",
            EditorMode::Armed { .. } => "Armed",
            EditorMode::Placing(_) => "Placing",
            EditorMode::Connecting { .. } => "Connecting",
        }
    }

    pub fn is_placing_from_drawer(&self) -> bool {
        matches!(self.mode, EditorMode::Placing(_))
    }

    pub fn is_armed(&self) -> bool {
        matches!(self.mode, EditorMode::Armed { .. })
    }

    pub fn is_connecting(&self) -> bool {
        matches!(self.mode, EditorMode::Connecting { .. })
    }

    pub fn blocks_board_picks(&self) -> bool {
        self.is_placing_from_drawer()
    }

    pub fn armed_tile(&self) -> Option<&TileSpawnKind> {
        match &self.mode {
            EditorMode::Armed { tile } => Some(tile),
            _ => None,
        }
    }

    pub fn connection_source(&self) -> Option<&NodeId> {
        match &self.mode {
            EditorMode::Connecting { source } => Some(source),
            _ => None,
        }
    }

    pub fn placement_tile(&self) -> Option<&TileSpawnKind> {
        match &self.mode {
            EditorMode::Placing(p) => Some(&p.tile),
            _ => None,
        }
    }

    pub fn placement_hover(&self) -> Option<PlacementHover> {
        match &self.mode {
            EditorMode::Placing(p) => p.hover.or(p.last_hover),
            _ => None,
        }
    }

    pub fn placement_mut(&mut self) -> Option<&mut PlacementSession> {
        match &mut self.mode {
            EditorMode::Placing(p) => Some(p),
            _ => None,
        }
    }

    /// Idle/Armed → Armed (replaces tile). Illegal from Placing/Connecting.
    pub fn arm(&mut self, tile: TileSpawnKind) -> Result<(), SessionTransitionError> {
        match &self.mode {
            EditorMode::Idle | EditorMode::Armed { .. } => {
                self.mode = EditorMode::Armed { tile };
                Ok(())
            }
            other => Err(SessionTransitionError::illegal(
                Self::mode_name(other),
                "arm",
            )),
        }
    }

    /// Idle/Armed → Placing. Clears pending press. Illegal from Placing/Connecting.
    pub fn begin_placing(&mut self, tile: TileSpawnKind) -> Result<(), SessionTransitionError> {
        match &self.mode {
            EditorMode::Idle | EditorMode::Armed { .. } => {
                self.pending_drawer_press = None;
                self.mode = EditorMode::Placing(PlacementSession {
                    tile,
                    source: None,
                    hover: None,
                    last_hover: None,
                });
                Ok(())
            }
            other => Err(SessionTransitionError::illegal(
                Self::mode_name(other),
                "begin_placing",
            )),
        }
    }

    /// Any mode → Idle. Clears pending press. Always succeeds.
    pub fn cancel(&mut self) -> Result<(), SessionTransitionError> {
        self.pending_drawer_press = None;
        self.pending_board_press = None;
        self.mode = EditorMode::Idle;
        Ok(())
    }

    /// Idle/Armed/Connecting → Connecting. Illegal from Placing.
    pub fn start_connection(&mut self, source: NodeId) -> Result<(), SessionTransitionError> {
        match &self.mode {
            EditorMode::Idle | EditorMode::Armed { .. } | EditorMode::Connecting { .. } => {
                self.pending_drawer_press = None;
                self.mode = EditorMode::Connecting { source };
                Ok(())
            }
            other => Err(SessionTransitionError::illegal(
                Self::mode_name(other),
                "start_connection",
            )),
        }
    }

    /// Connecting → Idle. No-op Ok from other modes.
    pub fn abort_connection(&mut self) -> Result<(), SessionTransitionError> {
        if matches!(self.mode, EditorMode::Connecting { .. }) {
            self.mode = EditorMode::Idle;
        }
        Ok(())
    }

    /// Take the placing payload for commit and return to Idle.
    pub fn take_placing_for_commit(&mut self) -> Option<PlacementSession> {
        match std::mem::replace(&mut self.mode, EditorMode::Idle) {
            EditorMode::Placing(placement) => Some(placement),
            other => {
                self.mode = other;
                None
            }
        }
    }

    /// After an accepted PlaceTile: leave Armed (click-to-place) for Idle.
    pub fn settle_after_place(&mut self) {
        if matches!(self.mode, EditorMode::Armed { .. }) {
            self.mode = EditorMode::Idle;
        }
    }

    /// After an accepted ConnectTiles: leave Connecting for Idle.
    pub fn settle_after_connect(&mut self) {
        let _ = self.abort_connection();
    }

    fn take_pending_drawer_press(&mut self) -> Option<DrawerTilePressed> {
        self.pending_drawer_press.take()
    }
}

pub fn blocks_board_picks_with_cursor(
    session: &EditorSession,
    cursor: CursorInteractionPhase,
) -> bool {
    should_block_board_pick(cursor, session)
}

const DRAWER_DRAG_THRESHOLD_PX: f32 = 10.0;

/// Registers the authoring session. Called by the editor plugin.
pub fn register_editor_session(app: &mut App) {
    app.init_resource::<EditorSession>()
        .add_message::<EditorSessionChanged>()
        .add_systems(
            Update,
            (
                process_drawer_press_queue,
                tick_editor_session_drawer_flow,
                sync_placement_hover_from_pointer,
                commit_placement_on_release,
            )
                .chain()
                .before(StateSet::Transition)
                .in_set(MusaicEditorSet::MutateState),
        );
}

fn process_drawer_press_queue(
    mut queue: ResMut<DrawerTilePressQueue>,
    mut board_queue: ResMut<BoardTilePressQueue>,
    mut session: ResMut<EditorSession>,
    mut changed: MessageWriter<EditorSessionChanged>,
) {
    if let Some(press) = board_queue.pending.take() {
        session.pending_board_press = Some(press);
    }
    if let Some(press) = queue.pending.take() {
        session.pending_drawer_press = Some(press);
        changed.write(EditorSessionChanged);
    }
}

fn tick_editor_session_drawer_flow(
    mut session: ResMut<EditorSession>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    project: Res<crate::application::session::MusaicProject>,
    mut commands: MessageWriter<EditorCommandBus>,
    mut changed: MessageWriter<EditorSessionChanged>,
) {
    let Some(mouse) = mouse else {
        return;
    };
    // A cancel and mouse release can arrive together. Clear the gesture before
    // release generates a placement command; a later queued cancel is too late.
    if keyboard.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)) {
        let _ = session.cancel();
        changed.write(EditorSessionChanged);
        return;
    }
    if windows.single().is_ok_and(|window| !window.focused) {
        let _ = session.cancel();
        return;
    }
    if let Some(pending) = session.pending_board_press.clone() {
        let moved = windows
            .single()
            .ok()
            .and_then(Window::cursor_position)
            .is_some_and(|cursor| {
                cursor.distance(pending.start_screen) >= DRAWER_DRAG_THRESHOLD_PX
            });
        if moved && matches!(session.mode, EditorMode::Idle) {
            session.pending_board_press = None;
            if let Some(node) = project.document.graph.node(&pending.node) {
                if let Ok(tile) = crate::application::command::editing::spawn_kind(&node.kind) {
                    if session.begin_placing(tile).is_ok() {
                        session.placement_mut().unwrap().source = Some(pending.node);
                        changed.write(EditorSessionChanged);
                    }
                }
            }
        } else if mouse.just_released(MouseButton::Left) {
            session.pending_board_press = None;
        }
    }
    if let Some(pending) = session.pending_drawer_press.as_ref() {
        if let Ok(window) = windows.single() {
            if let Some(cursor) = window.cursor_position() {
                if cursor.distance(pending.start_screen) >= DRAWER_DRAG_THRESHOLD_PX {
                    let tile = session.take_pending_drawer_press().unwrap().tile;
                    if let Err(error) = session.begin_placing(tile) {
                        bevy::log::warn!("{error}");
                    } else {
                        changed.write(EditorSessionChanged);
                    }
                    return;
                }
            }
        }

        if mouse.just_released(MouseButton::Left) {
            let tile = session.take_pending_drawer_press().unwrap().tile;
            commands.write(EditorCommandBus(EditorCommand::ArmPlacementTool { tile }));
            changed.write(EditorSessionChanged);
        }
    }
}

fn commit_placement_on_release(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut session: ResMut<EditorSession>,
    mut commands: MessageWriter<EditorCommandBus>,
    mut changed: MessageWriter<EditorSessionChanged>,
) {
    if session.is_placing_from_drawer()
        && mouse.is_some_and(|mouse| mouse.just_released(MouseButton::Left))
    {
        commit_drawer_placement(&mut session, &mut commands);
        changed.write(EditorSessionChanged);
    }
}

fn commit_drawer_placement(
    session: &mut EditorSession,
    commands: &mut MessageWriter<EditorCommandBus>,
) {
    let Some(placement) = session.take_placing_for_commit() else {
        return;
    };
    let tile = placement.tile.clone();
    let target = placement.hover.map(|hover| {
        session.last_placement_address = Some(hover.address);
        placement_hover_to_target(hover)
    });

    if let Some(target) = target {
        let command = if let Some(node) = placement.source {
            EditorCommand::EditTiles(crate::application::command::editing::TileEdit::Move {
                node,
                target,
            })
        } else {
            EditorCommand::PlaceTile { target, tile }
        };
        commands.write(EditorCommandBus(command));
    }
}

fn placement_hover_to_target(hover: PlacementHover) -> PlacementTarget {
    match hover.address {
        PlacementAddress::BoardSlot(slot) => PlacementTarget::BoardSlot {
            surface: hover.surface,
            slot,
        },
        PlacementAddress::StackIndex(index) => PlacementTarget::StackIndex {
            surface: hover.surface,
            index,
        },
    }
}

/// Resolve the placement hover slot from the sampled pointer.
///
/// Runs in `MusaicEditorSet::MutateState` (inside `MusaicSet::Commands`),
/// which is before `MusaicSet::SceneSync` in the same frame: the pointer
/// sample is current-frame, while `VisibleBoardState` is the previous frame's
/// projection. This is intentional — the projection only changes through
/// commands, which land next frame anyway, so hover is at most one frame
/// behind a document mutation and never behind the cursor.
fn sync_placement_hover_from_pointer(
    pointer: Res<BoardPlacementPointer>,
    visible: Option<Res<VisibleBoardState>>,
    project: Res<crate::application::session::MusaicProject>,
    mut session: ResMut<EditorSession>,
) {
    let Some(placement) = session.placement_mut() else {
        return;
    };

    let Some(visible) = visible else {
        return;
    };

    let Some(world) = pointer.cursor_world else {
        placement.hover = None;
        return;
    };

    let Some(slot) = slot_at_world_position(world, visible.layout) else {
        placement.hover = None;
        return;
    };
    let Some(surface) = visible.active_surface else {
        placement.hover = None;
        return;
    };

    let hover = match visible.pick_at(slot) {
        Some(PickHit::EmptySlot { .. }) => Some(PlacementHover {
            surface,
            address: PlacementAddress::BoardSlot(slot),
        }),
        Some(PickHit::StackInsert { index, .. }) => Some(PlacementHover {
            surface,
            address: PlacementAddress::StackIndex(index),
        }),
        Some(PickHit::Tile { node })
        | Some(PickHit::AtomCompound {
            primary_node: node, ..
        }) => {
            let own_source = placement.source.as_ref() == Some(&node);
            let numeric_note = super::super::transaction::drop::is_number(&placement.tile)
                && super::super::transaction::drop::note_owner(&project.document, &node).is_some();
            let stack_reorder = placement
                .source
                .as_ref()
                .and_then(|id| project.document.graph.location_of(id))
                .is_some_and(|loc| {
                    loc.surface == surface && matches!(loc.address, PlacementAddress::StackIndex(_))
                });
            if own_source || numeric_note || stack_reorder {
                project
                    .document
                    .graph
                    .location_of(&node)
                    .map(|loc| PlacementHover {
                        surface: loc.surface,
                        address: loc.address,
                    })
            } else {
                None
            }
        }
        _ => None,
    };
    placement.hover = hover;
    if hover.is_some() {
        placement.last_hover = hover;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::TileSpawnKind;

    fn output(name: &str) -> TileSpawnKind {
        TileSpawnKind::Output { name: name.into() }
    }

    #[test]
    fn pending_drawer_press_does_not_block_board_picks() {
        let session = EditorSession {
            pending_drawer_press: Some(DrawerTilePressed {
                tile: output("main"),
                start_screen: Vec2::ZERO,
            }),
            ..Default::default()
        };
        assert!(!session.blocks_board_picks());
    }

    #[test]
    fn active_placement_blocks_board_picks() {
        let mut session = EditorSession::default();
        assert!(!session.blocks_board_picks());
        session.begin_placing(output("main")).unwrap();
        assert!(session.blocks_board_picks());
    }

    #[test]
    fn begin_placing_from_armed_clears_armed_and_pending() {
        let mut session = EditorSession::default();
        session.arm(output("x")).unwrap();
        session.pending_drawer_press = Some(DrawerTilePressed {
            tile: output("y"),
            start_screen: Vec2::ZERO,
        });
        session.begin_placing(output("z")).unwrap();
        assert!(session.armed_tile().is_none());
        assert!(session.pending_drawer_press.is_none());
        assert!(session.is_placing_from_drawer());
    }

    #[test]
    fn arm_while_placing_is_illegal() {
        let mut session = EditorSession::default();
        session.begin_placing(output("a")).unwrap();
        assert!(session.arm(output("b")).is_err());
        assert!(session.is_placing_from_drawer());
    }

    #[test]
    fn start_connection_while_placing_is_illegal() {
        let mut session = EditorSession::default();
        session.begin_placing(output("a")).unwrap();
        assert!(session.start_connection(NodeId::new("n1")).is_err());
    }

    #[test]
    fn cancel_from_every_mode_returns_idle() {
        let mut session = EditorSession::default();
        session.cancel().unwrap();
        assert!(matches!(session.mode, EditorMode::Idle));

        session.arm(output("a")).unwrap();
        session.cancel().unwrap();
        assert!(matches!(session.mode, EditorMode::Idle));

        session.begin_placing(output("a")).unwrap();
        session.pending_drawer_press = Some(DrawerTilePressed {
            tile: output("p"),
            start_screen: Vec2::ZERO,
        });
        session.cancel().unwrap();
        assert!(matches!(session.mode, EditorMode::Idle));
        assert!(session.pending_drawer_press.is_none());

        session.start_connection(NodeId::new("src")).unwrap();
        session.cancel().unwrap();
        assert!(matches!(session.mode, EditorMode::Idle));
    }

    #[test]
    fn abort_connection_only_clears_connecting() {
        let mut session = EditorSession::default();
        session.arm(output("a")).unwrap();
        session.abort_connection().unwrap();
        assert!(session.is_armed());

        session.start_connection(NodeId::new("src")).unwrap();
        session.abort_connection().unwrap();
        assert!(matches!(session.mode, EditorMode::Idle));
    }

    #[test]
    fn start_connection_clears_armed() {
        let mut session = EditorSession::default();
        session.arm(output("a")).unwrap();
        session.start_connection(NodeId::new("src")).unwrap();
        assert!(session.armed_tile().is_none());
        assert_eq!(session.connection_source(), Some(&NodeId::new("src")));
    }
}
