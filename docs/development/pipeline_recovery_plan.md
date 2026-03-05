# Pipeline Recovery Plan

## Scope
Recover end-to-end authoring flow so that:
- selecting nodes/inner nodes targets the expected code,
- apply actions mutate the active scope deterministically,
- palette actions produce visible graph/source changes,
- runtime playback reflects the current source.

## Section 1: UI Selection + Authoring Surface (`ui/app.js`)

### Goal
Selection and editor target must stay aligned after every mutation.

### Current actions
- [x] Rebind selection after mutations when `statement_id` changes using `resolveStatementIdAfterMutation`.
- [x] Preserve inner-node edit context when possible after apply.
- [x] Stop text preview from mutating authority mode (`preview_source` now runs in-place).

### Remaining work
- [ ] Add per-action toast/status that includes selected statement id before/after apply.
- [ ] Add a small selected-target badge (`statement` vs `stack lane` vs `inner span`) near node code editor.
- [ ] Add UI tests for:
  - update statement with id churn,
  - update inner lane and keep selection stable.

### Acceptance
- Editing and applying never leaves the editor bound to a non-existent statement id.

## Section 2: Palette Behavior (`ui/app.js`)

### Goal
Palette clicks always perform a visible, intentional mutation.

### Current actions
- [x] Top-level inserts now create a statement instead of replacing selected statement.
- [x] Chain inserts append to selected statement; if none selected, create a seeded statement.

### Remaining work
- [ ] Replace naive append with semantic transforms for common calls (`gain`, `struct`, `s`) to avoid duplicated chains.
- [ ] Add preview diff panel before apply for palette actions in code authority mode.
- [ ] Add tests for all categories (`source`, `compose`, `fx`, `display`) verifying statement/graph deltas.

### Acceptance
- Every palette click changes either source text or diagnostics with explicit feedback.

## Section 3: Command/Mode Boundary (`src-tauri/src/commands`, `src-tauri/src/sync/ops.rs`)

### Goal
Single reliable mutation pipeline across both authority modes.

### Current status
- `graph_scope_apply_ops` / `graph_scope_apply_source` mode-gated in backend.
- UI uses `applyStatementOpsViaCurrentMode` to route graph ops via:
  - direct apply in `graph_authority`,
  - preview->rendered source apply in `code_authority`.

### Remaining work
- [ ] Add command-level tests for UI routing assumptions:
  - code mode op preview->source apply equivalence,
  - graph mode direct op apply parity.
- [ ] Add explicit conflict code taxonomy (`mode_mismatch`, `missing_statement`, `invalid_stack_lane_update_raw`, etc.) in one contract enum/string table.

### Acceptance
- Same user action yields equivalent resulting source/IR regardless of current mode.

## Section 4: Semantic Pipeline (`src-tauri/src/sync/*`)

### Goal
IR/projected graph stays coherent across parse/render/reparse cycles.

### Current status
- Stable-id reconciliation and projection tests are present.
- Stack lane updates supported as graph op (`stack_lane_update_raw`) via normalization path.

### Remaining work
- [ ] Add mutation tests that combine:
  - statement update + move + delete in one op batch,
  - lane update on nested/complex `stack(...)` args.
- [ ] Add deterministic ordering assertions for `statement_changes` consumed by UI selection reconciliation.

### Acceptance
- Any accepted op batch round-trips render->reparse without topology/type regression.

## Section 5: Runtime + Strudel Playback (`ui/src/bridge/strudel.js`, runtime commands)

### Goal
Playback must match current source and surface runtime failures quickly.

### Remaining work
- [ ] Add runtime smoke tests for known problematic patterns:
  - built-in/sample symbols (`vox`, `rd`, `oh`) audibility check path,
  - notation like `~@2 2 <7 9 6 6>@2 2 <8 6 4 4>@2` ensuring full phrase evaluation.
- [ ] Add diagnostics channel from `evalProgram` failures to mini console with failing snippet context.
- [ ] Add sample library readiness indicator (loaded/failed counts) before first play.

### Acceptance
- Runtime commit + play on fixture corpus yields audible and complete patterns with no silent truncation.

## Section 6: Deadpoint Removal

### Goal
No stale pathways that suggest supported behaviors but are disconnected.

### Remaining work
- [ ] Remove or hard-disable obsolete helpers not used by current flow.
- [ ] Keep one authoritative flow map in docs and update with each command/DTO change.

### Acceptance
- Searching for action labels in UI maps to one backend mutation route each.

## Delivery Order
1. Selection/authoring hardening tests.
2. Palette semantic transforms + tests.
3. Command taxonomy + mode parity tests.
4. Runtime smoke/diagnostic improvements.
5. Deadpoint sweep and doc sync.
