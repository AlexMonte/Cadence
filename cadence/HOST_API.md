# `cadence` Host API

`cadence` is the runtime kernel for Musaic-style hosts. Its public shape mirrors
Tessera: domain types stay first-class, infrastructure adds helpers (not opaque
wrappers), and optional Bevy glue lives behind a feature flag.

## Quick start

```rust
use cadence::prelude::*;

let score = merge(vec![
    Score::from(cycle(vec![tile(Time::ZERO, Time::new(1, 2), sample("bd"))])),
]);
let prepared = PreparedScore::new(score)?;

let window = Span::new(Time::ZERO, Time::ONE).unwrap();
let preview = CadenceCompiler::new().preview(&prepared, &window)?;
```

## Module map (Tessera parallel)

| Tessera | cadence |
|---------|----------------|
| `tessera::prelude` | `cadence::prelude` |
| `TesseraCompiler` | `CadenceCompiler` |
| `Board` / `Flow` / `notes()` | Host-owned (Tessera). Lower to `Score` in Musaic. |
| `sample()` / `cycle()` / `merge()` | `infrastructure::stack` + `score_ext` |
| `tessera::bevy` | `cadence::bevy` (feature `bevy`) |

## Authoring helpers

Free functions (no wrapper types):

- `sample`, `synth`, `tile`, `cycle`, `voice`
- `merge`, `stack`, `slow`, `fast`, `shift`, `reflect`, `with_controls`, `with_signal`
- `ScoreExt` for chaining (`.slow(2)`, `.merge(other)`, …)
- `PatternExt` for higher-order combinators (`.every(4, f)`, `.jux(f)`, …)

Voice-level transforms remain in `domain::voice::ops` (`euclid`, `chain`, `overlay`, …).

`Repeat::Forever` is cyclic on both sides of transport zero. Notes and control
tracks use the same signed period phase, so negative queries and shifted patterns
retain earlier occurrences. `Once`, `Count`, and `Until` have an explicit beginning
at zero and do not repeat backwards; shifting one moves that beginning.

## Patterns and signals

`Pattern` is a uniform "query a transport-time window" seam (Strudel-style)
that complements the structural `Score` IR. It lives in the domain layer and is
re-exported from `cadence::prelude`.

```rust
pub trait Pattern {
    type Event;
    fn query(&self, span: Span<Transport>) -> Vec<(Span<Transport>, Self::Event)>;
}
```

- `impl Pattern for Signal` (`Event = f64`) samples once at the window midpoint.
- `impl Pattern for Score` (`Event = ProjectedMoment`) returns the visible
  events for the window — equivalent to `evaluate_score` over visible spans.

### `Signal`: musical-time sources

A `Signal` is a small descriptor evaluated as a pure, deterministic function of
cycle time: `bias + depth * shape(rate * t + phase)`. Rate and phase use exact
`Time` ratios; amplitude and bias use `f64`.

```rust
use cadence::prelude::*;

let lfo = Signal::sine().with_rate(Time::new(2, 1)).with_depth(0.5).with_bias(0.5);
let value = lfo.eval(0.25);
```

Waveforms include `Sine`, `Saw`, `Tri`, `Square`, `Ramp`, seeded `Perlin`
(smooth noise), seeded `Rand` (sample-and-hold noise), and seeded `Random`
(independent fractional-time noise). Noise evaluation
allocates nothing and depends only on the seed and musical time.

### `with_signal`: continuous controls and note intensity

`score.with_signal(key, signal)` attaches a signal control. Gain, PlaybackRate,
LowPassCutoff and Transpose use continuous signals during voice rendering.
Velocity samples its signal once at the original note onset, producing a fixed
level throughout the note. Velocity signal ranges must stay finite within 0–1.

```rust
use cadence::prelude::*;

let score = Score::from(cycle(vec![tile(Time::ZERO, Time::ONE, sample("pad"))]))
    .with_signal(ControlKey::Velocity,
        Signal::random(73).with_rate(Time::new(4, 1)).with_bias(0.85).with_depth(0.15));
```

Gain, sample-speed and cutoff signals become `SignalBinding`s on the voice plan
and are evaluated each output frame using its musical clock. Transpose retains
its additive semitone owner. Velocity is resolved during projection, before
outer timing transforms, and multiplies other velocity controls. Arrange,
reverse and speed changes carry that chosen intensity with each source note;
attach the signal after a transform to sample the transformed note onsets.
Seeking into a held note recovers its original intensity even after the original
control tile has ended. Playback and WAV export share these values.

For note-onset Random, `Signal::eval_at(Time)` hashes the normalized exact
fraction after rate and phase are applied, plus the seed. Sixteen distinct
onsets can therefore receive sixteen independent values even with rate 4.
Continuous controls use `Signal::eval(f64)`, which hashes the floating clock.
These deterministic values provide the same note-by-note variation behavior
without claiming another application's PRNG identity.

### `PatternExt` combinators

`PatternExt` adds Strudel/Tidal-style higher-order combinators on `Score`. Each
takes a structural transform `f: Fn(Score) -> Score` and lowers to the existing
`Score` IR (no new evaluator code):

| Combinator | Lowers to | Behavior |
|------------|-----------|----------|
| `every(n, f)` | `CycleRoute` (len `n`) | applies `f` on every `n`-th cycle |
| `whenmod(a, b, f)` | `CycleRoute` (len `a`) | applies `f` on cycles `b..a` of each group |
| `superimpose(f)` | `Merge` | layers `self` with `f(self)` |
| `jux(f)` | `Merge` | `self` panned hard-left, `f(self)` hard-right |
| `sometimes_by(p, f)` | `WeightedChoice` | per-cycle: `f(self)` with prob `p`, else `self` |

Note: `sometimes_by` is per-cycle; Strudel's `sometimesBy` is per-event. A
per-event variant is a planned follow-up.

## Projection and preview

`CadenceCompiler` wraps `RendererCore` with Tessera-like naming:

- `preview(score, window)` → `PreviewReport { window, events }`
  - `events` — the full evaluation list (starts + `UpdateVoiceControls`), same as the scheduler
  - `PreviewReport::starts()` / `control_updates()` — filter `events` by kind
  - `EvaluatedEvent::kind()` / `projected()` / `into_projected()` — inspect any evaluated row (also in `cadence::prelude`)
- `RendererCore::evaluate_window` — full evaluation without `CadenceCompiler`

### Control timing on start vs update

| Control key | Timing | Start voice | Scheduled update |
|-------------|--------|-------------|------------------|
| Gain, Pan, PlaybackRate, … | SegmentSampled | First segment snapshot | Later segment boundaries |
| PostGain, PitchBend, Expression, ModWheel, SustainPedal | ContinuousRuntime | Entry snapshot | Control-span boundaries |
| Attack, Decay, … | VoiceLifecycle / Onset | Applied at voice start | Not re-scheduled |

Segment-sampled changes use one `StartVoice` per lifecycle plus `UpdateVoiceControls` at later boundaries (no sample retrigger).

Per-window event rows are stateless: a sub-window that begins mid-lifecycle may list `UpdateVoiceControls` only. The scheduler promotes the first update for an unseen `voice_id` to a backfill `StartVoice` so playback and scrubbing stay correct.

## Playback runtime

`PlaybackRuntime` (in `infrastructure::playback`) is the prepared-score runtime entry:

- `PreparedScore::new` checks the structural and one-cycle workload budget once
- `play_prepared_score`, `replace_prepared_score`, `tick`, transport commands
- Host owns audio device setup via `AudioRenderer::split`

## Bevy integration (`feature = "bevy"`)

```rust
use cadence::bevy_prelude::*;

app.add_plugins(CadencePlugin);
app.insert_non_send_resource(PlaybackRuntime::new(settings, audio_control));
```

Resources:

- `ActiveScores` — one merged `PreparedScore`, output count, and revision
- `PlaybackSync` — last applied score revision
- `PlaybackRuntime::new` — constructs the non-send runtime inserted into the app

Systems (registered by `CadencePlugin`):

- `replace_scores_system` — publishes the prepared score when its revision changes
- `tick_playback_system` — calls `PlaybackRuntime::tick` while playing

`PlaybackRuntime` is `!Sync`; always install it with `App::insert_non_send_resource`.

## PatternNodeIr ↔ ScoreKind / ControlScoreKind parity

Musaic lowering in `musaic/src/application/pipeline/lowering/` should be a **homomorphism**:
one Tessera `PatternNodeIr` variant maps to one Cadence `ScoreKind` / `ControlScoreKind`
arm (recursive child mapping). No offset accumulators or shift+merge encodings for
structural variants that already exist in Tessera.

Structural ops that exist on **both** trees lower event children with `Score::*` and
control children with the matching `ControlScore::*` (child cardinality preserved so
cycle/concat alignment stays honest). Leaf and score-only transforms are noted below.

| PatternNodeIr | ScoreKind | ControlScoreKind | Notes |
|---------------|-----------|------------------|-------|
| `FlowProjection` | `QuerySource` | `QuerySource` | Immutable language query adapter; runs during planning, never the audio callback |
| `CycleEventStream` | `Events` under its structural cycle owner | — | Whole held spans survive clipped queries |
| `Sequence` | `WeightedCycleSlots` | `WeightedCycleSlots` | Weighted child allocation with preserved parent clocks |
| `Arrange` | `Arrange` | `Arrange` | Explicit occurrence duration and repeat count |
| `Merge` | `Merge` | `Merge` | Simultaneous children |
| `Concat` | `Concat` | `Concat` | Sequential children in transport-time order |
| `CycleRoute` | `CycleRoute` | `CycleRoute` | |
| `CycleSlots` | `CycleSlots` | `CycleSlots` | |
| `TimeScale` | `TimeScale` | `TimeScale` | |
| `Shift` | `Shift` | `Shift` | |
| `ReflectCycle` | `ReflectCycle` | `ReflectCycle` | |
| `PriorityMerge` | `PriorityMerge` | `PriorityMerge` | Control conflicts treat `ControlKey` as lane identity |
| `WeightedChoice` | `WeightedChoice` | `WeightedChoice` | Uses `WeightedScore` / `WeightedControlScore` |
| `MaskClip` | `MaskClip` | `MaskClip` | Clips **visible** spans to open `Gate` regions; never silently merge |
| `SpaceShift` | `SpaceShift` | — | Score-only; controls pass through |
| `SpaceScale` | `SpaceScale` | — | Score-only; controls pass through |
| `SpaceReflect` | `SpaceReflect` | — | Score-only; controls pass through |
| `Degrade` | `Degrade` | — | Score-only; control-only trees → unsupported diagnostic |
| `Deduplicate` | `Deduplicate` | — | Score-only; control-only trees → unsupported diagnostic |
| `EventStream` | `Voice` / `Events` | — | Leaf lowering only; **intentional discard on the control path** (event-only leaf — `lower_control_node` yields no controls, no diagnostic) |
| `ControlStream` | — | `Track` | Leaf lowering only |
| `ScalarStream` | — | gate `Track` | Leaf lowering only |

`WithControls` is Cadence-only composition (source + control tree). Tessera has no
direct counterpart; Musaic attaches controls when lowering paired event/control material
(`lowered_to_score`).

Board placement UI pipeline (Musaic editor): see
[`musaic/docs/BOARD_PLACEMENT.md`](../musaic/docs/BOARD_PLACEMENT.md).

## Musaic host convention

Musaic should import:

```rust
use cadence::prelude::*;
use cadence::bevy_prelude::*;
use tessera::prelude::*;
use tessera::bevy::*;
```

Tessera IR → `Score` lowering stays in the `musaic` crate. Write results into
`ActiveScores` and let `CadencePlugin` drive playback.

Avoid importing `cadence::application::*` from host code.

## Host-facing modules

Per-topic infrastructure modules remain the integration boundary:

- `infrastructure::{score, voice, projection, input, audio, render, playback}`

`infrastructure::render::RendererCore` is a re-export of
`application::renderer_core::RendererCore` (one type, not a wrapper).

## Immutable host query sources

`Score::query_source` and `ControlScore::query_source` accept immutable,
thread-safe `ScoreQuerySource` and `ControlQuerySource` implementations. Cadence
stays independent of the host language. Queries run while preparing/scheduling
scores; the callback receives prepared events and never invokes a host source.

`QueryMoment` carries a whole `Moment`, stable `instance_key`, and ordered typed
controls. Keys identify occurrences across overlapping windows and direct seeks;
distinct unison occurrences need distinct keys. Duplicate occurrence keys are
rejected. `Moment::value_identity` optionally supplies musical value identity for
value-based merge/deduplication policies without confusing two pitches that share
an instrument. Existing intent-based policies retain their meaning.

Sources declare conservative `estimated_work(window_cycles)` and finite sequence
`extent()`. Non-finite or oversized work is rejected before calling the source;
results are bounded and controls validated, including source-specific support.
Control sources declare owned lanes with `contains_key`; the default conservatively
owns all lanes so a gap releases a previous control value. The adapter must not
underestimate internal work merely because its final output is small.

Musaic's adapter preserves recursive Tessera flow policies and authored provenance.
Its estimate includes repeated held-note overlap, complete onset-cycle queries,
and mask work across whole notes. `cadence/tests/query_sources.rs` verifies stable
identity, later seeks, control gaps, invalid source rejection, callback independence,
and sequence extent. Musaic's flow conformance suite checks the language adapter.
