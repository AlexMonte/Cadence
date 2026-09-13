# Board placement and movement

Library drags and placed-tile drags share one preview and commit pipeline. The
source identity distinguishes creating a tile from moving an existing one.
Click-to-arm placement produces the same `PlaceTile` command through board picks.

```
native library press → DrawerTilePressQueue
placed board tile press → BoardTilePressQueue
    → drag threshold → EditorSession::begin_placing
    → BoardPlacementPointer sample
    → sync_placement_hover_from_pointer
    → preview from hover.or(last_hover)
    → release with a current valid hover
        library tile → PlaceTile
        existing tile → EditTiles(Move)
```

## Library behavior

The library is a categorized, searchable native Bevy UI scroll view. Each entry
has a compact symbol face and a full name on hover; adding entries increases scrollable
content instead of shrinking every tile. It uses `UiTilePalettePlugin` and
`DrawerTileSource`, without an offscreen palette camera or a separate 3D picking
scene.

A primary click arms one tile for the next valid placement click. Moving at least
10 screen pixels while pressing an entry starts drag placement. Category contents
follow the active root-board or container context. Numbers 0 through 9 are available in both contexts. Negative values and fractions
are entered by editing a number; there are no separate −1, half, Natural, or Octave
library tiles. Nested pattern containers are available inside patterns.

## Preview and commit invariants

- The board camera frames the surface and responds through `CameraRequest`; it
  does not follow the ghost.
- Slot snapping belongs to `sync_placement_hover_from_pointer` and
  `VisibleBoardState::pick_at`. The renderer never invents an authored address
  from the cursor.
- The ghost uses the same tile shape and transform path as a placed tile, with
  preview materials. Display addresses are mapped back to canonical board slots
  or stack indices before a command is emitted.
- The ghost may remain at `last_hover` when the cursor leaves the viewport. This
  is visual persistence only: release commits **only** `placement.hover`, never
  `last_hover`. Release outside the board or over an invalid target cancels the
  drag without a placement or move.
- Escape and loss of window focus cancel the session. A simple placed-tile click
  remains a pick; movement starts only after the threshold.
- Occupied root cells are not replaced by dragging. Placement validates the full
  footprint and rejects conflicts without changing the document.

## Contextual drops and owned groups

A Number dropped or click-placed onto a Note sets or replaces its octave. The
value must be a whole number from −1 through 9. The existing octave is changed
in place when present; otherwise the note receives an owned octave without
replacing the next note. Accidentals, elongation, speed, and other modifier/value
groups remain attached. Invalid fractions and out-of-range octave values leave
the document unchanged and produce a diagnostic.

Dragging a note face moves its complete owned expression. Dragging an owned
modifier keeps its operands together. In the same container, dropping on another
expression reorders the dragged group before that expression; empty cells use the
canonical insertion address. The move keeps existing identities rather than
creating replacements. These changes participate in the normal undo/redo history.
Every empty displayed cell is a usable drop target, including later rows; the
append marker is a convenience hint. Gaps retain their authored rest positions.
The moved face remains selected as one note with its normal inspector.

Root-board placement and movement also run the shared contextual connection
planner. It chooses compatible free endpoints, preserves exact named roles for
pairs that remain adjacent, and keeps occupied ports and unrelated wiring intact.
Saturation or incompatibility produces explanatory feedback; geometry that would
redirect an existing connection is rejected. See [CONNECTIONS.md](CONNECTIONS.md).

## Compact container presentation

A container stores one ordered `StackIndex` sequence. `StackDisplayMap` collapses
owned expression pieces onto their note face without changing node IDs or
authored addresses. It preserves gaps and maps append cells back to the correct
authored index. Root containers occupy 3 × 1 cells; containers inside another
container occupy 2 × 1 cells. Composed notes occupy one cell. Both cells of a
nested container select the same container, and a two-cell face wraps together
when it reaches the end of a row. Faces fill their cells without outside gutters.
The container symbol stands alone in the left header. Its authored tile count
appears as a small `≡N` badge in the lower-right corner, below the connection
target. Nested previews and composed notes share inset paper edges and the same
corner badge; inspector thumbnails use the same treatment.

Displayed container cells wrap across twelve columns. Shared board geometry
provides forward and inverse mapping for placed tiles, insertion targets, ghosts,
camera framing, and the minimap. A selected hidden modifier highlights its note
face; the inspector still exposes the modifier's own value group.

Timing preview visibility is independent of editing attention. Showing preview
keeps compose authoring available, and navigating into containers does not close
the timing panel. Playback outlines and connection packets use actual accepted
playback activity, independently of placement previews and selection.

## System ownership and order

1. `sample_board_placement_pointer` samples the pointer before session mutation.
2. The session chain consumes presses, detects drags, resolves hover, then commits
   releases inside `MusaicEditorSet::MutateState`.
3. Board reconciliation runs after SceneSync and applies face replacements.
4. Drag previews, slot highlights, and placement pulses run after reconciliation,
   so animation cannot target a despawned face during a note change.

Hover reads the previous frame's `VisibleBoardState` with a current-frame pointer
sample. Rendered previews read the current projection after scene synchronization.
`scene_reconcile` maintains tiles and connections by stable identity under
`Board3dRoot`; ghost previews, slot highlights, and `BoardGridAnchor` have separate
lifecycles. Playback overlays attach to those persistent tile/connection entities
and do not create a second picking or placement path.
