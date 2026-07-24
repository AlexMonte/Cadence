# UI interaction channels

Exactly three input channels feed the editor. Infrastructure translates raw input
into these channels; the application layer owns all interpretation. Do not add a
parallel path (e.g. a UI widget mutating `EditorSession` or the document directly).

## 1. Shell commands

Menu chips, breadcrumb tabs, inspector buttons, and keyboard shortcuts emit
[`EditorCommand`](../src/application/command/mod.rs) values on the
`EditorCommandBus` message.

```
menu chip / breadcrumb / inspector button / keyboard (D, M, transport keys)
    → EditorCommandBus(EditorCommand)
    → dispatch_commands (MusaicSet::Commands, after MutateState)
    → execute_command → document / attention / selection / panels
```

Examples: `ToggleDrawer`, `TransportToggle`, `NavigateToSurface`, `PlaceTile`,
`StartConnection`, `JumpToTimelineSource`.

## 2. Drawer queue (tile library → placement)

The 3D palette is the sole interactive tile library — press and release, no
separate label-chip path:

```
on_drawer_tile_press (palette entry)
    → DrawerTilePressQueue
    → process_drawer_press_queue (MusaicEditorSet::MutateState)
    → tick_editor_session_drawer_flow:
        cursor moved past threshold → EditorSession::begin_placing (drag-to-place)
        released in place           → EditorCommand::ArmPlacementTool (click-to-arm)
    → release while placing → PlaceTile at the hovered slot
```

See [BOARD_PLACEMENT.md](BOARD_PLACEMENT.md) for the ghost preview pipeline that
runs while a placement session is active.

## 3. Board picks

3D board entities emit `BoardPickEvent` messages; the application classifies them:

```
tile / connection / stack insert / port glyph → BoardPickEvent::Hit { kind, .. }
board base:
    empty slot / stack insert under cursor → BoardPickEvent::Hit (Slot / StackInsert)
    anything else                          → BoardPickEvent::Miss
    (slot resolution goes through VisibleBoardState::pick_at — board_3d never
     invents pick rules)
    → handle_board_pick_events (MusaicEditorSet::InterpretInput)
    → classify_board_pick → EditorCommand (Focus, PlaceTile, EnterContainer,
      ConnectTiles, CycleConnection, …)
```

`BoardPickEvent::Miss` clears focus (`Focus { target: FocusTarget::None }`).
Picks are suppressed while a drawer placement drag is active
(`blocks_board_picks_with_cursor`).

## Frame order

`MusaicSet` chain per frame:
`Input → Commands (InterpretInput → MutateState → dispatch) → DocumentMutation →
Compile → Lower → Runtime → SceneSync → RenderUi`.

Inside `SceneSync`:
`rebuild_visible_board_state → compute_ui_projection_system → mark_scene_clean`.

`compute_ui_projection_system` writes `EditorUiProjection` + `UiDirty` (identity
diff vs previous frame). `RenderUi` region systems (shell, inspector, minimap,
timeline, diagnostics, palette) consume their dirty flag only — see
`docs/ARCHITECTURE.md` (UI projection).

Placement hover is resolved in `MutateState` against the previous frame's
`VisibleBoardState` (documented one-frame semantics; see BOARD_PLACEMENT.md).
Scene-reading systems (`board_3d` keyed sync, drag preview, slot highlight) run
after `SceneSync` and see the current frame's board projection. Placement /
camera paths never read `UiDirty`.
