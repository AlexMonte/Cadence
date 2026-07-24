use bevy::prelude::*;

pub mod atoms;
pub mod connection;
pub mod interaction;
pub mod layout;
pub mod panels;
pub mod selection;
pub mod transaction;
pub mod transport;
pub mod workspace;

pub use atoms::{
    Accidental, AtomCompoundSemantic, AtomCompoundView, AtomKind, AtomView, NoteName, OperatorKind,
    compound_atoms,
};
pub use connection::{
    ConnectionEndpointView, PortSlotState, connection_endpoint_view, cycle_port_state,
    port_state_for_side, set_port_state,
};
pub use interaction::{
    BoardCursorHover, BoardPick, BoardPlacementPointer, BoardViewportPointerInput,
    CursorInteraction, CursorInteractionChanged, CursorInteractionPhase, DrawerTilePressQueue,
    DrawerTilePressed, EditorMode, EditorSession, EditorSessionChanged, PickHit, PickModifiers,
    PlacementHover, PlacementSession, SessionTransitionError, apply_invalidation,
    blocks_board_picks_with_cursor, command_from_pick_checked, cursor_icon_for,
    push_transaction_diagnostics, should_block_board_pick,
};
pub use layout::{WorkspaceLayoutKind, layout_for_mode};
pub use panels::{
    DrawerPanelState, InspectorLayout, InspectorPanelKind, MinimapPanelState, TileDrawerItem,
    TileLibraryContextKind, TimelinePanelState, atom_tile_drawer_rows, basic_tile_options,
    derive_inspector_layout, inspector_panel_title, inspector_title, labeled_tile_drawer_items,
    placement_target_label, tile_inspect_description, tile_inspect_io_lines, tile_inspect_title,
};
pub use selection::{SelectionMode, SelectionState};
pub use transaction::{
    DiagnosticSeverity, EditorTransactionDiagnostic, EditorTransactionResult, Invalidation,
    PlacementTarget, TimelineSource, TimelineSourceResolver, TransactionStatus,
};
pub use workspace::{
    ActiveSpace, ActiveSurfaceChangeReason, ActiveSurfaceChanged, ActiveSurfaceError,
    EditorAttention, FocusError, FocusTarget, NavigationMode, WorkspaceMode,
};

/// The editor: canonical project state, input interpretation, command dispatch,
/// transport clock, and the visible-board projection.
///
/// One plugin per concept — the frame pipeline it participates in is the
/// [`MusaicSet`](crate::infrastructure::app::MusaicSet) chain:
/// Input → Commands → DocumentMutation → … → SceneSync.
pub struct EditorPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MusaicEditorSet {
    InterpretInput,
    MutateState,
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        use crate::infrastructure::app::{AppState, MusaicSet};
        use bevy::state::condition::in_state;

        // ── Canonical state the editor owns ──
        app.init_resource::<crate::application::session::MusaicProject>()
            .init_resource::<crate::application::pipeline::runtime::RuntimeState>()
            .insert_resource(crate::application::session::ProjectSession::restored())
            .init_resource::<crate::infrastructure::diagnostics::DiagnosticStore>()
            .init_resource::<transport::TransportClock>()
            .init_resource::<crate::application::board_view_settings::BoardViewSettings>()
            .insert_resource(EditorAttention::new(crate::domain::board::BoardSurfaceId(
                0,
            )))
            .init_resource::<SelectionState>()
            .init_resource::<DrawerPanelState>()
            .init_resource::<MinimapPanelState>()
            .init_resource::<TimelinePanelState>();

        // ── Staging inside MusaicSet::Commands ──
        app.configure_sets(
            Update,
            (
                MusaicEditorSet::InterpretInput,
                MusaicEditorSet::MutateState,
            )
                .chain()
                .in_set(MusaicSet::Commands),
        );

        // ── Input interpretation: cursor state machine, authoring session, picks, keyboard ──
        interaction::register_cursor_interaction(app);
        interaction::register_editor_session(app);
        interaction::register_input_interpretation(app);

        // ── Command dispatch: bus → history → document mutation ──
        crate::application::command::register_command_bus(app);

        // ── Transport clock (single owner; cadence overrides position while playing) ──
        app.add_systems(
            Update,
            transport::sync_clock_from_document
                .in_set(MusaicSet::DocumentMutation)
                .run_if(in_state(AppState::Editor)),
        );

        // ── Scene projection: document + attention + selection → visible board ──
        crate::application::pipeline::scene_sync::register_scene_sync(app);
    }
}
