# Voice mixing controls

`ControlKey::Pan` accepts `ControlValue::Bipolar(SignedUnitValue)` in `[-1, 1]`.
Zero is center, -1 is full left, and +1 is full right. Nested pan offsets add;
the result is clamped once, during rendering. For example, +0.75, +0.75, and
-0.75 resolve to +0.75 regardless of grouping. Projected combined controls use
`ControlValue::Pan(PanControl)`; `value()` returns their unclamped sum.

Pan applies stereo balance after the existing spatial position, without adding
distance attenuation. The channel factors are `sqrt(1 - pan)` on the left and
`sqrt(1 + pan)` on the right. Center is exactly unchanged; mono signals preserve
total stereo power when panned. Existing stereo channels are balanced separately,
so hard right removes the left channel rather than folding it into the right.
Spatial position still has its existing independent pan and distance behavior.

`ControlKey::PostGain` accepts a finite nonnegative scalar. Nested values multiply
and remain independent of the note's `Gain`, envelope, and velocity. It applies
after voice filters and compression, before spatial placement, pan, and the
reverb/delay send feeds. It is a per-voice control, not a fader over already
ringing shared effect tails.

Both controls support timed constant tiles. Changes update the existing voice
without restarting it. When an outer lane ends, the nested value resumes (or the
identity: centered pan, PostGain 1). Direct seeks reconstruct the value at the
requested time. Pan and PostGain ramps/signals are not supported by these lanes.

A host can wrap each output's complete score in a repeating full-cycle control
track using `Score::with_controls`. Static mixer volume uses PostGain; mute or
solo exclusion sets it to zero. Static mixer pan uses Pan. This preserves the
controls inside each note. Score replacement still follows the host's revision
boundary; these wrappers do not create an immediate output-bus control path.

Validation: `cargo test -p cadence --test audio_pan` renders stereo PCM through
the shared scheduler and renderer, checks center/edges, nested and timed controls,
preserves note onsets/phase, and compares block sizes and direct seeks.
