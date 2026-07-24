# Musaic sprite manifest (per-file, pre-atlas)

Each PNG listed here is registered in `adapter/load_up` and loaded before the editor runs.
Add a new file under `assets/`, then add a `#[dependency(path = "...")]` field on the matching pack.

**Rendering:** all packs use **nearest-neighbor** filtering. UI sprites are laid out at **2×** integer scale (`PIXEL_UI_SCALE` in `adapter/load_up/sampler.rs`) so 32px art displays at 64 logical pixels without blurry filtering.

## UI (`UiSpriteAssets`)

| File | Use |
|------|-----|
| `ui/breadcrumb_slot.png` | Active bottom-tab chip |
| `ui/scroll_bar.png` | Inspector title strip |
| `ui/scroll_anchor.png` | (reserved) scroll chrome |
| `ui/control_map_connection.png` | Port compass — control map |
| `ui/scalar_value_connection.png` | Port compass — scalar |
| `ui/edge_disconnected_.png` | Port compass — closed |
| `ui/edge_direction_none.png` | Port compass — off |
| `ui/edge_*_connection.png` | Connection edge variants |
| `ui/edge_direction_{north,south,east,west}.png` | Directional edge art |

## Board ortho strips (`OrthoTileAssets`)

| File | Use |
|------|-----|
| `tiles/transform_Ortho_32x32.png` | Transform / trick tile shell (6 frames) |
| `tiles/container_Ortho_32x32.png` | Container shell |
| `tiles/output_Ortho_32x32.png` | Output shell |

## Transform icons (`BoardTileIconAssets`)

| File | Use |
|------|-----|
| `tiles/transform_fast.png` | Trick prototype 0 |
| `tiles/transform_slow.png` | Trick prototype 1 |
| `tiles/transform_legato.png` | Trick prototype 2 |
| `tiles/transform_gain.png` | Trick prototype 3 |

## Atom drawer (`AtomTileAssets`)

| Pattern | Use |
|---------|-----|
| `tiles/atom_note_{a..g}.png` | Pitch names |
| `tiles/atom_note_silence.png` | Rest |
| `tiles/atom_accidental_{sharp,flat}.png` | Accidentals |
| `tiles/atom_scalar_{0..9}.png` | Octave / digit literals |
| `tiles/atom_operator_*.png` | Operator tiles |

Lookup: `AtomTileAssets::image_for_atom(&AtomValue)`.
