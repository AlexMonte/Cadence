# Timed arrangements

`TimedScore::new(source, duration, repeats)` describes a compact source use.
`Score::arrange(segments)` loops those uses in order. Both constructors return
`Result<_, ArrangementError>`. The parallel `TimedControlScore` and
`ControlScore::arrange` APIs support control-only uses. `Timed<T>` is the
shared descriptor, exposing `source()`, `duration()` and `repeats()`.

Every occurrence queries source-local time `[0, duration)`. Time progresses at
its original speed, and resets to zero on every repeat, every new segment, and
every outer arrangement loop. The total period is the sum of
`duration * repeats`. Durations can be fractional.

For example, Alternate C/D used for three cycles and repeated twice plays
`C D C | C D C`. Slowing that source by two plays C over `[0,2)` and D over
`[2,3)`, then starts C again at cycle 3. A later direct query produces the same
events and identities as slicing a query that began at zero.

Whole note and lifecycle spans are clipped to the occurrence before clipping
visible spans to the requested window. Authored MomentIds remain unchanged for
tile provenance; internal scheduling identities additionally distinguish the
occurrence and source. Parallel source notes are preserved. Normal envelopes,
release tails and Legato still apply to the resulting note duration.

Attached controls and control-only arrangements use the same mapping. Ramps
are sliced when clipped, continuous signals restart their local phase, and
spatial trajectories use the occurrence's local clock. Runtime control updates
retain the same occurrence identity as their note start.

Construction allows 1–1024 segments, positive repeats, and positive durations
with denominators at most 1,000,000. Individual and total durations must be at
most 1,000,000 cycles and fit that denominator bound. Repetitions stay compact.
A direct seek skips preceding repeats instead of expanding them. Each
arrangement query caps occurrence traversal and emitted results at 16,384 and
accepts windows within ±1,000,000,000 cycles. Exceeding those limits returns
`ControlModelError::ArrangementQueryLimit`, including for control-only uses.
The existing audio preparation limits also reject excessive source density.

`cadence/tests/arrangement.rs` covers odd-period nested Alternate resets,
fractional duration, slow-note clipping, control-only uses, stable sliced
queries, authored provenance, runtime updates, rendered signal/noise repeats,
direct audio seeks and bounded dense queries.
