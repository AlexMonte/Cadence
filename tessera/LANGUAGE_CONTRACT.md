# Tessera language contract

## Canonical authored groups

Tessera is a spatial language. The authored model makes ownership explicit:

- `AtomTile::Note(NoteAtom)` is a natural note letter. `Accidental(Sharp|Flat|Natural)`
  and `Octave(i64)` are typed pitch components of the same expression.
- `AtomTile::Modifier(AtomModifier)` is a complete group, such as `Fast(3)` or
  `Elongate(2)`. Its number stays bound when the group moves. Modifier evaluation
  follows authored group order; moving a group never changes operand ownership.
- `StackCompound::resolve` is the common resolver used by normalization. A stack
  with note C, octave 4, weight 2 and speed 3 keeps all three values through all
  six group permutations. A missing operand cannot borrow an octave or another
  complete modifier's value.
- `RootSurfaceNodeKind::Scalar(ScalarAtom)` is a numeric source tile with an `out`
  socket. `Board::at(x,y).scalar(value)` can place it above a transform and bind
  the output south to the transform's north-facing factor or amount input.

A note retains letter, accidental and octave. `NoteAtom::pitch_label` supplies
chromatic spelling (for example `c#`), and `semitone(default_octave)` supplies a
MIDI-style semitone coordinate. C#4 and Db4 both yield 61; B#4 yields 72 and Cb4
59. The compiled note keeps pitch spelling separate from host instrument choice.

## Recursive timing contract

Compilation preserves containers as a tree. An atom is a one-cycle repeating
leaf; `EventStream` remains explicitly finite material for host-authored IR.
The owned `Rev` modifier reverses note order and timing within each pattern
cycle, matching the connected `Rev` transform. It is separate from the
`Reverse(bool)` sampler setting, which changes recorded-sound playback.

`Arrangement` runs child patterns in order for their `Elongate` duration in cycles
(one cycle by default). Each section starts its child clock at zero; Fast/Slow
change that child's speed while its section keeps the authored duration. The
complete arrangement repeats after the sum of its section durations.

`Sequence` assigns children weighted shares of each parent cycle. Its child
query preserves the parent cycle index, so nested Alternate changes branches
on successive cycles. `Layer` merges child patterns. `Alternate` retains a
cycle-route operation until a bounded query is requested.

A modifier changes its child before the child is placed into its parent slot:

- `C * 3 D` produces C at 0, 1/6 and 1/3, then D at 1/2.
- `C @ 2 * 3 D` produces C at 0, 2/9 and 4/9, then D at 2/3.
- A nested Alternate(C,D) with `*2` queries two inner cycles, producing C then D
  within the allotted slot. It does not repeatedly reuse cycle zero.
- A lone `C / 2` has whole spans 0–2, 2–4, and so on. Within `C / 2 D`, the
  same slowed note covers its two first-half slots with a stable whole span
  0–3/2; D still begins at 1/2 and 3/2. Direct seeking must not change that
  note's whole span or voice identity. Slot visibility and whole sounding span
  are separate concepts.
- `@` distributes sequence time. A gate/sustain control changes sound duration
  independently and must not be implemented by changing the sequence weight.

Literal sound parameters rewrite event fields through the tree.
Patterned sound parameters preserve their recursive control trees. Fast
and Slow transform inputs currently require constant scalar values; an unsupported
patterned factor produces a diagnostic instead of silently using its first event.
Degrade remains a recursive policy. Musaic and Cadence preserve its portable
musical-value identity, whole span and seed; weighted choices use the same signed
cycle hash. Cross-crate property tests cover reordered builds, negative and
fractional windows, and later seeks. Language previews and runtime must choose
the same events rather than depend on process-local IDs.

`PatternNodeIr::query(CycleSpan)` and `TesseraCompiler::preview_authored_container` provide
bounded previews of the recursive core. `flatten()` materializes one finite
inspection stream; hosts should use bounded queries for multi-cycle previews.
Container nesting is rejected if cyclic or deeper than 128 levels.

## Serialization

Tile and root-node variants use adjacent `kind`/`value` records. Socket endpoints
use `{kind: "socket", port: ...}`.
Spatial endpoint maps write lists of endpoint/side pairs because structured
endpoints cannot be JSON object keys. The authored-board roundtrip contract exercises nodes, numeric inputs,
ports, positions, connections, compilation and an eight-cycle seek.

The public builders `octave`, `accidental`, and `modifier`, and the corresponding
`SequenceStack` methods, create typed components. `SequenceStack::fast`, `slow`,
and `elongate` write complete groups; `E @2 Octave(4)` keeps timing and pitch
ownership explicit.

## Sound parameters

`ParameterKey::ALL` is the editor/compiler catalog. Each `.spec()` declares the
unit, validation domain, default, suggested editor range, musical timing,
composition rule, source support and whether a patterned operand is supported.
Suggested slider bounds are not validation limits. The libraries remain
independent: Musaic maps `ParameterKey::control_key()` to Cadence's typed key and
checks its canonical name, timing, merge rule and source support against
`host_control_name` and the corresponding catalog fields.

`AtomModifier::parameter_key` and `parameter_value` expose a group's typed role
and operand. `with_parameter_value` validates an edit while retaining that role.
Musaic stores a sound modifier as one `AtomValue::Modifier` node; its owned
numeric accessor is separate from the free scalar-source accessor. Moving or
serializing that node cannot detach its value. Gate keeps a boolean operand and
sample-bank selection keeps a symbolic operand.

| Parameter | Value and default | Applied when | Meaning |
| --- | --- | --- | --- |
| Fast / Slow | Positive rate ratio; 1 | Pattern query | Changes pattern time; constant operand in this release. |
| Gain | Nonnegative linear gain; 1 | Sampled segments | Multiplies existing gain. |
| Velocity | 0–1; 1 | Onset | Sets note intensity before processing. |
| Expression | 0–1; 1 | Continuous runtime | Multiplies live voice volume. |
| Post-effects gain | Nonnegative linear gain; 1 | Continuous runtime | Multiplies level after voice effects. |
| Clip length | Nonnegative duration ratio; 1 | Onset | Scales the sounding gate without redistributing sequence slots; zero cuts it off. |
| Pitch bend | −1 to 1; 0 | Continuous runtime | Normalized bend, mapped by Cadence to ±2 semitones. |
| Attack / Decay / Release | Nonnegative seconds; 0 | Voice lifecycle | Sets the corresponding envelope stage without moving musical onsets. |
| Pan | −1 to 1; 0 | Continuous runtime | Adds a stereo position: −1 left, 0 center, 1 right. Independent of spatial attenuation. |
| Transpose | Signed semitones in −127–127; 0 | Continuous runtime | Adds an offset on the dedicated Transpose lane; the base note remains on Pitch. |
| Gate | Boolean, or scalar 0/1; open | Sampled segments | Opens/closes sound; does not redistribute sequence slots. |
| Legato / Note length | Positive duration ratio; 1 | Onset | Multiplies sounding gate length while leaving event onsets/spans unchanged. |
| Sustain level | 0–1; 1 | Voice lifecycle | Envelope level after decay; it is not a duration. |
| Low-pass cutoff | Positive Hz; 20,000 | Sampled segments | Sets filter cutoff; the host handles device-dependent effective limits. |
| Low-pass resonance | 0–1; 0 | Sampled segments | Normalized resonance level, not an unbounded Q value. |
| High-pass cutoff | Positive Hz; 20 | Sampled segments | Removes low frequencies below the cutoff. |
| High-pass resonance | 0–1; 0 | Sampled segments | Normalized resonance around the high-pass cutoff. |
| Sample bank | Nonempty bank identifier; explicit | Onset, samples only | Selects a bank on an already bound sample source. Does not change an instrument's source kind. |
| Sample variant | Nonnegative integer; 0 | Onset, samples only | Selects a bank variant. |

Gain, Velocity, Expression, PostGain, PitchBend, ClipLength, Attack, Decay,
Sustain, Release, Transpose, Pan, Gate, Legato, both filters'
cutoff/resonance and sample variant are available as flow transforms and owned
atom modifier variants;
SampleBank is an owned symbolic modifier until a symbolic pattern authoring
surface exists. Sample selection uses `SampleBank`/`SampleVariant` fields and
keys, never the generic `Select` lane. Except Fast/Slow and symbolic bank names,
the scalar sound catalog accepts patterned operands and validates every branch
before a revision is published. Sample rate, start/end, reverse, fit and loop
also have scalar flow transforms for patterned operands. An invalid value hidden in a later Alternate branch is
an error immediately.

Cadence validates the combined transpose range as well as each operand; adding
valid offsets can still exceed its −127–127 semitone limit. Its continuous
runtime and tile catalog accept literal/patterned offsets and complete modulation
values. Numeric transposition adds to modulation in either owned-group order. `PitchBend` is now a separate owned tile
and flow transform: its input is normalized −1–1 and the audio runtime maps it
to −2–2 semitones. It is not an alias for semitone-valued Transpose.

The authoring contract intentionally narrows two generic Cadence value domains:
sample variants must be whole numbers within `u32`, and sample-bank identifiers
must be nonempty. Cadence's control metadata exposes value shape, timing, merge
and source support rather than units or editor bounds. Musaic's conformance
tests check those shared metadata fields and exercise the actual lowering and
pitch-bend conversion, without treating slider ranges as engine limits.

`C Legato(3) Sustain(1/4) D` retains C's half-cycle span and D's onset at 1/2,
with a sounding gate ratio of three and sustain level of one quarter.
`C @3 D` instead moves D to 3/4. Patterned Legato, cutoff and variant controls
remain recursive and keep their Alternate choices through direct seeks.

Event fields use adjacent tags, so nested field values have one unambiguous
representation.


### Sample playback groups

`PlaybackRate(ratio)`, `PlaybackStart(position)`, `PlaybackEnd(position)`,
`Reverse(bool)`, `Fit(bool)` and `Loop(bool)` are sample-only owned modifiers.
Rate changes the source playback rate, not the pattern clock; it must be positive
and at most 65536. Start/end are absolute normalized sample positions in 0..1,
with the final effective region satisfying `start < end`. Fit changes source rate
to fill the authored note slot before explicit rate and pitch changes; Loop repeats
the selected source region until the note ends. Neither changes sequence weights.

`Slice { index, count }` is one tile with two owned operands. Its zero-based index
must be below count, and count is 1..65536. Its catalog value is
`FieldValue::Slice { index, count }`, never two unclaimed neighboring numbers.
A slice divides the note's effective source region into equal pieces. The host
first combines explicit per-note start/end with the assigned instrument's region,
then selects the slice. Reordering these different-role groups preserves the
result. Multiple slice selectors on one note are rejected. A slice does not have
a single scalar control key; hosts lower it to the event's source region.

Sample selection fields remain in the recursive pattern tree, including inside
Alternate. Hosts must resolve the selected source for each output before accepting
sample-only fields, and must diagnose jointly invalid regions without replacing
the last valid compiled score. Sequence speed (`Fast`/`Slow`), relative sequence
weight (`Elongate`), held length (`Legato`), and source rate (`PlaybackRate`) remain
distinct operations.


### Timed pattern occurrences

`PatternNodeIr::Arrange { segments }` retains recursive pattern sources. Each
`TimedPatternIr { duration, repeats, node }` allocates `duration` cycles per
occurrence. It queries the source at local time zero for each repeat, allows its
inner cycles to advance within that occurrence, and clips authored events at the
occurrence bounds. It does not stretch the source to fill the allocation. The
complete arrangement loops after the sum of all `duration × repeats` values.

For example, Alternate(C,D) with duration 3 and repeats 2 plays
`C D C | C D C`. A direct seek inside either occurrence produces the same pattern
choice, scoped fields, and clipped event span as a continuous query. Named pattern
compilation uses `TesseraCompiler::compile_normalized_container_ir`, which returns
the recursive tree without evaluating cycle zero.

Durations must be positive rationals and repeats must be nonzero. Hosts validate
individual segments with `TimedPatternIr::validate` and combined allocation with
`TimedPatternIr::total_duration` before publishing a revision. Malformed serialized
timing yields an empty bounded query instead of division by zero. Querying a late
repeat visits only occurrences intersecting the requested window; repeat counts
are not expanded into copies of the source tree.


## Optional authoring provenance

A host can populate `Container.source_nodes` with a map from authored stack token
indices to its tile IDs. Normalization carries each note's initial token identity
into `AtomExpr.source_node`, and compilation emits that identity in the event's
provenance. Grouping and reordered modifiers do not substitute normalized event
positions for authored IDs. This metadata is optional and does not change music.
Musaic supplies it from the document graph and keeps the mapping together with an
audio-accepted compilation, so preview navigation resolves the original tile.


### Complete effect groups

`Delay(DelayParameters)`, `Reverb(ReverbParameters)` and
`Compressor(CompressorParameters)` are owned note/container modifiers. Their
entire settings travel together through moving, editing, file serialization,
recursive compilation, host lowering and Cadence playback. Delay owns amount,
time, feedback and damping; reverb owns amount, decay and damping; compressor
owns threshold, ratio, knee width in dB, attack and release. All values are rational. Times are
seconds. Levels use 0–1; delay time is 0.001–2 seconds (the current runtime buffer
limit), reverb decay is 0.001–60 seconds, and compressor ratio is 1–20 with
attack/release 0.001–60 seconds. These bounded authoring ranges intentionally
narrow Cadence's more general constructors.

`FieldValue` and `ControlValueIr` carry the full typed structures, rather than
encoding a settings object as a scalar amount. These groups override the matching
instrument defaults. Whole groups can be attached to nested containers to affect
all child notes; alternating child containers can author different complete
settings.

The same complete effect tile can begin a typed control expression. In a container
of standalone effect values, successive tiles occupy successive time slots; rests
leave a gap. Delay, Reverb and Compressor flow transforms accept a matching typed
control container above their main note input. A held note therefore follows the
changing complete settings without retriggering. Every branch is type checked;
a Reverb value cannot enter Delay, and numbers cannot replace settings groups.
Fast/Slow timing and sequence/alternate structure remain recursive. Gaps restore
the instrument defaults (or bypass), while existing effect tails continue naturally.
Effect values retain their current note-owned meaning when placed after a note.
Scalar control patterns likewise allow rests without changing their scalar type.

### Continuous modulation tiles

`Modulation { parameter, value: ModulationParameters }` owns its target, waveform,
rate, phase, minimum, maximum and noise seed. Supported targets are Gain, Velocity,
PlaybackRate, LowPassCutoff and Transpose. Velocity samples its signal at each note onset, then holds that level. Waveforms are Cadence's actual sine,
saw, triangle, square, smooth seeded noise, stepped seeded noise, exact-onset Random and clamped
non-repeating ramp. Random hashes the normalized musical onset and seed, allowing
neighboring notes to vary independently without changing on later seeks. Periodic/noise outputs map -1..1 into the owned range; a ramp
maps 0..1 from minimum to maximum. The clock is musical cycles: rate scales it
and phase offsets it. Zero and negative rates retain Cadence's stopped/reversed
clock behavior. A ramp follows that absolute clock rather than restarting on
each note.

Velocity ranges remain within 0..1. Gain ranges stay nonnegative; sample rate and cutoff ranges stay strictly
positive; transposition remains within -127..127 semitones. Minimum cannot exceed
maximum. The same complete modulation tile can follow a note or lead a typed
control-pattern slot connected to its matching flow transform. Every branch is
validated before publication. Numeric gain multiplies a gain modulation in
either order; numeric transposition adds to transpose modulation. Multiple gain
signals in one lane are rejected because the current engine does not compose
their product. Changing modulation values while a note is held replaces the
binding without restarting the note. Returning to scalar controls or a gap
clears the old binding. PlaybackRate modulation requires a sample instrument.

### Available rhythm authoring

Musaic exposes complete Fast, Slow, Elongate, Replicate, Degrade, Euclid and
EuclidRot groups in its rhythm library. Euclid owns pulse count and step count;
EuclidRot owns those counts plus rotation. The inspector edits the group rather
than placing unrelated numbers beside it. Binary Choice (`C | E`) and Parallel
(`C , E`) tokens now survive document import/export and have visible library
entries. Choice alternates its branches across cycles; it is not a random draw.

ModWheel remains a live-input lane without a built-in voice mapping. SustainPedal
belongs to held live-note lifecycle handling. Neither is advertised as a working
scheduled note tile. Sample bank remains symbolic and Slice remains composite; neither claims
a scalar flow-transform operand.

## Compact pattern utility

`parse_mini_notation` returns a bounded recursive tree for sequences,
subdivision, layering, alternation, rests, repetition, weight and speed. It is a
focused program-building utility; Musaic authors the same structures directly as
tiles. The owned `Late` modifier shifts by a signed cycle amount through the
existing recursive Shift operation, preserving seek behavior and downstream
speed changes.
