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

let window = Span::new(Time::ZERO, Time::ONE).unwrap();
let preview = CadenceCompiler::new().preview(&score, &window)?;
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

### `Signal`: continuous modulation sources

A `Signal` is a small, serializable-friendly descriptor evaluated as a pure,
deterministic function of cycle time: `bias + depth * shape(rate * t + phase)`.
It carries only `f64`/`u64` parameters so `Score`/`ControlScore` keep their
`Clone + PartialEq + Debug + Send + Sync` invariants.

```rust
use cadence::prelude::*;

// Constructors: sine/saw/tri/square + seeded perlin/rand noise.
let lfo = Signal::sine().with_rate(2.0).with_depth(0.5).with_bias(0.5);
let value = lfo.eval (0.25); // pure, deterministic
```

Waveforms: `Sine`, `Saw`, `Tri`, `Square`, `Perlin { seed }` (smooth seeded
noise), `Rand { seed }` (sample-and-hold seeded noise). The noise shapes use
fast hash-based math — no allocation, no external state.

### `with_signal`: per-frame control modulation

`with_signal(score, key, signal)` attaches a continuous `Signal` to one
modulatable control lane. The signal is carried unchanged through projection
(attach-once; never ramp-sliced) and evaluated **per audio output frame** on the
audio thread, so it modulates the live voice rather than being sampled once.

```rust
use cadence::prelude::*;

let score = Score::from(cycle(vec![tile(Time::ZERO, Time::ONE, sample("pad"))]))
    .with_signal(ControlKey::Gain, Signal::sine().with_rate(4.0).with_bias(0.5).with_depth(0.5));
```

Modulatable lanes: `Gain`, `Pan`, `PlaybackRate`, and `LowPassCutoff`. During
lowering, signal-valued lanes become `SignalBinding`s on the `AudioVoicePlan`
(carrying the voice start cycle and cycles-per-second), which the audio voice
evaluates each frame as `start_cycle + (rendered_frames / sample_rate) * cps`.

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

- `preview(score, window)` → `PreviewReport { window, projected, evaluated }`
  - `projected` — start-voice moments only (timeline-friendly, backward compatible)
  - `evaluated` — full evaluation list (starts + `UpdateVoiceControls`), same as the scheduler
  - `PreviewReport::starts()` / `control_updates()` — filter `evaluated` by kind
  - `EvaluatedEvent::kind()` / `projected()` / `into_projected()` — inspect any evaluated row (also in `cadence::prelude`)
- `RendererCore::evaluate_window_full` — full evaluation without `CadenceCompiler`
- `projected_output` / `projected_mosaic` — starts-only direct access

### Control timing on start vs update

| Control key | Timing | Start voice | Scheduled update |
|-------------|--------|-------------|------------------|
| Gain, Pan, PlaybackRate, … | SegmentSampled | First segment snapshot | Later segment boundaries |
| PostGain, PitchBend, Expression, ModWheel, SustainPedal | ContinuousRuntime | Entry snapshot | Control-span boundaries |
| Attack, Decay, … | VoiceLifecycle / Onset | Applied at voice start | Not re-scheduled |

Segment-sampled changes use one `StartVoice` per lifecycle plus `UpdateVoiceControls` at later boundaries (no sample retrigger).

Per-window `evaluated` rows are stateless: a sub-window that begins mid-lifecycle may list `UpdateVoiceControls` only. The scheduler promotes the first update for an unseen `voice_id` to a backfill `StartVoice` so playback and scrubbing stay correct.

## Playback runtime

`PlaybackRuntime` (in `infrastructure::playback`) is the score-first runtime entry:

- `play_score`, `replace_score`, `tick`, transport commands
- Host owns audio device setup via `AudioRenderer::split`

## Bevy integration (`feature = "bevy"`)

```rust
use cadence::bevy_prelude::*;

app.add_plugins(CadencePlugin);
app.insert_non_send_resource(PlaybackHandle::new(settings, audio_control));
```

Resources:

- `ActiveScores` — lowered `BTreeMap<String, Score>` with revision tracking
- `PlaybackSync` — last applied score revision
- `PlaybackHandle::new` — constructs `PlaybackRuntime` for `insert_non_send_resource`

Systems (registered by `CadencePlugin`):

- `replace_scores_system` — merges `ActiveScores` into playback when revision changes
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
| `EventStream` | `Voice` / `Mosaic` | — | Leaf lowering only |
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

## Stable seams (unchanged)

Per-topic infrastructure modules remain the integration boundary:

- `infrastructure::{score, voice, projection, mosaic, input, audio, render, playback}`

`infrastructure::render::RendererCore` is a re-export of
`application::renderer_core::RendererCore` (one type, not a wrapper).
