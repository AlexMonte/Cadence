# Cadence · Tessera · Musaic

A Rust workspace for composing music with connected tiles.

| Crate | Responsibility |
| --- | --- |
| **Cadence** | Exact musical time, score queries, scheduling, voices, and audio rendering |
| **Tessera** | The spatial tile language, validation, normalization, and compilation |
| **Musaic** | The editor, project document, persistence, assets, transport, and cross-crate lowering |

Musaic depends on both libraries. Cadence and Tessera remain independent.

## Run Musaic

From the workspace root:

```sh
cargo run --locked --profile dev-fast -p musaic
```

Open the compact connected example:

```sh
cargo run --locked --profile dev-fast -p musaic -- --example
```

Open a saved project:

```sh
cargo run --locked --profile dev-fast -p musaic -- --project "path/to/project.musaic.json"
```

The project folder owns its imported recordings under `assets/samples`. Move
the project JSON and that directory together.

## Compose

Musaic presents a board of typed musical tiles:

- notes and rests form patterns inside Sequence, Alternate, and Layer
  containers;
- values stay attached to the note, transform, or effect they control;
- compatible neighboring ports can connect automatically, while explicit cable
  tools handle longer routes;
- Sound tiles turn note patterns into audio;
- Gain, Pan, Gate, filters, modulation, and effects shape connected branches;
- Output tiles name timeline lanes;
- tricks save reusable tile programs and may accept a pattern input;
- the inspector edits exact numeric values, samples, sounds, and focused
  tile settings;
- accepted edits support undo and redo.

Incomplete edits show diagnostics without replacing the score currently
playing. A valid edit made during playback is prepared and published at the
next accepted boundary.

## Sound and files

Imported recordings become project-owned samples with stable identities.
Instrument banks can describe tuning zones, labels, sustain loops, playback
limits, and hat choking. Preview, live playback, and WAV export use the same
compiled music and sound definitions.

Project files contain one current authored document. Unknown, missing, or
invalid fields are rejected before the open document changes. Preferences and
keymaps are stored separately from songs.

## Included examples

- [Portable instrument banks](musaic/examples/instrument-banks/birds-of-a-feather/README.md)

## Architecture and behavior

- [Musaic architecture](musaic/docs/ARCHITECTURE.md)
- [Tessera language contract](tessera/LANGUAGE_CONTRACT.md)
- [Cadence host API](cadence/HOST_API.md)
- [Board placement](musaic/docs/BOARD_PLACEMENT.md)
- [Connections](musaic/docs/CONNECTIONS.md)
- [UI interactions](musaic/docs/UI_INTERACTIONS.md)
- [Flow tiles](musaic/FLOW_TILES.md)
- [Tricks](musaic/TRICKS.md)
- [Sound design](musaic/SOUND_DESIGN.md)
- [Sample banks](musaic/SAMPLE_BANKS.md)

## Verify

```sh
cargo test --locked --offline --workspace --no-fail-fast
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo check-wasm --locked --offline
```

The native app also needs a graphics device supported by Bevy and an audio
output device for live sound.
