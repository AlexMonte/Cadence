# UI interaction channels

The editor has a native Bevy tile library beside the rendered board. Infrastructure
reports pointer and keyboard input; the application layer interprets it and emits
commands. Widgets do not mutate the document or `EditorSession` directly.

## 1. Shell commands

The footer's **Master** bars measure left/right sample peaks sent through the
native device callback, after channel mapping and before device-format clipping.
They include auditions and effect tails, use a short visual decay, and show
**no device data** if callbacks stop. **Clip · reset** stays visible after an
overload until reset. These levels cannot measure system volume or the speakers.

**No sound?** opens a read-only check of device delivery, playback acceptance and
obvious project candidates: missing output routes, closed owned Gate tiles, zero
owned Gain or instrument volume, and unavailable instrument samples. Selecting
a check opens its actual tile through the existing navigation/selection commands.
Unused or deliberately silent tiles can appear; arbitrary connected or modulated
controls and system settings still need inspection. Escape closes the modal and
restores focus. Meter controls stay on the bottom row so changing status text
cannot move a pressed button before release. Browser output remains unavailable.

The main menu's **Start a first loop** creates ordinary `C4 E4 G4 ~` tiles,
a visible instrument and an output, connected at 120 BPM and four beats per
cycle. It opens stopped, with pattern timing visible. The optional guide asks
the user to Play, open the first note and change its pitch, then save. Progress
follows accepted playback and an accepted pitch/octave change. **Dismiss guide**
hides it without a music edit and removes its controls from the Tab order.
Opening another project or leaving Editor clears the guide, including when node
IDs repeat. Tutorial state is never saved into the project or music history.

Menus, breadcrumbs, inspector controls, and keyboard shortcuts emit
[`EditorCommand`](../src/application/command/mod.rs) values through
`EditorCommandBus`:

```
menu / breadcrumb / inspector / keyboard
    → EditorCommandBus
    → dispatch_commands
    → document, attention, selection, or transport change
```

Examples include `ToggleDrawer`, `TransportToggle`, `NavigateToSurface`,
`StartConnection`, and `JumpToTimelineSource`. Focused text and exact-number
fields consume their editing keys before global shortcuts run.

The editor menu bar groups commands under **File**, **Edit**, and **View**.
File offers New project, Open project, Save, Show project file, and Export audio. New and Open ask
to save unsaved changes; Cancel keeps the current project. Cmd/Ctrl+N creates a
project, Cmd/Ctrl+O opens one, and Cmd/Ctrl+S saves. Opening uses the native picker
and the existing unsaved-changes prompt. Edit holds undo/redo and structural tile
commands. View toggles the tile library, timing preview, and minimap. Escape
closes an open menu without leaving the editor. File → Close project is the explicit
exit path and retains unsaved-change protection.

Pattern timing has **Previous cycle**, **Next cycle**, and **Follow playback**.
Browsing changes only the preview; it never seeks, pauses, or edits the music.
The cycle label adds **Preview** while browsing, and the playhead is hidden when
the playing position is outside that window. Follow playback returns to the
current cycle (or the cycle where a queued edit will begin). Stop, Panic, and
opening a project restore following from cycle one. These controls also support
F6 to the timing region, Tab, and Enter/Space. Their entities stay alive across
cycle refreshes so playback does not interrupt a click or discard their focus.

The footer reports **Checking edits**, **Edits accepted**, **Queued**, or
**Edit rejected**, even with timing closed. Queued means the audio runtime has
accepted the edit for the displayed cycle; it changes to **Playing current
version** only after that boundary. Browsing another timing cycle does not freeze
this status. Rejection explains whether the previous valid version is kept or
still playing; its reason remains in the error banner. A timing-preview failure
is identified separately. The status is a polite accessibility announcement and
updates only when its content changes. Accepted/playing describes revision and
transport state, not measured speaker output or an audibility check.

**Edit → Transpose** actions and command search move selected notes up or down by
one semitone or one octave. Selecting a container includes its nested notes;
selecting a note's octave or modifier targets that complete note. Rests, rhythm,
modifier values, sounds, and connections stay unchanged. Each operation is
one Undo step. Existing pitch pieces keep their identities; missing accidentals
or octaves are added when needed. Octave moves retain spelling, and chromatic
moves retain a flat spelling where possible (otherwise they use sharps).
An existing accidental becomes an explicit natural when appropriate. If any
result would leave C−1 through B9, the entire operation is rejected. During
playback, the existing queued-edit rule applies at the next cycle boundary.

With the default shortcuts, arrow keys move the board cursor; Shift+arrows extends a selection. Enter opens a
container, inspects a tile, or places an armed tile. **Keys** / F1 explains the
optional Vim profile and reduced-motion preference. These preferences persist
independently of the project. Cmd/Ctrl+K or Edit → Search commands opens a searchable
list of common actions; arrows choose, Enter runs, and Escape cancels. Dialogs own
modal tab groups and restore their initiating focus on dismissal. Escape from a
focused control returns to the board without closing the project.

**Keys → Customize shortcuts…** lists every board/editor binding in the active
profile. Choose an action and press a new key combination; this replaces all
aliases for that action in both profiles. Escape cancels recording without
changing the binding. Conflicts in either profile are rejected locally. Unbind
selected, Reset selected, and Reset all shortcuts are available; a per-action
reset is rejected if another override now occupies its default key. The
**Character shortcuts** switch disables unmodified letters, punctuation, Vim
counts and `gg`, while retaining arrows, Space and modified commands. Shift+Space
stops transport by default. The help and region footer show current bindings.

Tab, Escape, local field editing and OS window/application keys keep their
built-in roles. Region navigation can use F1–F12 (with optional modifiers), so
it cannot replace ordinary text input while a field owns focus. Bindings use
physical key positions and a shared Cmd/Ctrl modifier. Changes persist in editor
preferences, outside the song and Undo history. Native Import/Export shortcuts
uses validated JSON files up to 32 KiB; invalid imports leave bindings unchanged.
Browser builds support editing and local preference storage; shortcut file
import/export is currently native only.

F6 and Shift+F6 move between the board, toolbar, inspector, tile library, and
pattern timing. Closed regions are skipped. Returning to a region restores its
last surviving control; returning to the board restores cursor input. Dialogs
retain their focus trap. The footer names the focused region, and scroll panels
bring focused controls into view, including a library nested inside the inspector.

The library's **All tiles / Recent / Favorites** controls combine with search.
Arrow Down from search enters the first result directly. Tab reaches the controls
and result grid; arrows browse the tiles. Press **F**
while a tile is focused, or click its **+ / −** control, to toggle a favorite
without starting placement. Recent lists the last twelve unique choices (including
choices whose placement was canceled); Favorites holds up to 64. Collections
persist in editor preferences and never enter song files or Undo history.
Only choices available in the current context appear. Named pattern favorites
resolve by name in the open project and do not import definitions from other songs.
Recent and Favorites each offer **Clear** for their entire saved collection,
including entries hidden by search or unavailable in the open project. Reaching
the favorite limit shows a local explanation without changing the collection.

**Insert notes** is available from Edit, command search, or Insert while the board
owns input. Vim navigation adds `i` before and `a` after the focused expression.
Enter complete notes such as `C4 F#4 Bb3 ~`; every note requires an octave (0–9),
and `~` is a rest. The form previews the order and equal relative weights of the
new expressions. Invalid text stays editable. Arrow/Home/End and Backspace/Delete
edit the field; Enter submits once, Escape cancels, and Tab reaches its buttons.
The form closes only after the dispatcher accepts the insertion; a rejected
destination leaves the text and reason in place.

Turn **Step entry: On** to keep the form open after each successful insertion.
Enter or Insert commits the current notes/rests, clears the field, and advances
after the last complete expression. The same text field retains keyboard focus.
The command receipt supplies the exact next target; a new root or layer sequence
continues inside that sequence. Invalid input does not advance. **Done** or Escape
discards only the unsubmitted draft; each committed insertion has its own Undo.
Step entry resets to Off when the form is reopened and accepts ordinary notes and
rests.

At an empty root cell this creates a sequence; selecting a root pattern appends
to that pattern. Within a sequence, insertion moves following expressions without
splitting pitch, octave or owned modifiers. Layers and alternating patterns receive
one nested sequence, so space-separated entry does not silently become a chord.
Arrangements require entering a contained pattern first. One Undo removes the
whole insertion and restores the previous order. A new root sequence still needs
an instrument/output connection to be heard; this form does not imply audition.

**Connect to tile…** in the inspector opens a compatible-destination chooser.
The command search also offers **Connect selected tile…**. Search names, board
positions, or port names; arrows select a result and show its provisional route.
The preview names both tiles, the output/input ports and sides, and the stream
type. Enter or Connect commits; Escape cancels and restores the initiating focus.
The list includes compatible nearby and distant destinations using the routing policy's
chosen endpoint pair. Existing connections, incompatible inputs, and feedback
loops are excluded. Opening this chooser cancels an unfinished placement tool.

Confirmation revalidates the exact previewed ports and positions through the
existing connection transaction. A matching dispatcher receipt closes the form
only on acceptance; rejection keeps the query and reason visible. One Undo restores
the original ports and cable relation. **Draw cable** in the inspector starts pointer routing: click a compatible tile
to connect or press Escape to cancel. Long cables cross empty cells and stay
connected when their tiles move. Clicking a cable segment disconnects it. Automatic
connections from placement still require adjacent tiles; wire tiles remain available.

The timeline's bottom grip and the minimap's left grip follow total pointer
travel. Both reveal continuously, snap open or closed on release, and stay alive
throughout the gesture. Timeline visibility resizes the existing shell; it does
not rebuild the handle being dragged.

Container faces are flat brass-framed strips with a symbol in the left rail and
up to 24 authored preview faces in the remaining area. Note stacks show a composed
pitch, green inset frame, offset sheet edges, a ≡ count, and small modifier marks.
Modifier and numeric input faces remain flat white. The shared Monoid glyph atlas
keeps note labels and symbols legible at different board sizes. These are display
changes: authored placement footprints and saved musical data remain intact.
The inspector uses a north/east/south/west compass around the selected tile.
Board connection lines meet the receiving face and carry their arrowhead there.

## 2. Native tile library

[`tile_palette.rs`](../src/infrastructure/ui/controls/tile_palette.rs) builds a
categorized, searchable, vertically scrollable library using ordinary Bevy UI
nodes. Entries have 36 px symbol faces in five columns; hovering reveals
the full tile name above the library. Search still matches full names and categories. The catalog does
not shrink into an offscreen 3D scene as it grows; the old palette camera and
pickable mesh library are retired.

The library grid has one Tab stop. Arrow keys move between tiles, preserving the
nearest column when moving vertically; Home/End reach the first/last result.
The focus hint shows the tile's full name. Enter or Space chooses it, consumes that
key, and returns keyboard input to the board. Move to a destination and press Enter
to place it; Escape cancels the armed choice. Pointer dragging keeps its existing
gesture path. Library search and tile faces also carry accessibility names/roles;
this does not establish VoiceOver usability.

Categories include Numbers, Notes & accidentals, Pattern containers, Rhythm &
operators, Note sound, Sample controls, Effects, Modulation, Connected controls,
and Flow & output. The active surface determines which entries are available.
Numbers 0 through 9 are available in both contexts, and pattern containers can
be nested. Negative values and fractions are entered by editing any number tile;
they do not require separate library entries. There is no Natural tile in the
library: a note with no accidental is natural. The accidental inspector offers
Remove accidental, which preserves the note and its settings and supports undo. Octave is not a separate library entry: a Number can
set a Note's octave. Search filters entries within their categories, and scrolling
keeps the remaining catalog reachable. The 330 px inspector gives the library
all remaining height, without a fixed maximum height.

Library press handling remains one command funnel:

```
Pointer<Press> on a library entry
    → DrawerTilePressQueue
    → process_drawer_press_queue
    → tick_editor_session_drawer_flow
        movement of at least 10 px → begin placement drag
        release before threshold → ArmPlacementTool
```

A click arms the chosen tile for the next valid board click. A drag uses the board
placement preview and commits on release over a valid current target. Releasing
outside the board or over an invalid target ends the drag without placing a tile.
Escape or loss of window focus cancels the placement session.

## Numeric entry

Every slider uses the shared Musaic widget. Click its displayed number to replace
it, type a decimal or fraction, and press Enter to commit. Escape restores the
current value. Invalid text stays focused and does not change the project.
The inline fields for notes, modifiers, effects, sounds, and samples use their
existing exact-value validation and command observers. Dragging the track remains
available, with the existing gesture history grouping.

## 3. Board clicks and drags

Rendered board entities and minimap interactions report `BoardPickEvent` values.
`VisibleBoardState::pick_at` resolves cells to tiles, compact note compounds, or
canonical insertion addresses. The application produces commands such as Focus,
PlaceTile, EnterContainer, ConnectTiles, and CycleConnection. A board miss clears
focus.

A primary press on a placed tile also records its identity and initial pointer
position in `BoardTilePressQueue`. Crossing the drag threshold starts the same
placement session with `PlacementSession::source` set. Release then emits
`EditTiles(TileEdit::Move { node, target })` instead of creating a copy. Ordinary
board picks are suppressed during an active drag.

Dragging a compact note moves its owned expression together, including modifier
values. Inside a container, dropping on another expression reorders the dragged
group before that expression; an empty insertion cell uses its authored address.
Root-board moves keep tile and child identities and use contextual connection
planning at the destination. Occupied root cells are not replaced by dragging.

A Number placed on a Note sets or replaces its octave. Only whole numbers from
−1 through 9 are accepted. Existing accidentals and other modifier-owned values
remain attached to the note. Both click-to-arm placement and drag/drop use this
same command path, including undo/redo.

## Playback feedback

Active notes, their containing tiles, and the stages on their accepted route show
outlines. Small packets move along active connections; value/control routes are
marked while their musical route uses them. Compact note faces aggregate the
activity of their hidden owned pieces.

`runtime::activity` derives this state from Cadence's actual playback position and
the accepted score/routing snapshot, rather than the future timing preview or
pending edits. Seek, held-note control changes, mute/gate changes, pause, and stop
update the activity. `board::playback_activity` reuses persistent, non-pickable
outlines and packet entities. These indicate musical score/gate activity, not PCM
level or residual effect tails.

## Frame order

The main chain is:

```
Input → Commands → DocumentMutation → Compile → Lower → Runtime → SceneSync → RenderUi
```

Within Commands, input interpretation precedes session mutation and dispatch.
Session mutation chains queue consumption, drag detection, current hover
resolution, and release commit in that order. The pointer sample is current-frame;
hover resolution uses the preceding `VisibleBoardState` projection. Scene and
preview systems run after scene synchronization and see its current projection.

UI regions consume application projections. The native library compares its
catalog and search state before rebuilding entries; placement/camera paths do not
use `UiDirty`. Playback overlays update after runtime and board reconciliation.
See [BOARD_PLACEMENT.md](BOARD_PLACEMENT.md), [CONNECTIONS.md](CONNECTIONS.md), and
[ARCHITECTURE.md](ARCHITECTURE.md) for the corresponding ownership contracts.

### Native picker scheduling

macOS Save and keyboard Open use main-thread systems: AppKit's synchronous
pickers must not dispatch from a Bevy worker while the main thread waits for the
schedule. Open after Save or Don't save has the same requirement. Regression
tests check this scheduling property. Project changes and save metadata still
pass through the existing command bus.

The command search has separate **Show tile library** and **Hide tile library**
actions. Repeating either action preserves the requested visibility, including
when moving to an empty cell has already opened the library automatically.

### Board zoom and fitting

View and Cmd/Ctrl+K command search offer **Zoom in**, **Zoom out**, **Fit selected
tiles**, and **Fit whole board**. Zoom keeps the current viewing target. Fit
selected uses the selected visible tiles, or the focused tile if there is no
selection; with neither, it leaves the view alone. Fit whole board frames the
current surface's authored content. Both fitting actions include full tile
footprints and reset panning/orbit. Resizing keeps fitted bounds in view, while
manual zoom remains at its chosen scale. These actions do not edit music or add
undo steps. Label text is inset from connection sockets for readability.

Board faces simplify as their on-screen size decreases. The middle view keeps a
principal label (containers show their type and tile count); the smallest view
uses broad role marks for containers, notes, rests, values, processors and outputs.
Nested previews and modifier badges return when there is room. Longer labels need
more space before appearing, so exact values are not truncated to fit. The legend
identifies simplified views. Select a tile for its full inspector or use **Fit
selected tiles** to enlarge it. Selection corners, connection targets and tile
footprints persist through zoom. Window size and board tilt also affect detail.

### Coordinate rulers

Root-board columns and rows follow the visible camera. A1 is the authored (0, 0)
cell. Columns to its left are −A, −B, …; rows above 1 are 0, −1, … . The inspector
uses the same labels, while project files keep the original signed coordinates.
Zooming out shows fewer labels and wider grid intervals. Pattern interiors show
“Pattern order” instead of root-board coordinates. Rotating the board hides the
axis labels; Fit whole board restores the upright view and rulers.

New/Open/Adopt project replacement resets the previous pan, zoom and orbit and
frames the new document, including when the two documents reuse the same root ID.


### Reference-style toolbar and library

Click the tempo or beats number in the toolbar, type a value and press Enter.
Escape cancels. Tempo accepts 1–999 BPM at one decimal place; cycle length accepts
1–64 whole beats. Invalid input remains editable and sends no document command.
One Undo restores an accepted edit. The adjacent status says Playing, Paused or
Stopped independently of the Play/Pause/Resume button label.

View → Tile library gives the library the right panel even while a long note
stack is selected. Close library returns to that selection. Library opening
and closing do not alter the document, selection or musical timing.


### Inspector sections and scroll memory

Returning to a tile restores its inspector scroll position. Opening and closing
the library also preserves the tile's place; the library remembers its position
by root-board or container context rather than by each insertion cell. A smaller
window or shorter content clamps the position to the available scroll range.
Keyboard focus still scrolls its control into view when necessary.

Instrument source choices appear before connection details. **Sound shaping**
starts expanded; **Saved sounds** and **Save reusable tile** start collapsed.
Use their Show/Hide buttons with the pointer or Tab and Enter/Space. Collapsing
retains the controls and draft text, removes hidden controls from the Tab order,
and leaves focus on the section button. Returning to the same tile or rebuilding
its controls preserves section choices. Existing output names and linked source
actions remain directly visible.

This is presentation state: toggles and scrolling do not edit music or consume
Undo. Up to 64 panel scroll positions and 64 section choices are remembered for
the current editor session. Opening another project, including one with matching
tile IDs, resets them; they are not saved in project files. The existing command
path still owns saved-sound edits and their Undo.


## Reference tile sizes

Root containers occupy five horizontal cells; outputs occupy one cell. The whole
container footprint participates in placement and picking. Project loading
validates that each saved footprint is in bounds and does not overlap another
root tile.


Owned modifier cards separate their symbol from their numeric value (for example
`@` and `4`). Typed and separate-tile speed/weight values use the same inspector
roles. Compound rhythm controls are shown once when both parts would select the
same node and show identical content. A modifier applies to the preceding note
or pattern; its value remains attached when it moves.


### Exact export scope and completed files

Native File → Export audio uses exact whole cycle entry (1–4096, with a
20-minute limit at the current tempo). Enter applies the draft; an invalid or
uncommitted value disables Choose file and export. Escape in the number field
restores its committed value; Escape outside the field dismisses the dialog.
The form shows the project name, duration, tempo and beats per cycle. The current
authored project, including unsaved and queued edits, is captured when the
command chooses the destination. It starts at cycle 1, renders 48 kHz stereo WAV,
and cuts effect tails at the selected cycle boundary without moving live playback.

Closing the form leaves a running export alone. A successful worker write keeps
its project name, cycle count and path for reopening the dialog. Show export file
uses that completion, while File → Show project file uses the last successfully
saved project path. macOS requests Finder selection; other native platforms open
the containing folder. Missing export files report an error in the form.
Failed/new exports clear the previous completion action. These
file operations do not enter the song's Undo history. Browser export remains unavailable.


### Linked source discovery

A reusable trick instance has **Find linked uses** and **Edit source tiles** actions.
Both support pointer activation and Tab/Enter. Source navigation selects the actual
source tile on its containing board and fits the selection into view. The linked-use
dialog lists direct authored instances, including nested pattern locations, five per
page. Choosing a result navigates to that tile. Shared source containers and their
owned notes display the affected trick name and direct-use count ahead of editing
controls. Duplicating an instance keeps the shared link.

The retained dialog refreshes on document revision changes, including Rename/Undo,
and closes on project replacement, even when the new project reuses the same IDs.
Escape closes only this dialog. Browsing and navigation do not enter Undo. This is
an authored reference list, not transitive playback-impact analysis or independent
variation creation.

### Independent variations

Use **Edit → Duplicate as independent variation**, command search, or the action
in a linked tile's inspector. The operation includes complete containing patterns
and upstream tiles, so nested references and shared control values also become
independent. Copied definitions receive distinct names such as “Evening theme
variation.”

The new groups appear below the existing root board with their relative layout,
contents, internal connections and sound settings preserved. They are selected
together and fitted into view; selection transpose can change their pitched notes.
Original definitions and music remain unchanged. Outputs outside those groups are
not connected automatically. Regular **Duplicate** keeps existing source links.
One Undo removes the entire variation; Redo restores its node and definition IDs.
Rejected edits preserve the original selection, music and history.
