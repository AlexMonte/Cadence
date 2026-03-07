# Cadence Tauri + Leptos Pivot (Discovery)

## Objective
Establish a clean Tauri + Leptos baseline with one proven backend↔UI command path before feature expansion.

## Current Root Layout
```text
cadence/
  src-tauri/
  ui/
  target/
```

## D1 Milestone Status
`D1: First End-to-End Command` is complete.

## D2 Milestone Status
`D2: Stateful Project IO Commands` is complete.

## D3 Milestone Status
`D3: Runtime Commit Loop` is complete.

## D4 Milestone Status
`D4: Production UX + Playback Correctness + Typed Nodes` is in progress (core integration complete).

## M7 Milestone Status
`M7: Production UX Core Completion` is in progress.

### Completed in M7 so far
1. Backend history stack added with undo/redo/status commands.
2. Menu wiring now includes `File -> Export Song` and real `Edit -> Undo/Redo` handlers.
3. Unsaved changes decision moved to frontend modal flow (`Save / Discard / Cancel`), with backend `dirty` status as source of truth.
4. Export command now writes real Strudel program output (or script override) to disk.
5. UI bridge stubs removed for `sync_apply_code`, `sync_preview`, and `export_song`.
6. Node DTO parity updated in the UI bridge for name mode, enabled slots, sequencer metadata, and port category.

### Completed checks
1. `src-tauri` runs through `tauri::Builder::run(...)`.
2. Backend command `project_create` has strict validation and deterministic response shape.
3. UI bridge `project_create` uses invoke-backed transport on wasm target.
4. Tauri window loads `ui/index.html` visual shell and can invoke `project_create`.
5. Setup verification script enforces integration wiring and compile checks.
6. `ui` crate compiles for both host and `wasm32-unknown-unknown` targets.
7. `project_open` and `project_save` are wired from the visual shell to backend commands.
8. Backend stores active project state and persists real project JSON.
9. Backend runtime commit commands generate deterministic Strudel program text from active scope.
10. UI play/stop buttons call runtime commit + Strudel host bridge (`initStrudel/evaluate/hush`).
11. Runtime commit accepts optional raw song script override so full Strudel mini-notation can run without node translation.
12. Native app menu is wired (File/Edit/View), with menu actions bridged into UI handlers.
13. Project new/open flow now enforces hard runtime stop/reset and unsaved changes prompt flow.
14. Main workspace no longer includes file path/create/open/save controls.
15. Typed nodes are active (`Voice`, `Const`, `Samples`) with deterministic runtime emission order.
16. Voice control deck derives from code via `node_ui_spec` and patches code via `node_apply_control_patch`.
17. Diagnostics moved to a hidden mini-console and separate dev inspector window scaffold.

## Verification Commands
```bash
cargo check -p src-tauri
cargo check -p ui
cargo check -p ui --target wasm32-unknown-unknown
cargo test -p src-tauri
cargo test -p ui
npm --prefix ui run build
```

## Next Milestone
D4 completion: replace `osascript` dialog fallback with cross-platform Tauri dialog plugin when dependency fetch is enabled.

## Phase 12.1 Status
`Phase 12.1: Import-First Architecture Lock` is in progress (backend contract slice complete).

### Implemented in this slice
1. Added statement provenance metadata to graph nodes/scopes (`source_origin`, `source_meta`).
2. Added import pipeline and coverage report generation for statement-first script import.
3. Added commands: `project_import_script`, `project_import_file`, `project_import_report`.
4. Added byte-stable-aware ordering contract in serializers for untouched imported statements.
5. Added UI import panel (paste/file import) and import report summary rendering.
6. Added generated coverage doc path contract at `docs/import_coverage.md`.
