Layer 1 — Piece Registry (piece_registry)
This is what Psi calls "pieces". It's a static catalog — it doesn't change at runtime. Each entry describes a type of node, not an instance of one:

```rust
struct Port = {
  id: string,
  label: string,
  type: "Pattern" | "Number" | "String" | "Rhythm" | "Any",
  side: TileSide  // for Psi-style grid
}


enum TileSide {
    North,
    South,
    East,
    West
}
Struct PieceDef = {
  id: string,            // "strudel.fast"
  label: "fast",
  category: "Transform", // Transform | Generate | Output | Control | User-Trick
  inputs: Port[],
  outputs: Port[],
  defaultParams: Record<string, any>,
  // The key: how does this piece compile?
  compile: (inputs: Record<string, CodeExpr>) => CodeExpr,
}
For Strudel specifically, your categories map neatly to Psi's piece taxonomy:

Piece Type | Strudel Equivalent    | Example
Selector   | Generator             | note("c3 e3"), s("bd")
Operator   | Transformer           | fast(2), rev, jux
Trick      | User-defined function | composed pattern block
Constant   | Literal value         | 2, "c3", a rhythm string
Action     | Output/Effect         | stack(...), play output




struct NodeId (Uuid)  // UUID
struct EdgeId (Uuid)

pub struct Node {
  id: NodeId,
  pieceId: string,   // foreign key into the Piece Registry
  position: { col: number, row: number },  // grid coords for Psi-style
  params: Record<string, any>,  // overridden constant values
}

struct Edge = {
  id: EdgeId,
  from: (nodeId: NodeId, portId: string),
  to:   (nodeId: NodeId, portId: string ),
}

struct Graph = {
  nodes: HashMap<NodeId, Node>,
  edges: HashMap<EdgeId, Edge>,
  name: String,
}
