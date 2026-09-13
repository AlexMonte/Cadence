# Instruments for Birds of a Feather

These twelve portable packages contain the exact 28 recordings selected for the
song, with original tuning, pitch zones and internal sustain-loop coordinates.

Place or select a Sound tile in Musaic, choose **Import WAV / instrument…**, and open the
desired folder's `instrument.musaic-bank.json`. The complete instrument is
imported as one operation. Saving the project copies its decoded recordings into
the project's owned sample folder.

`index.json` maps song track names to packages. Each package's `source.json`
records original source URLs and tuning/loop provenance. The GM presets come from
the WebAudioFont data referenced by Strudel; the drum recordings are the selected
LinnDrum and Roland TR-808 samples from Strudel's drum-machine bank.

See [sample banks](../../../SAMPLE_BANKS.md) for the importer and converter, and
the Musaic editor for a
complete project that uses every package.
