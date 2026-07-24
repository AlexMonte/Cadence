# Musaic module contracts

The registry of per-module architectural contracts. Every module section names
its **pattern**, its **single writer**, its **public API**, and its
**forbidden operations**. A change that violates a contract is wrong even if
it works; extend the owning module instead.

**Global rule: every resource has exactly one writer system. Everything else
submits requests (messages/commands) or reads.**

Sections are added as each module is migrated (see the module architecture
plan). Declared exceptions are listed explicitly — anything not listed is not
an exception.

---

## Frame schedule — `MusaicSet`

**Pattern:** Host-owned pipeline order. Kernel Bevy glue exposes nestable
sets; Musaic places them inside `MusaicSet` so compile/tick cannot race
bare `Update`.

| Stage | `MusaicSet` | Who runs |
|-------|-------------|----------|
| Input | `Input` | editor interaction |
| Commands | `Commands` | `dispatch_commands` (+ staging) |
| Document mutation | `DocumentMutation` | graph mutations after commands |
| Compile | `Compile` | document→`TesseraBoard` → nested `TesseraSystems` → collect IR |
| Lower | `Lower` | Pattern IR → Cadence `Score` / `ActiveScores` |
| Runtime | `Runtime` | nested `CadenceSet::{ReplaceScores, Tick}` + transport/preview/audio pump |
| Scene sync | `SceneSync` | `VisibleBoardState` + UI projection |
| Render UI | `RenderUi` | shell / inspector paint |

**Compile order (same frame):**
`sync_document_to_tessera_board` → `TesseraSystems` (authored sync →
`compile_on_request` → tile sync) → `collect_tessera_compile_output`.

**Runtime order (same frame):**
`CadenceSet::ReplaceScores` → `sync_transport_to_playback` →
`CadenceSet::Tick` → transport clock readback → preview dirty/project →
audio pump (after `Tick`).

Nesting is configured in `MusaicPlugin` after `TesseraPlugin` /
`PlaybackPlugin` (`CadencePlugin`) are added:
`TesseraSystems.in_set(Compile)`, both `CadenceSet` variants
`.in_set(Runtime)`.

**Forbidden:** scheduling Tessera compile or Cadence tick in bare `Update`
from Musaic host code; relying on accidental plugin registration order for
correctness.

---

## Board camera — `infrastructure/ui/camera_rig.rs`

**Pattern:** Camera rig (dolly-style): request queue → single arbitrator →
single smoother.

| Role | Owner |
|------|-------|
| Pose truth | `BoardCameraRig` resource |
| Only `BoardCameraRig` writer | `arbitrate_camera_rig` (`CameraRigSet::Arbitrate`) |
| Only camera `Transform`/`Projection` writer | `smooth_camera_transform` (`CameraRigSet::Smooth`) |
| Input vocabulary | `CameraRequest::{FrameSurface, FocusAddress, Pan, Orbit}` |

**Arbitration priority (per frame):** `FrameSurface` > `FocusAddress` >
`Pan`/`Orbit`. A pan arriving in the same frame as a deliberate jump is
dropped, never merged. Deliberate jumps reset `pan_offset`.

**Smoothing:** frame-rate-independent exponential damping
(`1 - exp(-rate * dt)`); rates differ between `Follow` and `UserControl`
modes. Never a fixed per-frame lerp.

**Producers (emit requests only):**

- `OnEnter(AppState::Editor)` — `CameraRequest::FrameSurface(OpenDocument)`
  (never poke `BoardCameraRig` directly).
- `board_camera_nav.rs` — pointer pan / orbit / edge scroll (screen deltas;
  the rig converts to plane units using its own yaw, never the smoothed
  transform).
- `camera_rig::emit_navigation_requests` — surface changes
  (`ActiveSurfaceChanged`), focus jumps (`EditorAttention` change), placement
  settles (drawer drag end).

**Declared read-only consumers of the smoothed transform:**
`sync_board_grid_anchor` (grid backdrop coverage),
`sample_board_placement_pointer` (cursor ray).

**Forbidden:** writing `BoardCameraRig` or the camera `Transform` anywhere
else; reading the smoothed `Transform` to derive *input* (feedback loop);
camera chasing the drag ghost (`BOARD_PLACEMENT.md` invariant).

---

## Command authority — `application/command/`

**Pattern:** CQRS write side — producers emit intents; one dispatcher mutates.

| Role | Owner |
|------|-------|
| User intent vocabulary | `EditorCommand` (public) |
| Undo vocabulary | `EditorInverse` (private; never emitted by UI) |
| Only mutation authority | `dispatch_commands` via `CommandContext` |
| History | `CommandHistory` + `HistoryEntry { forward, inverse, invalidation }` |

**Public API:** `EditorCommandBus(EditorCommand)`. Producers (keyboard, picks,
menu, inspector, drawer) write the bus only — they do not touch document,
attention, selection, session, tool, view settings, project open/save, or
project replacement.

**Execution split:**

- Durable / attention commands → `execute_command` → transactions
- Undo inverses → `execute_inverse` (history path only)
- Session/tool/panel/project/view/save arms → dispatcher early match (still the
  single writer; not free-form resource pokes from UI)

**Undo contract:** redo/undo applies the **stored**
`HistoryEntry.invalidation`, not a recomputed inverse result.

**Declared exceptions (UI-local chrome, not editor semantics):**

- Minimap / timeline edge-drag panel width (`shell/layout.rs`)
- Drawer override clear on surface change (`interaction/plugin.rs`)

**Forbidden:** mutating `MusaicProject` / `RuntimeState` / `ProjectSession` /
`EditorAttention` / `SelectionState` / `EditorSession` / `BoardViewSettings` /
`CommandHistory` from UI observers or menu systems; putting undo payloads on
`EditorCommand`; ignoring stored invalidation on undo/redo.

---

## Document vs pipeline resources

`MusaicProject` no longer bundles compile/runtime state. Each resource has a
clear writer role (no silent multi-writer on one bag of fields).

| Resource | Contents | Writer |
|----------|----------|--------|
| `MusaicProject` | `document` + `metadata` (incl. save dirty / file path) | `dispatch_commands` only |
| `ProjectSession` | recent paths, `last_saved_path` | `dispatch_commands` only |
| `RuntimeState` dirty flags | `ProjectDirty` (tessera/lower/scene/runtime/…) | **Set** by `dispatch_commands` via `apply_invalidation` / project replace; **cleared** by Compile / Lower / Runtime / SceneSync |
| `RuntimeState` compiled IR | `compiled: Option<CompiledProject>` | Compile + Lower only |

Pipeline stages (`MusaicSet::{Compile, Lower, Runtime, SceneSync}`) may
`ResMut<RuntimeState>` and may **read** `MusaicProject`. They must not
`ResMut<MusaicProject>`.

Save/open: menu and unsaved-dialog emit `EditorCommand::{SaveProject,
SaveProjectAs, OpenProject, …}` only. Persistence adapters perform I/O and
return paths; the dispatcher applies metadata/`ProjectSession` updates.

---

## Session statechart — `application/editor/interaction/session.rs`

**Pattern:** Modal statechart — mutually exclusive authoring modes with
`Result` transitions. Illegal overlaps are unrepresentable.

| Role | Owner |
|------|-------|
| Mode truth | `EditorSession.mode: EditorMode` |
| Mode writers | Session systems (`MutateState`) + command dispatcher arms |
| Transition API | `arm` / `begin_placing` / `cancel` / `start_connection` / `abort_connection` |

**Modes:** `Idle | Armed { tile } | Placing(PlacementSession) | Connecting { source }`

Connect tool is not a separate resource — `Connecting` *is* the connect tool
(`EditorToolState` deleted).

**Outside the enum (by design):**
- `pending_drawer_press` — pre-mode latch while a drawer tile is held
- `last_placement_address` — camera settle side-channel after commit

**Readers (never write mode):** cursor FSM, pick classify, keyboard Esc,
board preview, inspector palette highlight, camera edge-scroll gate.

**Attention validation** (`workspace/types.rs`): `EditorAttention::focus`
requires `DocumentQueries` and rejects missing nodes/surfaces and occupied
"empty" slots. `set_active_board_unchecked` is deleted; `enter_compose` stays.

**Forbidden:** setting `armed_tile` and `placement` independently; a separate
Connect tool flag; cursor writing session mode; blind `focus = …` in
production paths (tests may assign the field when testing readers only).

---

## Root-board connections

Connection legality, executable bindings, automatic placement wiring, and the
connect-mode preview are specified in [CONNECTIONS.md](CONNECTIONS.md).

---

## UI projection — `application/pipeline/ui_projection.rs`

**Pattern:** Single post-`SceneSync` read model + region dirty flags. One writer
computes identity-aware fingerprints; region systems consume their flag only.

| Role | Owner |
|------|-------|
| Read-model truth | `EditorUiProjection` |
| Region dirty flags | `UiDirty` |
| Only writer | `compute_ui_projection_system` (after `rebuild_visible_board_state`, in `MusaicSet::SceneSync`) |
| Enter-editor reset | `reset_ui_projection` in `ui_projection.rs` (OnEnter Editor; scheduled from `scene_sync`) |
| Pure diff | `diff_ui_regions(prev, next) → UiDirty` |
| Inspector layout | `derive_inspector_layout` called **only** inside the projection writer |

**Identity rules (no same-count staleness):**

| Region | Diff key |
|--------|----------|
| Timeline | `preview_event_ids` set (+ revision/surface) |
| Minimap | `board_occupancy_hash` + `connection_hash` + `focused_slot` |
| Inspector | full `InspectorLayout` + `selection_ids` + focus |
| Diagnostics | `diagnostics_summary` string — never `shell_structure` |
| Palette | context + armed tile + options content hash |
| Shell structure | surface / `layout_kind` / sprites-ready |
| Shell layout | panel open/dims chrome only |

Board 3D is **not** a `UiDirty` flag. `sync_board_3d_scene` owns keyed
`NodeId` / connection reconcile under a stable `Board3dRoot` (surface/layout/
asset-ready changes rebuild the root once).

**Consumers (`MusaicSet::RenderUi`):** shell rebuild, inspector transition,
minimap, timeline, diagnostics banner, palette sync. Each clears nothing
beyond its own work; they never recompute inspector layout.

**Declared exceptions (UI-local chrome writers that projection *reads*):**

- Minimap / timeline edge-drag panel dims (`shell/layout.rs` →
  `MinimapPanelState` / `TimelinePanelState`)

**Forbidden:** length-only fingerprints; `diagnostic_count` forcing shell
respawn; inventing inspector content from `armed_tile` alone; placement ghost /
slot highlight / grid anchor / camera reading `UiDirty`.
