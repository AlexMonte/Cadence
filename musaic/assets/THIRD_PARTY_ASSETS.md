# Third-Party Assets Manifest

This directory is the intake path for external assets used by the Bevy Musaic app.

## License Policy

Only assets with these licenses are accepted:

- `CC0`
- `MIT`

Any non-CC0/MIT package must be rejected.

## Intake Checklist

For each imported package, record:

- Source URL
- License name and license file path
- Downloaded archive filename
- SHA-256 hash of the archive
- Imported file list and destination folder

## Approved Source Buckets

### Kenney UI Packs (CC0)

- Source: https://kenney.nl/assets

### Tabler Icons (MIT)

- Source: https://tabler.io/icons
- Destination: `assets/icons/`

## Imports

### Monoid Regular (MIT option of dual license)

- Source: https://github.com/larsenwork/monoid
- Pinned font: https://raw.githubusercontent.com/larsenwork/monoid/c1a36469991fb117da2a39265f1356363506a37d/css/Monoid-Regular.ttf
- License: MIT; `fonts/Monoid-LICENSE` (copyright Andreas Larsen and contributors)
- Download: standalone `Monoid-Regular.ttf`, no archive
- SHA-256: `fd119e732472cc35803480668f316160b0cfa4d5217e7c995dbe91bd9cf19706`
- Imported: `fonts/Monoid-Regular.ttf`, `fonts/Monoid-LICENSE`
- Derived: `ui/tile-glyphs.png`, generated with `../tools/generate_tile_glyphs.py`


## First-Party Assets

- `assets/palette/artist_color.png` — master color grid; semantic tokens live in
  `infrastructure/ui/theme/semantic.rs` (`MusaicUiTheme.semantic`)
- `assets/tiles/` — ortho tile sheets and transform icons (loaded by Bevy)
- `assets/icons/` — port and UI SVGs (reserved for future use)

- `fonts/MusaicSymbols-Regular.ttf` — first-party geometric accidental/arrow
  fallback under the workspace MIT OR Apache-2.0 license; generated from
  `../tools/musical_symbols.py` by `../tools/generate_symbol_font.py` (fontTools).
  No third-party outlines. SHA-256:
  `274658c635d2cb025ba13374cc60b273443bad06d36aab899a8f9cab7a45fa63`.
  The same strokes feed the tile atlas; committed assets require no runtime Python.
