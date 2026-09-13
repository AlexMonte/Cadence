# Flow tiles

The root board palette includes Layer, Merge, Mix, Split, Mask, Switch, Route,
and Choice. Each uses Tessera's default policy when first placed. Flow tiles
have distinct compact faces and stay on the root board; note expressions remain
inside containers.

Select a flow tile to choose its policy and assign each named input or output
to West, North, East, South, or Off. Adjacent tiles on that side connect. Members
sharing a side share its incoming pattern; give separate output branches distinct
sides so each destination has an unambiguous source. Clicking a board port opens
these named controls. The ordinary Connect action is also available.

Group members can be added and removed within the signature's declared limits
and the editor's 128-member creation limit. Newly added members begin Off.
Split-by-pitch exposes an exact whole-number octave threshold. Changes use the
same undo history as placement and structural edits. Click a member's Name
field and press Enter to rename it. The renamed member keeps its position and
side binding; empty or duplicate names are rejected. For label routing, use
note labels such as `c` and `d`. For field routing, use field names such as
`gain`, `legato`, or `plain`.

The document stores the complete Tessera flow node: kind, policy, input/output
signature, port count rules, socket defaults, and ordered member names. Import,
copy, move, undo, redo, save, and reopen retain those values and individual side
bindings. Changing a policy does not reset other settings. Custom signatures
and imported member labels are retained; custom signature editing remains a
language/API operation.

`tests/flow_tiles.rs` covers all policies, custom signatures and member names,
placement identity, independently routed streams, Cadence pitches and PCM,
member-count validation, undo/redo, and persistence. UI tests exercise actual
policy/endpoint button observers, focused member naming, and distinct compact glyphs.

Policy queries retain recursive input clocks. Switch/choice decisions, index
splitting, masks, and mixing are evaluated against each whole event's onset,
so partial previews and later seeks choose the same held notes. Field and
control routing use the actual connected pattern. `flow_runtime_semantics.rs`
compares all 26 authored policies against Cadence over multiple cycles and far
seeks, checks mask/mix control values and held voice identities, and verifies
live PCM across different renderer block sizes. It also checks that a later
gain transform applies after mixing and that extremely long repeating held
notes are rejected before excessive query preparation. Tessera's structural Layer,
Merge and MaskClip remain native score operations; remaining policies use the
immutable generic query-source adapter during planning.
