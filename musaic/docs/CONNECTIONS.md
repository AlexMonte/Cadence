# Root-board connections

Root-board wiring uses Tessera's footprint-aware spatial endpoint bindings.
`domain/document/connection_policy.rs` owns type compatibility, endpoint occupancy,
and exact endpoint selection. For example, a 5×1 container at `(0, 0)` reaches a
tile at `(5, 0)` on its east edge; `(6, 0)` is not adjacent. Coordinate deltas alone
are not connection authorization rules.

## Wire tiles

The root library includes **Wire** under **Flow & output**. It occupies one cell
and carries its incoming notes, numeric pattern, or effect controls unchanged.
Place wires beside one another to extend a route. Contextual connection planning
chooses input and output sides, including turns; the normal side controls can
change those directions. The line and arrow on the face follow the actual ports.

Wire is Tessera's parameterless identity transform (Musaic prototype 32). Its
output retains the upstream stream type, so routing a note through a wire cannot
turn it into a numeric speed or gain input. Type inspection follows only that
wire's upstream path and tolerates unrelated unfinished authoring. Placement,
movement, duplication, undo, and persistence use the existing transform path.

Explicit endpoint relations take precedence over spatial inference for the same
input. Changing a side removes that endpoint's previous explicit relation;
renaming a flow member updates its explicit endpoints. Moving a tile preserves
explicit cables while automatic spatial links follow adjacency.

## Choosing a connection

`authorize_connection` returns an `AuthoredEdge` containing the exact output,
input, shared side, and any unused same-side endpoints that must be disabled to
make the connection unambiguous. It is a pure preview: it does not change the
program.

Compatibility follows Tessera's compiler rules. Notes and musical patterns feed
pattern inputs; numbers feed value or compatible control inputs. Custom socket
types and ordered Flow members remain authoritative. Type inspection can examine
a valid source while unrelated parts of the board are unfinished.

A connected input or output is occupied. The planner uses another compatible free
endpoint when available, preferring useful existing directions and declared
member order. It does not add group members, repoint an occupied port, or duplicate
one pattern into every unused member. Unused same-side conflicts may be turned Off
to ensure that the chosen input has one source. Feedback loops and changes that
would remove or add an unrelated connection are rejected.

`transaction::connect_tiles` writes the exact authorized pair into the candidate
document. The command validates that candidate before committing it. Visible
ports and Tessera input are both derived from the committed bindings. Explicit
connection undo/redo records the canonical before/after change, including
previous unused side assignments.

## Side controls and inactive outputs

A side control edits actual endpoint bindings. Source-only tiles cycle between
Off and Output; sink-only tiles cycle between Off and Input. Tiles supporting both
roles use available compatible roles before Off. A role whose endpoints are
already occupied on other sides is skipped. An incompatible or saturated side
keeps its existing bindings and reports why it cannot change.

Facing neighbors use the same exact, typed connection policy as explicit wiring.
Disabling an output clears its exact destination inputs; other sides and named
members remain intact. Display markers are derived from the committed bindings,
and undo restores both endpoints' exact prior settings.

An output with no physical input is an inactive editor track. Live playback and
export omit that output while preserving its tile and lane name. Connected tracks
keep playing; reconnecting restores the completed musical branch. If every output
is disconnected, export renders silence for the requested duration. Unfinished
note syntax and malformed partially connected routes still report errors and
retain the last accepted live revision.

## Contextual placement and movement

`connection::plan_contextual_connections(previous, current, affected, preferred)`
returns proposed bindings, exact added connections, and readable feedback without
mutating the document. `reconnect_after_edit` commits this plan after geometry is
projected to the live board and before the edit is recorded in history.

Placement checks adjacent tiles even when none is focused. The focused tile or
selection is a preferred neighbor, not the only candidate. New tile endpoints are
considered safely before compatible connections are added; harmless initial
port directions can remain visible.

Movement preserves the exact existing endpoint pair whenever the tiles remain
adjacent, including a move around the partner to another side. Moving a note from
the west of a Choice to its north therefore keeps the same named option. Existing
shared-output connections are restored before new free ports are considered.
Unused authored directions are retained when safe.

If adjacent tiles are incompatible or their matching ports are full, placement or
movement can finish without a new connection and reports the reason. A position
that would block or redirect unrelated wiring on a shared edge is rejected.
Container stack edits use expression order and ownership rather than root-board
spatial wiring.

## Executable and visible state

`MusaicDocument::connections` is the authored authority. Default directions are
hints for planning, not evidence that a wire exists. `export_document_program`
copies the committed bindings and explicit relations into Tessera input, while
the board projection derives port markers and cables from those same values.

`connections_from_program` projects real matched input/output pairs, so an
enabled output facing an unbound neighbor does not draw a phantom wire.
Disconnecting one destination clears its exact inputs and keeps a shared source
output enabled while other destinations still use it. Named identities and side
settings survive undo/redo and project persistence.

## Connection previews and playback

While `EditorMode::Connecting` is active, `sync_connection_preview` draws a
translucent, non-pickable wire toward the hovered root-board tile or slot. It uses
the cursor and visible-board projection after scene synchronization and does not
change camera framing or `BoardGridAnchor`.

During playback, active route stages show outlines and small packets move from
source toward destination along active connections. Value/control paths are marked
while used by an active musical route. Activity uses the accepted score's routing
snapshot and Cadence's actual playback clock; pending edits and the future timing
preview do not supply a different route. These overlays show musical activity,
not PCM metering or residual effect tails, and never intercept editing input.


## Deliberate cables across empty cells

The inspector's **Connect to tile…** chooser and the pointer connection tool
can connect distant root-board tiles. `authorize_manual_connection` shares the
existing port typing, occupancy, ambiguity and cycle checks. It chooses one exact
free endpoint pair and commits an explicit relation. Automatic placement still
uses `authorize_connection` and requires adjacency.

The preview lists both tile names, port names, sides and stream type. Confirmation
revalidates the exact plan, including placement and chosen endpoints. A stale or
incompatible plan cannot overwrite an occupied route. Deliberate cables preserve
their endpoint identities and sides through movement; disconnected inputs remain
disconnected. Copying a source does not copy or steal an external destination.

Cables render as orthogonal segments on the authored sides: green for musical
patterns and brass for numeric/control patterns. Touching faces get a short link.
A backward cable uses an outside lane around its endpoint tiles. A cable's playback
packet traverses the whole route, with only one segment showing it at a time;
reduced motion fixes it at the route midpoint. The pointer preview reuses five
segment meshes, caches endpoint planning by source/destination/revision, and is
removed when the tool ends.

Each segment picks the same logical connection. Clicking one disconnects the
route; Undo restores its exact endpoints, sides and relation. The scene owner
retains connection entities until the source, destination, side or type changes.
There is no per-frame document mutation or alternative graph authority.

Routing is deterministic. The chooser offers the policy-selected endpoint pair;
the document does not store editable waypoints or a second routing graph.
