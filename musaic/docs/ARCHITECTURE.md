# Musaic architecture

Musaic lets a person arrange musical tiles, hear the result, and save the
authored project. The implementation follows that experience in one direction:

```text
project file
    → MusaicDocument
    → Tessera program
    → PatternIr
    → Cadence PreparedScore
    → preview, playback, and WAV export
```

The document is the only editable source of truth. Everything after it is a
derived result that can be rebuilt.

## Crate responsibilities

| Crate | What the user gets | What the crate owns |
| --- | --- | --- |
| `tessera` | Tiles have precise musical meaning | The authored tile language, spatial resolution, validation, normalization, and `PatternIr` compilation |
| `cadence` | The music can be queried and played at exact times | Scores, exact transport time, bounded preparation, event evaluation, scheduling, voices, and audio rendering |
| `musaic` | A complete editor | The document, commands, persistence, Tessera-to-Cadence lowering, project assets, transport, and UI |

Tessera and Cadence do not depend on each other. Musaic is the only crate that
knows both.

## Authoritative state

### `domain/document`

`MusaicProject` is the persisted aggregate. It contains the canonical
`MusaicDocument`, project metadata, the reusable sound library, and
project-owned samples.

`MusaicDocument` contains the editable composition graph:

- the node graph and surface hierarchy;
- each node's location and authored tile data, including each Sound tile's
  complete sound definition;
- spatial endpoint bindings and explicit root relations;
- tricks, channel limits, and playback settings.

Document constructors and edits enforce ownership, placement, endpoint, and
reference rules. A document never needs a second board-shaped store to explain
what it means.

`export_document_program(&MusaicDocument)` is the only boundary from the
editor document to Tessera. It derives a complete `AuthoredTesseraProgram`
without changing the document.

### Application resources

| Resource | Purpose | Writer |
| --- | --- | --- |
| `MusaicProject` | Saved document, reusable sounds, and project assets | Command dispatcher |
| `ProjectSession` | File location, save state, and recovery session | Command dispatcher |
| `EditorSession` | Current pointer/keyboard interaction mode | Editor interaction systems |
| `RuntimeState` | Attempted, proposed, and accepted pipeline revisions | Compile, lower, and runtime stages |
| `VisibleBoardState` | Board read model | Scene-sync projection |
| `EditorUiProjection` | Inspector, timeline, palette, and shell read model | UI projection |

Each resource has one writer. Other modules read it or submit a command.

## Editing path

### `application/editor`

Editor interaction translates pointer and keyboard activity into intent. Its
session state represents one mutually exclusive mode: idle, armed, placing, or
connecting. Cursor and picking code may observe that mode but do not mutate the
document.

### `application/command`

`EditorCommand` is the write interface for durable project changes. The
dispatcher applies a command to a candidate document, verifies the affected
invariants, and commits the candidate atomically. A rejected edit leaves the
document and history unchanged.

Undo and redo store canonical document changes and their change impact. UI
systems never construct inverse commands.

### `application/history`

History groups one accepted user action into one entry. Replaying an entry
reuses its recorded composition, sound, presentation, or transport impact so
the necessary pipeline stages run again.

## Compile and playback path

Musaic orders all editor work with `MusaicSet`:

| Stage | Responsibility |
| --- | --- |
| `Input` | Read interaction and emit editor commands |
| `Commands` | Validate and commit document changes |
| `DocumentMutation` | Finish document-owned mutation work |
| `Compile` | Export the document and ask Tessera for `PatternIr` |
| `Lower` | Translate `PatternIr` into Cadence scores and prepare them |
| `Runtime` | Publish accepted scores, advance transport, and render audio |
| `SceneSync` | Derive board and UI read models |
| `RenderUi` | Reconcile Bevy entities with those read models |

### `application/pipeline`

Compile calls Tessera directly. Tessera resolves the authored spatial program,
validates its shape, normalizes it once, and returns `PatternIr`.

Lowering maps each `PatternIr` operation to the matching Cadence score
operation. Finite source events become `Score::events(Vec<Moment>)`.
Recursive structure stays recursive until Cadence evaluates a requested time
window.

Cadence prepares the complete lowered score once. A `PreparedScore` proves
that its structure and bounded workload are safe for the supported query
window. Preview, live playback, and export all consume that prepared source.

`RuntimeState` records exact revisions:

- attempted: the document revision most recently compiled;
- proposed: a valid prepared result waiting to become audible;
- accepted: the result currently driving playback and visual feedback.

If compilation or preparation fails, diagnostics describe the attempted
revision while the accepted revision continues playing.

## Persistence and assets

### `adapter/persistence`

Persistence reads one private project DTO with required fields and rejects
unknown fields. Construction validates the full document before publishing it.
An invalid file produces a normal project-read error and cannot partially
replace the open project.

Saving serializes the canonical document directly. Project audio is copied into
the adjacent `assets/samples` directory with stable sample identities. Atomic
writes and recovery checkpoints protect the current file without changing the
document model.

Preferences and exported keymaps each have one schema. Invalid stored
preferences reset to defaults with one warning. An invalid explicit keymap
import leaves the active keymap untouched.

### `adapter/audio`

The audio adapter resolves project-owned samples and connects prepared Cadence
scores to the platform audio device. It contains device and asset plumbing, not
musical authoring rules.

## Presentation

### `application/pipeline/scene_sync`

Scene sync projects `MusaicDocument` directly into `VisibleBoardState`.
Ports, cables, tile faces, nested surfaces, and focus addresses are derived from
the same committed document revision.

### `infrastructure/ui`

UI modules paint read models and emit commands:

- `board` reconciles keyed tile and cable entities;
- `controls` presents the contextual tile library;
- `inspector` edits the focused document concept;
- `shell` owns application chrome and dialogs;
- `timeline_view` displays evaluated Cadence events;
- `camera_rig` owns framing, pan, orbit, and smoothing;
- `widgets` provides shared interaction components.

Entity construction includes the transform and visibility components required
by Bevy. Keyed reconciliation creates, updates, and removes rendered entities;
it does not repair document state.

## Invariant boundaries

Checks run where invalid state could first enter:

- file shape and full-document integrity during import;
- placement, ownership, connection, and reference rules at command commit;
- Tessera language rules during compilation;
- Cadence workload bounds during score preparation;
- time-dependent source evaluation immediately before runtime publication.

Later stages trust those guarantees. They do not maintain parallel stores,
repeat structural validation, or scan the running world to repair authored
state.
