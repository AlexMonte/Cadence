# The Trilogy

Three crates, one pipeline. Each owns exactly one concern and knows nothing about the internals of the others.

| Crate   | Role               | Owns                                                 | Speaks                                        |
| ------- | ------------------ | ---------------------------------------------------- | --------------------------------------------- |
| Tessera | Authoring language | The spatial AST — tiles, stacking, chaining, flow    | Lowers to `Score` / `ControlScore`            |
| Cadence | Runtime kernel     | Exact time, voices, projection, scheduling, audio    | Consumes `Score`, emits playable musical data |
| Musaic  | Host application   | The product shell, the lowering boundary, the device | Owns Tessera↔Cadence, drives Bevy             |

The contract between them is one-directional and narrow: structure flows down, sound flows out. Tessera never knows how a note becomes audio. Cadence never knows a tile existed. Musaic is the only crate that holds both at once.



   Tessera                 Cadence                  Musaic
 (compose)               (evaluate)               (host)
┌─────┐   Score       ┌──────┐  audio       ┌────┐
  │     tiles         │─────►│    projection    │────►  │  device    │
  │     layout      │  controls      │    scheduling   │   frames     │  app UI    │
└─────┘                    └──────┘                 └────┘
      ▲                                                                                           │
      └─────── user edits ────────────┘

---

# Cadence

## The Problem

Tessera produces structure — a spatial arrangement that resolves into patterns. But a pattern is not sound. Between "this is the music" and "this is what you hear" sits an enormous amount of work: turning recursive, transformed, possibly-infinite musical material into exact, sample-accurate events that a speaker can play, at any point on the timeline, without drift.

That work has hard constraints:

- Time must be exact. Floating-point time accumulates error. Loop a pattern a thousand times and the thousandth cycle lands a little late. For music, "a little late" is wrong.
- Material is unbounded. A voice repeats forever. A pattern alternates every cycle. You cannot render "the whole song" because there is no end — only the next window.
- Transforms compose. Slow this, shift that, reflect the whole thing, then play it on every fourth cycle. The engine has to apply a tree of transforms and still land on exact moments.
- Playback is live. The transport scrubs, jumps, loops. Whatever the engine computes for cycle 12 must be identical whether you played from cycle 0 or dropped the needle straight onto 12.

Cadence is the kernel that solves this. It takes typed musical meaning and turns it into playable data — and it does so without ever knowing how that meaning was authored.

---

## The Insight: Source vs. Projection

The breakthrough is refusing to conflate two things that look like one.

There is what exists — the complete, recursive, infinite description of the music. And there is what is audible right now — a concrete, finite list of events inside a time window.

Cadence keeps these strictly apart.

- A `Score` is the source: a lazy, exact, recursive tree. It describes everything that could ever play, including material that repeats forever. You never "run" a `Score`. You query it.
- A projection is the answer to one query: "given this window of transport time, what moments are audible, where, and with what controls?" It is finite, concrete, and clipped to the window.

This is the same query-over-a-span seam Tessera leans on — the `Pattern` trait — but applied to the whole engine:

pub trait Pattern {

type Event;

fn query(&self, span: Span<Transport>) -> Vec<(Span<Transport>, Self::Event)>;

}

Everything downstream is a consequence. Scrubbing is just a query at a different window. Looping is a query whose window wraps. There is no global playhead mutating state — there is a pure function from `(Score, window)` to `events`. Determinism is free.

This one separation — source you query, projection you play — is to Cadence what stacking is to Tessera.

---

## How It Works

### Exact Time

The foundation is `Time` — a normalized rational number (`domain::rational`), not a float. One cycle is `Time::ONE`. A half-cycle is `Time::new(1, 2)`, automatically reduced to lowest terms. Addition, scaling, and subdivision stay exact forever.

let beat = Time::new(2, 4);

assert_eq!(beat, Time::new(1, 2)); // reduced, exact, no drift

Because time is exact, a pattern looped ten thousand times lands on the same rational boundary every cycle. There is no accumulated error to correct.

### Voices and Tiles

The leaf of the source tree is a `Voice` — a repeating period of musical material. Inside a voice, a `Tile` is one phase-local intent: a span within the period carrying a typed `Intent` (play this sample, run this synth, set this level). A voice declares how it repeats:

pub enum Repeat {

Forever, // loops indefinitely

Once, // a single period

Count(u32), // a fixed number of periods

Until(Time), // until the transport reaches a time

}

A `Tile` here is the runtime echo of a Tessera tile — but stripped of layout, reduced to "this intent occupies this phase span."

### The Score Tree

A `Score` wraps a recursive node. Its kinds are the engine's whole vocabulary of structure and transformation:

|Kind|Meaning|
|---|---|
|`Voice`|A repeating source leaf|
|`Mosaic`|Already-projected transport-time material|
|`Merge`|Children sound simultaneously|
|`CycleRoute`|Route successive cycles across children|
|`CycleSlots`|Assign specific cycle slots to children|
|`TimeScale { rate }`|Scale child time (slow / fast)|
|`Shift { offset }`|Move child material in transport time|
|`ReflectCycle`|Mirror child material inside a cycle|
|`Degrade`|Deterministically keep/drop event lifecycles|
|`WeightedChoice`|Per-cycle probabilistic selection|

This is the lowering target for Tessera. A subdivide container becomes a routed/scaled arrangement of voices; an alternate becomes a `CycleRoute`; a parallel becomes a `Merge`. Tessera's spatial AST collapses into this typed tree — and Cadence never needs to know it came from tiles.

A parallel `ControlScore` tree carries modulation (gain, pan, cutoff…) with the same set of transforms, so controls bend through `slow`/`shift`/`reflect` exactly like sources do.

### Projection

Querying a `Score` over a window produces `ProjectedMoment`s collected into a `ProjectedMosaic` (`domain::projection`). Projection is the canonical truth layer: it keeps clipped visibility (an event partly outside the window is marked, not dropped) and carries the projected control values that apply at each moment. This is "what is audible," fully resolved, for exactly one window.

### Signals

Not all modulation is discrete. A `Signal` (`domain::signal`) is a small, serializable descriptor evaluated as a pure function of cycle time:

value(t) = bias + depth * shape(rate * t + phase)

Shapes are `Sine`, `Saw`, `Tri`, `Square`, plus seeded `Perlin` and `Rand` noise. Signals carry only `f64`/`u64` so the `Score` stays `Clone + PartialEq + Send + Sync`. Attached via `with_signal`, a signal is carried unchanged through projection and evaluated per audio frame on the audio thread — so it modulates the live voice continuously rather than being sampled once.

### Scheduling and Playback

Above projection sits the runtime:

- `RendererCore` / `CadenceCompiler` turn a `Score` + window into a `PreviewReport` — `projected` (start-voice moments, timeline-friendly) and `evaluated` (starts plus `UpdateVoiceControls`, what the scheduler actually consumes).
- The scheduler walks forward in windows, promotes the first control update of an unseen voice into a backfill `StartVoice` so scrubbing stays correct, and feeds a performer.
- `PlaybackRuntime` is the score-first entry point: `play_score`, `replace_score`, `tick`, transport commands. The host owns the audio device.

### The Layered Kernel

Cadence is organized as clean architecture, dependencies pointing inward:

infrastructure ← supported host-facing API (the stable seam)

application ← projection, scheduling, audio lowering

adapter ← concrete integrations (sample playback)

domain ← exact time, voices, scores, projection, signals

`domain` knows nothing of audio. `infrastructure` is the only surface a host should touch. Authoring, ASTs, and notation live above this crate and lower into `Score` — the kernel stays string-free.

---

## The Complete Flow

1. Lowering Host lowers a typed source tree into Score / ControlScore

↓

2. Query Engine queries the score over a transport-time window

↓

3. Projection Window resolves into ProjectedMoments + projected controls

↓

4. Evaluation Moments lower to StartVoice / UpdateVoiceControls events

↓

5. Scheduling Scheduler advances windows, backfills, drives the performer

↓

6. Audio Voices render frames; signals modulate per-frame; sound out

---

## Design Principles

Source and projection are different things. A `Score` describes what could play, forever and exactly. A projection is the finite answer for one window. Never confuse the infinite description with the audible result.

Time is exact. Rational, normalized, drift-free. Floats appear only at the audio boundary, never in musical time.

Everything is a query. Scrubbing, looping, preview, scheduling — all are queries over a span. The same input window always yields the same events.

Transforms compose as a tree. Slow, shift, reflect, route, degrade — each is a `Score` node. Complexity is depth, not special cases.

The kernel is string-free. Cadence consumes typed meaning. Notation, syntax, and layout are someone else's problem — they lower into `Score` before they reach the engine.

Layers point inward. `domain` is pure; `infrastructure` is the contract. Hosts adapt to `HOST_API.md`, never to internals.

---

# Cadence in a nutshell

Cadence is the runtime kernel that turns typed musical structure into playable sound. It does not parse, and it does not draw — it consumes a recursive `Score` tree and produces sample-accurate audio.

Its core move is separating the source (a lazy, exact, possibly-infinite description) from the projection (a finite, concrete list of audible moments inside a queried window). Time is exact rational arithmetic, so nothing drifts. Every operation — preview, scrub, loop, schedule — is a pure query over a transport-time span.

A `Score` is built from voices and a vocabulary of composable transforms (merge, route, scale, shift, reflect, degrade). A parallel `ControlScore` and continuous `Signal`s carry modulation. Projection resolves a window into concrete moments; the scheduler advances those moments through a performer; `PlaybackRuntime` drives it all while the host owns the device.

Cadence consumes structure and emits sound. It never knows how the structure was authored.

---

# Musaic

## The Problem

Tessera is a language with no runtime. Cadence is a runtime with no language. Each is deliberately ignorant of the other: Tessera lowers to a typed `Score` and stops; Cadence consumes a `Score` and never asks where it came from. That ignorance is the point — it keeps both crates pure.

But a purely pure system plays no music. Something has to:

- Hold the canvas the user actually edits — the tiles, the chains, the flow.
- Perform the lowering — translate Tessera's spatial AST into Cadence's `Score` / `ControlScore` IR.
- Own the audio device, the transport, the clock, the window loop.
- Be the product — the window, the input, the file the user opens.

That crate is Musaic. It is the only place where Tessera and Cadence meet.

---

## The Insight: The Host Owns the Seam

The temptation is to let one crate reach into the other — to give Tessera a "play" button that calls Cadence, or give Cadence a notion of tiles. Both would couple two things that should never know each other.

Musaic's insight is to make the integration its own concern. Neither language nor kernel is allowed to import the other. Instead, Musaic depends on both and owns the single translation boundary between them:

use cadence::prelude::*;

use cadence::bevy_prelude::*;

use tessera::prelude::*;

use tessera::bevy::*;

The lowering — Tessera IR → `Score` — lives in Musaic and nowhere else. Tessera stays a pure authoring language. Cadence stays a pure kernel. Musaic is the seam, the shell, and the product all at once.

This is why the contract is so narrow: Musaic writes lowered scores into a shared resource, and Cadence's Bevy plugin drives playback from there. The handoff is data, not function calls reaching across crates.

---

## How It Works

### The Bevy App

Musaic is a Bevy application. It composes both crates' Bevy integrations:

- `CadencePlugin` (from `cadence::bevy`) registers the playback systems.
- Tessera's Bevy layer owns the canvas, the tiles, and user interaction.
- `PlaybackHandle` carries the `!Sync` `PlaybackRuntime`, installed as a non-send resource.

app.add_plugins(CadencePlugin);

app.insert_non_send_resource(PlaybackHandle::new(settings, audio_control));

### The Lowering Boundary

When the user edits tiles, Tessera resolves its spatial AST into typed musical meaning. Musaic lowers that meaning into `Score` and `ControlScore` and writes the result into the shared `ActiveScores` resource — a `BTreeMap<String, Score>` with revision tracking.

Tessera board ──lower──► Score / ControlScore ──write──► ActiveScores

### The Playback Loop

From there, Cadence takes over without Musaic micromanaging it:

- `replace_scores_system` notices the `ActiveScores` revision changed and merges the new scores into the running playback.
- `tick_playback_system` advances `PlaybackRuntime::tick` each frame while the transport is playing.

Musaic's job is to keep `ActiveScores` correct and let `CadencePlugin` do the rest.

### The Boundaries Musaic Respects

The host-API contract draws bright lines Musaic must not cross:

- Import from `cadence::prelude` / `cadence::bevy_prelude` — never `cadence::application::*`. Internals are off-limits; the infrastructure seam is the contract.
- The Tessera→`Score` lowering belongs to Musaic. Cadence must never gain tile-awareness; Tessera must never gain a runtime.
- `PlaybackRuntime` is `!Sync` — always installed with `insert_non_send_resource`.

---

## The Complete Flow

1. Author User arranges tiles on the Tessera canvas

↓

2. Resolve Tessera collapses the spatial AST into typed meaning

↓

3. Lower Musaic translates that meaning into Score / ControlScore

↓

4. Publish Musaic writes scores into ActiveScores (revision bumped)

↓

5. Sync CadencePlugin merges changed scores into playback

↓

6. Tick PlaybackRuntime advances; Cadence projects and plays

---

## Design Principles

The host owns the seam. Tessera and Cadence never import each other. Musaic is the single crate that knows both — and the only place the lowering lives.

The handoff is data. Musaic publishes lowered scores into a shared, revision-tracked resource. It does not orchestrate the kernel by hand.

Respect the contract. Depend on the infrastructure prelude, never internals. The lines in `HOST_API.md` are load-bearing.

Keep purity upstream. Every coupling concern — devices, transport, app lifecycle, translation — lives in Musaic so that the language and the kernel stay clean.

Musaic is the product. It is where the user lives: the window, the canvas, the sound. Tessera and Cadence are invisible from the user's seat.

---

# Musaic in a nutshell

Musaic is the host application that binds Tessera and Cadence into a playable instrument. Tessera is a language with no runtime; Cadence is a runtime with no language; Musaic is the only crate that holds both — and the single place their translation boundary lives.

Built on Bevy, Musaic composes Tessera's canvas layer with Cadence's playback plugin. When the user edits tiles, Musaic lowers Tessera's resolved structure into Cadence's `Score` / `ControlScore` IR and writes it into a shared, revision-tracked `ActiveScores` resource. Cadence's plugin notices the change, merges it into playback, and ticks the runtime forward each frame.

The coupling concerns — devices, transport, app shell, and the lowering itself — all live in Musaic, so the language and the kernel each stay pure. Musaic is the seam, and Musaic is the product.