# Phase Tracker

## Phase 1: Boundary Freeze

- Added the system map and ADRs.

## Phase 2: Extract Application Editor Core

- Added `application/editor/board_view_model.rs` as a board-facing read model.

## Phase 3: Create Infrastructure Canvas Host

- The live board route now mounts `infrastructure/ui/editor_canvas_host.rs`.

## Phase 4: Create Canvas Adapter

- Added `adapter/canvas/{draw_command,renderer,hit_testing,input_mapper,theme_bridge,sprite_atlas,nine_slice}.rs`.

## Phase 5: Render the Board from View Model

- The canvas host now builds a board view model and canvas draw-command frame.

## Phase 6: Add Interaction

- The host now owns pointer entry and the input mapper file exists, but command
  dispatch is still partial and needs to replace the old Dioxus board path.

## Phase 7: Delete DOM-Based Editor Objects

- The active studio route no longer mounts `CompositionBoard`. The DOM board is
  now inactive code awaiting deletion.

## Phase 8: Add Pixel-Art Renderer

- Added sprite atlas and nine-slice adapter homes. Real sprite rendering is not
  implemented yet.

## Phase 9: Reevaluate Library Drawer

- The library drawer remains Dioxus-owned for now. No canvas migration has
  started there yet.
