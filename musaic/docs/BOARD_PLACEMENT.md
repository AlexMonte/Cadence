# Board placement pipeline

Single contract for drawer → board ghost feedback. Do not add a third preview path in
`board_3d.rs`; extend `tile_visual` and `placement_preview` instead.

```
Drawer press (inspector/panels/drawer / tile_palette)
    → DrawerTilePressQueue
    → EditorSession::begin_placing (PlacementSession)
    → BoardPlacementPointer (sample_board_placement_pointer)
    → sync_placement_hover_from_pointer (slot address)
    → resolve_placement_preview_center (hover.or(last_hover) only)
    → tile_visual::spawn_board_tile (Preview mode)
    → sync_placement_slot_highlight (optional ring)
```

## Invariants

- **Camera** (`camera_rig::BoardCameraRig`) frames the board surface; it never reads ghost
  position. All camera motion goes through `CameraRequest` — see `docs/ARCHITECTURE.md`.
- **Ghost center** is session hover geometry only (`hover.or(last_hover)`). Cursor must not
  invent a slot inside `placement_preview`; slot snap lives in
  `sync_placement_hover_from_pointer` → `VisibleBoardState::pick_at`.
- **Ghost transform** uses the same `spawn_board_tile` / `board_tile_transform` path as
  placed tiles (`BoardTileMode::Preview` only swaps in `translucent_preview_material`).
- **Preview persistence**: while `EditorSession::is_placing_from_drawer()`, ghost stays at
  `last_hover` when the cursor leaves the board viewport briefly.
- **Inspector**: panel stack comes from `derive_inspector_layout` (application layer). Tile
  focus renders a full `TileInspectPanel` above the drawer when the user keeps the drawer
  open; there is no compact strip. Drawer "Armed: …" labels are allowed.

## System order (`Board3dPlugin`)

1. `sample_board_placement_pointer` — **before** `MusaicEditorSet::MutateState`
2. `sync_placement_hover_from_pointer` — in `MutateState` (inside `MusaicSet::Commands`)
3. `sync_board_3d_scene`, `sync_drag_preview`, `sync_placement_slot_highlight` —
   **after** `MusaicSet::SceneSync` (they read `VisibleBoardState`)

`sync_board_3d_scene` reconciles tiles/connections by `NodeId` under a stable
`Board3dRoot` (respawn only the tiles whose visual key changed). Placement
preview, slot highlight, and `BoardGridAnchor` stay on their own paths and are
not part of that keyed map.

Hover (step 2) reads the previous frame's `VisibleBoardState` by design: the pointer
sample is current-frame, and the projection only changes through commands that land
the next frame anyway. Preview systems (step 3) read the current frame's projection.

## Drawer visuals

The 3D tile palette (`UiTilePalettePlugin` + `spawn_board_tile_child` with
`BoardTileMode::Palette`) is the sole interactive tile library. On-tile glyphs
come from application catalog identity (`TileDrawerItem` / `basic_tile_options`);
there is no duplicate label-chip grid. Palette presses share one funnel:
`on_drawer_tile_press` / `on_drawer_tile_release` → `DrawerTilePressQueue` (drag
past threshold places; plain click arms). The viewport fills the drawer panel
and the ortho camera frames every catalog entry.
