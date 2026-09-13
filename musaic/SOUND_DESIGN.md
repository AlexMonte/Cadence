# Sound design with connected tiles

Sound follows the visible route on the board:

```text
note pattern → processing tiles → Sound → Output
```

A Sound tile chooses how incoming notes become voices. An Output names the
timeline lane that receives the completed branch. Sound and mixing controls
belong to explicit connected tiles, so changing or disconnecting a branch has
the same meaning in the editor, preview, playback, and export.

## Sound tiles

Sound tiles can select:

- a synth waveform or preset;
- a fixed built-in drum;
- a project-owned recording;
- an imported instrument bank.

The Sound tile owns its source defaults, envelope, and ordered inserts. Changing
the source preserves authored settings that still apply. Imported recordings
retain stable project identities.

Saved sounds are reusable snapshots. Capture a named sound, apply it
to another Sound tile, rename it, or remove it. These actions support Undo
and Redo and never modify tiles that already use an independent copy.

## Processing tiles

Place controls in the musical branch where they should apply:

- Gain sets an explicit level;
- Pan positions the signal in stereo;
- Gate changes audible note length;
- PostGain adjusts the completed branch;
- low-pass, high-pass, and drive shape tone;
- delay and reverb provide bounded space effects;
- compressor controls dynamics;
- modulation tiles vary supported parameters over musical time.

Tile order is processing order. Moving a control changes its scope while
preserving the value owned by that tile. A disconnected control has no hidden
effect.

Every numeric inspector control has an exact field. Enter a whole number,
decimal, or fraction; Enter commits and Escape cancels. Invalid values remain
available for correction and do not change the document. A slider gesture and
an accepted exact edit each create one history entry.

## Shared meaning

Musaic lowers authored sound settings into immutable Cadence sound definitions.
Tessera note controls override the matching sound default at their declared
scope. Cadence then applies event controls and continuous signals while rendering
the voice.

Preview, live playback, audition, and WAV export use the same prepared score and
decoded samples. Export stops at the requested cycle boundary, including when an
effect tail would continue beyond it.

Seeking reconstructs musical events and their current controls. It does not
simulate audio that occurred before the requested window, such as an already
decaying delay tail.

## Velocity modulation

The Modulation library includes Velocity modulation. Its inspector provides
waveform, rate, phase, minimum, maximum, and seed controls. Velocity remains
between zero and one.

Connect a note pattern to Velocity and connect a modulation pattern to the
amount input. The signal is sampled when each note starts, and that intensity
stays fixed while the note rings. Seeded random and stepped shapes produce the
same result for the same exact musical time.

Place Velocity before a timing operation when the intensity should travel with
the source notes. Place it after the timing operation when it should sample the
resulting onsets.

Focused tests cover sound editing, processing order, exact values,
modulation, live score replacement, project round trips, and rendered audio.
