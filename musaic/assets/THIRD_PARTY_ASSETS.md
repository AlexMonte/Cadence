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

_No third-party packages imported yet._

## First-Party Assets

- `assets/palette/artist_color.png` — master color grid for UI (`src/ui/artist_palette.rs`)
- `assets/tiles/` — ortho tile sheets and transform icons (loaded by Bevy)
- `assets/icons/` — port and UI SVGs (reserved for future use)
