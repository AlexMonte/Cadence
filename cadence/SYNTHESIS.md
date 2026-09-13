# Synth voices

`Intent::synth(BuiltInSynthSource::Sine | Saw | Square | Triangle | Noise)` selects
an oscillator. MIDI `Pitch` remains absolute note pitch; signed `Transpose` adds
semitones. Sine is unchanged. Saw and square use two-sample polynomial step
correction; triangle uses polynomial corner correction. These reduce aliasing
without oversampling, allocation, extra oscillators or unbounded harmonic loops.
They are not ideal bandlimited waveforms. At or above half the output sample
rate, pitched sources are silent instead of playing a substituted pitch.

The method follows polynomial residual correction described in the
[Aalto synthesis research](https://research.aalto.fi/en/publications/reducing-aliasing-from-synthetic-audio-signals-using-polynomial-t/)
and [DAFx BLAMP paper](https://dafx.de/paper-archive/2016/dafxpapers/18-DAFx-16_paper_33-PN.pdf).
Rendered regressions measure unwanted, nonharmonic energy at a coherent test
frequency, retaining over 90% of the naive fundamental power and less than 8%
of its alias power at 44.1, 48 and 96 kHz. This is a measured test case, not a
guarantee for every pitch or rapidly changing modulation.

Noise is unpitched, mono white noise before the existing spatial/filter stages.
It uses a fixed counter-based sequence per note, independent of process state,
voice identity or render block size. Retriggering restarts that sequence. A seek
restores its sample position directly. Constant-pitch oscillator seeks restore
phase; prior time-varying pitch and filter history are not reconstructed.

## Useful defaults

`Intent::synth_preset(SynthPreset::Bass | Pad | Percussion)` attaches a typed
sound definition. `SynthPreset::controls()` exposes every default for hosts.
Defaults apply before ambient controls and authored note controls; runtime
control changes retain them. `SynthIntent` equality and hashing include the
preset, so selecting a different sound remains a meaningful score edit.

| Preset | Source | Attack / decay / release | Sustain | Gain | Filters |
| --- | --- | --- | --- | --- | --- |
| Bass | Saw | 4 / 120 / 60 ms | 0.55 | 0.30 | Low-pass 900 Hz, resonance 0.15 |
| Pad | Triangle | 150 / 200 / 400 ms | 0.65 | 0.25 | Low-pass 3500 Hz |
| Percussion | Noise | 1 / 120 / 15 ms | 0 | 0.30 | High-pass 1800 Hz, low-pass 9000 Hz |

Decay goes from peak to sustain; the gate still follows note duration and
Legato. Release can continue after the note slot. Cutoffs use the existing
device-rate-aware filter limits. There is no separate filter-envelope lane, FM, or unison. Ordered low-pass,
high-pass, and drive inserts are available through `InsertChain`. Existing synth polyphony and
voice/effect resource bounds still apply.

Audition uses the same defaults and scales gain by 0.65. Attack/release are
bounded to half the requested preview duration, with a small minimum fade;
gate time leaves room for release, so the total preview stays within two
seconds. Previewing the Pad therefore preserves its slow attack instead of
turning it into a short click.

Run `cargo run -p cadence --example melody_bass -- /tmp/cadence-melody-bass.wav`
to render a 16-second melody, bass line, pad and noise percussion at 120 BPM.
The example needs no samples, device, app or network access.

## Custom instrument defaults

Both `SynthIntent::with_sound_defaults` and
`SampleIntent::with_sound_defaults` carry an immutable `SoundDefaults` value.
It accepts an `EnvelopeDefaults`, gain, `CompressorSettings`, `DelaySettings`,
and `ReverbSettings`. Empty settings preserve existing behavior. A synth
preset's defaults apply first, then custom sound defaults, then ambient controls,
then authored note controls. This avoids duplicate override-lane conflicts when
a pattern sets attack, release, or sends. These defaults also apply to live
sample/synth plans and audition. Inserts remain a separate ordered chain.

`SoundDefaults` participates in intent equality and hashing, so changing an
instrument's envelope or effects creates a distinct sound definition. Host
serialization remains the host's responsibility; Musaic format 9 saves these
settings and validates them before assigning or reopening an instrument.
