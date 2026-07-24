# Musaic compose UI layout

Wireframe regions (see `assets/prototype_interface.png`). The board stays center-weighted; edge drags reveal auxiliary panels. Structure / style / behavior follow Bevy `Node` + Feathers tokens under `infrastructure/ui/`.

## Module map

```
infrastructure/ui/
  theme/       # MusaicUiTheme tokens, Feathers mapping, motion
  widgets/     # button, panel, dialog card
  screens/     # MainMenu, Loading, UnsavedDialog
  shell/       # editor root, top chrome, breadcrumbs, rebuild
  board/       # 3D viewport reconcile / picking / ghost
  inspector/   # paints InspectorLayout DTOs
  controls/    # tile palette 3D viewport
```

## Regions

| Region | Position | Behavior |
|--------|----------|----------|
| Menu bar | Top | In-editor chrome (`shell/menu`): Play / Tiles — not the boot MainMenu |
| Board | Center (flex) | 3D Tessera board viewport (`board/`) |
| Inspector | Right (~35%, min 300px) | Context lens from `InspectorPaint` |
| Surface tabs | Bottom | Breadcrumb trail from `BreadcrumbPaint` |
| Minimap | Left of board | Hidden by default; drag board left edge or `M` |
| Piano roll | Above board | Present only when `WorkspaceLayoutKind::TimelineStackedOverBoardInspector` |

## WorkspaceLayoutKind (honest shell tree)

| Kind | Shell tree |
|------|------------|
| `ComposeBoardInspector` | Top menu → board+inspector → breadcrumbs (**no** timeline band entity) |
| `TimelineStackedOverBoardInspector` | Top menu → **timeline band** → board+inspector → breadcrumbs (timeline open by default) |

Dragging the breadcrumb timeline grip emits `EnterTimelineMode` / `EnterCompose` so the shell structure matches the layout kind.

## Theme tokens (`MusaicUiTheme`)

| Group | Roles |
|-------|--------|
| `spacing` | `xs`…`xl`, `panel_gap`, `shell_padding`, `panel_padding` |
| `radii` | `sm` / `md` / `lg` |
| `typography` | `body`, `title`, `hero`, `brand` |
| `chrome` | `window_bg`, `panel_bg`, `panel_inset`, `border`, `button_*`, `overlay_scrim`, `accent`, `danger`, `playhead`, `edge_handle*`, `diagnostics_bg`, `crumb_*`, `board_clear`, `palette_*` |
| `semantic` | Artist palette: container / output / atom_* / flow_* / focus / empty_cell |
| `board` | 3D material base colors (grid, tiles, connections, drag preview) |
| `motion` | `panel_slide_secs`, `panel_slide_distance_fallback` |

Feathers `UiTheme` is built from the same tokens via `theme::feathers_theme_from` (replaces bare `create_dark_theme()`).

## Widget catalog

| Widget | API | Use |
|--------|-----|-----|
| Button | `widgets::musaic_button` / `musaic_clickable` | One Activate-on-click path for shell chips |
| Chrome button | `widgets::musaic_chrome_button` | Themed bg/border button |
| Panel | `widgets::spawn_shell_panel` + `PanelBackdrop` | Shell / inspector / minimap frames |
| Dialog | `widgets::spawn_dialog_overlay` / `spawn_dialog_card` | Unsaved-changes and future modals |

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

See `infrastructure/ui/musaic_tile.rs`, `tile_visual.rs`, `board/`.
