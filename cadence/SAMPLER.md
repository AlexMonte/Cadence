# Cadence sampler contract

Run the self-contained demonstration from the workspace root:

```sh
cargo run -p cadence --example sliced_break -- /tmp/cadence-sliced-break.wav
```

It generates a deterministic stereo drum break, divides it into 16 slices, and writes four repetitions of a two-bar arrangement. The second bar changes slice order, reverses selected slices and changes their pitch/rate. No downloads, audio device, Musaic or Tessera are required.

| Operation | Typed Rust interface | Meaning |
| --- | --- | --- |
| Region | `SampleIntent::region(start, end)` or `PlaybackStart` / `PlaybackEnd` scalar controls | Normalized bounds, `0 <= start < end <= 1`. Bounds take effect when a note starts. |
| Equal slice | `SampleIntent::slice(index, count) -> Result` | Zero-based selection inside the current region; 1–65,536 slices, `index < count`. This changes source bounds, without changing authored note slots. |
| Reverse | `SampleIntent::reverse(true)` or `Reverse` boolean control | Play the selected region backward; sampled at note start. |
| Fit | `Fit` boolean control | Resample the region to the full authored slot's wall-clock duration, before rate and pitch changes. This changes pitch. Legato and ClipLength still control the sounding gate separately. |
| Rate | `SampleIntent::rate(value)` or `PlaybackRate` scalar control | Positive multiplier. Patterned updates retain fitting and root-pitch multipliers. Resolved rate before transpose must be finite and at most 65,536. |
| Transpose | `Transpose` signed semitone control | Multiplies playback rate by `2^(semitones/12)`. Samples change duration along with pitch. |
| Loop | `Loop` boolean control | Repeat the selected region until the envelope/gate ends or the voice is released. Loop direction follows Reverse. Start/end/Loop are note-start settings. |
| Bank / variant | `SampleBank` choice / `SampleVariant` scalar controls | Choose among decoded entries registered under one logical sample name. Explicit variant indices wrap within the selected bank. |
| Root pitch / pitch zones | `SampleLoadOptions::root_pitch` / `pitch_range` | MIDI-note metadata on a decoded variant. `Pitch` selects/repitches from that root; no root metadata means no assumed absolute sample pitch. |
| Attack then sustain | `SampleLoadOptions::sustain_loop` / `SampleLoopRegion::new(start_frame, end_frame)` | Source-frame region with exclusive end. Plays into the internal loop once and repeats it through the note/release. Pitch changes preserve source coordinates; seeking preserves phase. Source loops take precedence over whole-region looping and clip to the selected playback region. |
| Choke | `SampleLoadOptions::choke_group` | Existing `ChokeGroup::Hat` fades a previous hat when a new one starts. Arbitrary named groups are still future work. |
| Retire asset | `SampleBank::remove(name)` | Remove all variants from future resolution on the control thread. Already-started voices keep their decoded audio references. |

Fitting a 100 ms source to a 200 ms slot sets rate 0.5 and lowers its pitch by an octave. Adding `PlaybackRate = 2` or `Transpose = 12` afterward returns its source duration to 100 ms. The note's authored slot remains 200 ms. Fit is rate-based resampling; pitch-preserving stretching is not implemented.

Fractional resampling clamps interpolation to one-shot region edges and wraps taps inside loops, so neighboring slices cannot leak into the output. Forward/reverse loop advancement uses constant-time modulo arithmetic, including high rates and direct seeks. Plain sample seeks use elapsed source time instead of warping the region by the percentage of its note slot.

Direct seek is verified for constant-rate regions, fitted samples and forward/reverse loops. Reconstructing the source phase of a note whose rate or pitch varied earlier requires integrating that modulation history; that case is not yet guaranteed. Effect history also resets on seek. Crossfaded loop seams, manual slice-marker editing, arbitrary choke groups and pitch-preserving stretching remain later sampler work.

## Sound audition

`PlaybackRuntime::audition(intent, pitch, duration)` accepts a sample or synth intent, an optional MIDI pitch in [0, 127], and a positive `Duration` of at most two seconds. It starts at the next unrendered device frame without replacing the score, moving transport, or changing keyboard mappings. A one-shot source can finish earlier. Preview gain is 0.65 times the source gain, with attack/release fades of at most 5 ms included within the requested duration.

One reserved audition slot is independent of the song's 128-voice budget and eight-synth polyphony. A new audition replaces the prior one, and a preview cannot choke or steal a song voice. `stop_audition()` stops that slot; Panic and transport invalidation silence it along with other sound. Unsupported intents, missing samples and invalid values return errors before replacing a valid preview.
