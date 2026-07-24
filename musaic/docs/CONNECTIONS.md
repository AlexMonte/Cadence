# Root-board connections

`domain/document/connection_policy.rs` owns whether a connection is legal.
`authorize_connection` derives the source side exclusively through Tessera's
footprint-aware `neighbor_at_side`; coordinate deltas and
`spatial_side_between` are not authorization rules. Consequently, a 2×2
container at `(0, 0)` is adjacent to a tile at `(2, 0)`, while a tile at
`(3, 0)` is not.

`transaction::connect_tiles` is a homomorphism over that policy: authorize an
`AuthoredEdge`, bind its Tessera endpoints, set both corresponding
`port_endpoints` to the policy's bindable kind, and synchronize the live
Tessera program back into the document. There is no second connection
interpretation in the transaction layer.

Placing a root-board tile attempts this same authorized operation with the
focused tile (otherwise the selection anchor or first selected tile) as its
preferred source. Failed authorization is not a failed placement; it merely
means no automatic connection is created. Stack placements never auto-connect.

Connections have two synchronized stores:

- Tessera root-surface endpoint bindings are the executable wiring.
- `PortEndpointStore` records the bindable authored state that validates and
  projects those bindings.

While `EditorMode::Connecting` is active, `board_3d::sync_connection_preview`
renders a translucent, non-pickable wire from the source tile center to the
pointer's hovered root-board tile or slot. It reads the cursor and visible-board
projection after scene synchronization; it does not influence camera framing
or `BoardGridAnchor`.
