# Cadence compose UI layout

Wireframe regions (see `assets/prototype_interface.png`). The board stays center-weighted; only edge drags reveal auxiliary panels.

## Regions

| Region | Position | Behavior |
|--------|----------|----------|
| Menu bar | Top | `Menu · Edit · Play · Comp · Help`, plus `Tiles` (toggle tile drawer) |
| Board | Center (flex) | 3D Tessera board viewport |
| Inspector | Right (~35%, min 300px) | Context lens: header, tile/atom drawer, NESW ports, Enter/Preview |
| Surface tabs | Bottom | Breadcrumb trail (`Home \| …`), click to navigate |
| Minimap | Left of board | **Hidden by default**; drag the board’s **left edge** or press `M` |
| Piano roll | Above tabs | **Hidden by default**; drag the tab bar **upward** |

## Focus rules

- **Empty slot / stack insert** → tile drawer in the **inspector** (not a left column).
- **Tile / container / atom focus** → inspector details + port diagram when on root board.
- **Left edge** → minimap only (never a tile drawer).

## Shortcuts

- `D` — toggle tile drawer in inspector
- `M` — toggle minimap

## Shell sprites (`assets/ui/`)

| Asset | Use |
|-------|-----|
| `panel_bg.png` | Menu, board frame, inspector, minimap, tabs |
| `section_panel_bg.png` | Piano roll band, inset scroll areas |
| `menu_item_slot.png` | Top menu chips |
| `minimap_*_tile.png` | Minimap cell glyphs |
| `minimap_selection_marker.png` | Focused minimap slot |
| Port / edge PNGs | Inspector NESW compass |

## 3D board tiles (`assets/tiles/`)

Ortho sheets (6×32×32 frames each) for board meshes only — not for shell chrome.
The tile drawer is a sole 3D palette viewport (on-tile glyphs); there is no
duplicate label-chip grid and no 2D drawer atlas.

| Sheet | Used for |
|-------|----------|
| `container_Ortho_32x32.png` | Container tiles |
| `output_Ortho_32x32.png` | Output tiles |
| `transform_Ortho_32x32.png` | Trick / transform tiles |

See `infrastructure/ui/cadence_tile.rs`, `transform_tile.rs`, `board_3d.rs`.
