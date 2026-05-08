# System Map

## Boundary Rule

Use this when a file feels ambiguous:

- Domain defines Tessera meaning and data.
- Application defines editor behavior, state transitions, and queries.
- Adapter translates between internal concepts and external technology.
- Infrastructure exposes the concrete user-facing surface.

Special rule:

- Framework binding belongs to adapter.
- User-facing Dioxus and canvas surfaces belong to infrastructure.

## Current Direction

This frontend crate is converging on four top-level zones:

- `src/domain`
- `src/application`
- `src/adapter`
- `src/infrastructure`

The current board implementation is still DOM-heavy and still leaks board behavior
through Dioxus-specific modules, but the target ownership is now explicit:

- `infrastructure::ui` owns the visible app shell and future canvas host.
- `adapter::dioxus` owns event and command bridging plus Dioxus-facing
  translation helpers.
- `adapter::canvas` will own draw-command mapping, rendering, hit-testing, and
  input normalization.
- `application::editor` owns editor state, commands, transitions, and view
  models.
- `domain` should stop importing adapter concerns as the next cleanup step.

## Immediate Violations To Remove In Follow-Up Passes

- Dioxus component trees still live under `adapter/dioxus/components`.
- The current board behavior is still implemented inside Dioxus-heavy modules
  instead of a canvas adapter plus infrastructure host split.
- `domain/editor.rs` still depends on adapter types.
- `adapter/dioxus/editor_service.rs` still owns too much orchestration and
  editor behavior.

Those are now explicit migration targets, not acceptable resting places.
