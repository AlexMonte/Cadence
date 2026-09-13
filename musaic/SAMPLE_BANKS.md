# Imported sample banks

## Import a complete instrument

Select a Sound tile and choose **Import WAV / instrument…**, then select an
`instrument.musaic-bank.json` file. Musaic decodes and checks the entire package
in the background loader, imports every recording as project-owned audio,
creates the ordered bank, and assigns it to the instrument. An invalid
member or a full destination project leaves the project unchanged. Saving the
project copies all decoded WAVs into its own `assets/samples` folder.

The ready-to-import presets for this remake are indexed in
[`examples/instrument-banks/birds-of-a-feather/index.json`](examples/instrument-banks/birds-of-a-feather/index.json).
They include the exact used kalimba, piano, bass, guitar and organ zones, the
LinnDrum kick/snare/hat, and the TR-808 shaker. Their original recordings retain
their attack and sustain boundaries. There are no expanded eight-second loops.

To prepare another WebAudioFont preset, use Python 3:

```sh
python3 musaic/tools/import_webaudiofont.py /path/to/preset.js /path/to/new-instrument --name "My instrument"
```

An exact HTTPS preset URL is also accepted. `--zones 0,2,5` optionally selects
specific zero-based source zones; omitting it includes all zones. The converter
reads data literals and numeric tuning offsets without executing JavaScript.
Encoded recordings are decoded by Musaic during import, so an external audio
converter is unnecessary. The package keeps a `source.json` provenance file.
WebAudioFont's optional custom AHDSR curves are recorded there but are not
automatically converted to Musaic envelope controls; set the instrument's envelope
to the desired song settings. Overlapping key zones follow Musaic's
nearest-root selection, which can differ from WebAudioFont's last-zone priority.

## Instrument package

The manifest contains one ordered bank. Paths are relative to its folder:

```json
{
  "variants": [
    {
      "path": "Piano C4.wav",
      "sample_rate": 44100,
      "options": {
        "root_pitch": 60.25,
        "default_gain": 0.8,
        "sustain_loop": {"start_frame": 2024, "end_frame": 2192}
      },
      "pitch_zone": {"low": 48, "high": 64},
      "bank": "soft",
      "playback_limit_ms": 8000,
      "hat_choke": false
    }
  ]
}
```

`variants` and each `path` are required. Optional
`sample_rate` verifies that decoding preserved the frame scale used by loops.
Bank members can be WAV, MP3, FLAC, Ogg/Vorbis or another format supported by the
bundled decoder; saved projects always own WAV files. Packages permit 1 to 256
recordings, up to 64 MiB per decoded recording and 256 MiB total. Paths and
symlinks must stay within the package directory. This workflow is currently
available in the desktop app.

## Attack and sustain loops

In **Recording settings**, enable **Sustain loop** and enter the exact start and
end frames. Frame zero is the recording's beginning; the end frame is excluded.
Forward playback plays the attack once, then repeats the internal region through
the held note and its envelope release. Reverse playback enters from the other
end. Pitch changes affect speed while the boundaries remain in source frames.
Seeking into a sustained note continues its original loop phase. Slicing clips
the loop to the selected source region; a slice outside it remains a one-shot.
These source loops take precedence over the whole-region Loop tile. Disable the
recording's sustain loop to use a whole-region loop instead.

Loop edits use the same undoable recording metadata command as tuning. Invalid
or out-of-recording boundaries are rejected, including when relinking to a
shorter recording. Audition, playback and WAV export share this implementation.

## Build a bank from individual recordings

Import the recordings you want to use, choose a recording in a Sound tile, and select **Create bank from imported samples** in its recording settings. That recording becomes the lead. Add other imported recordings as variants. The lead keeps its stable sample identity, so existing Sound tiles continue to work.

Recording settings include both sliders and exact numeric entry. Use exact entry for fractional root pitches such as `60.5` or finer gain values such as `1/8`; Enter saves and Escape cancels. Relinking preserves these recording settings, including edits made while the replacement is decoding.

The bank inspector shows the explicit variant order, starting with lead variant 0. Other variants can move up or down or be removed from the bank. **Dissolve bank** returns the lead to a single recording and keeps every imported recording. A recording may belong to only one bank. Missing, duplicate, or invalid member edits are rejected before changing the project or history.

Each variant can have:

- A **Bank label**, such as `soft` or `bright`. A Bank tile on a note filters to matching labels. The Bank tile inspector lists labels already used by imported banks and also accepts exact text. Blank variant labels mean unlabelled recordings; they remain eligible when the note has no Bank tile.
- An inclusive **pitch range**, entered as MIDI pitches from 0 to 127. Among matching zones, the engine chooses the recording with the nearest root pitch. If no zone matches, it chooses the nearest root among the remaining candidates. Each recording's own root pitch controls its tuning.
- A **playback limit** in positive whole milliseconds.
- **Hat choke**, which cuts off other voices in the shared hat choke group. This is the same group used by built-in hats; arbitrary named choke groups are not supported.

A Variant tile chooses a zero-based index after Bank filtering; indexes wrap within the matching variants. Two separately imported takes remain separate variants even when they have identical labels and tuning. With no explicit Variant tile, pitch chooses among the available recordings. The lead recording's gain applies to the complete bank; other members' recording gain still applies when those recordings are selected directly outside the bank.

All bank changes support Undo and Redo. Settings are stored in the project. Banks reference project-owned audio, so saving or moving a project preserves the recordings and ordering. Relinking a member changes its audio without changing its identity or position. If a member is missing, the lead bank becomes unavailable instead of silently renumbering the remaining variants. Relink the missing member to repair it.

Playback, audition, and WAV export use the same decoded-bank loader. Replacing a project unregisters its sample aliases, and replacing a bank leaves already-started voices holding their original decoded buffers. Automated tests in `tests/sample_banks.rs` cover selection, zones, equal-hint variants, ordering, relink, invalid edits, missing assets, playback limits, hat choking, undo/redo, file roundtrips, and export snapshot behavior.
