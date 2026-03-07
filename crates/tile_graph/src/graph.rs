use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{EdgeId, GridPos, TileSide};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub piece_id: String,
    #[serde(default)]
    pub inline_params: BTreeMap<String, Value>,
    #[serde(default)]
    pub input_sides: BTreeMap<String, TileSide>,
    #[serde(default)]
    pub output_side: Option<TileSide>,
    /// Optional user-defined display name for this node instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional per-node opaque state blob (for stateful pieces).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_state: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub from: GridPos,
    pub to_node: GridPos,
    pub to_param: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    #[serde(with = "grid_nodes_serde")]
    pub nodes: BTreeMap<GridPos, Node>,
    pub edges: BTreeMap<EdgeId, Edge>,
    #[serde(default)]
    pub name: String,
    /// Grid width in columns. Defaults to 9.
    #[serde(default = "default_grid_cols")]
    pub cols: u32,
    /// Grid height in rows. Defaults to 9.
    #[serde(default = "default_grid_rows")]
    pub rows: u32,
}

fn default_grid_cols() -> u32 {
    9
}
fn default_grid_rows() -> u32 {
    9
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum GraphOp {
    NodePlace {
        position: GridPos,
        piece_id: String,
        #[serde(default)]
        inline_params: BTreeMap<String, Value>,
    },
    NodeMove {
        from: GridPos,
        to: GridPos,
    },
    NodeSwap {
        a: GridPos,
        b: GridPos,
    },
    NodeRemove {
        position: GridPos,
    },
    EdgeConnect {
        #[serde(default)]
        edge_id: Option<EdgeId>,
        from: GridPos,
        to_node: GridPos,
        to_param: String,
    },
    EdgeDisconnect {
        edge_id: EdgeId,
    },
    ParamSetInline {
        position: GridPos,
        param_id: String,
        value: Value,
    },
    ParamClearInline {
        position: GridPos,
        param_id: String,
    },
    ParamSetSide {
        position: GridPos,
        param_id: String,
        side: TileSide,
    },
    ParamClearSide {
        position: GridPos,
        param_id: String,
    },
    OutputSetSide {
        position: GridPos,
        side: TileSide,
    },
    OutputClearSide {
        position: GridPos,
    },
    /// Set or clear a user-defined display label on a node.
    NodeSetLabel {
        position: GridPos,
        label: Option<String>,
    },
    /// Set opaque state data on a node (for stateful pieces).
    NodeSetState {
        position: GridPos,
        state: Option<Value>,
    },
    /// Resize the grid bounds.
    ResizeGrid {
        cols: u32,
        rows: u32,
    },
}

#[derive(Debug, Clone)]
pub struct GraphOpRecord {
    pub do_ops: Vec<GraphOp>,
    pub undo_ops: Vec<GraphOp>,
    pub removed_edges: Vec<Edge>,
}

mod grid_nodes_serde {
    use super::{GridPos, Node};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct NodeEntry {
        position: GridPos,
        node: Node,
    }

    pub fn serialize<S>(value: &BTreeMap<GridPos, Node>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries = value
            .iter()
            .map(|(position, node)| NodeEntry {
                position: position.clone(),
                node: node.clone(),
            })
            .collect::<Vec<_>>();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<GridPos, Node>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<NodeEntry>::deserialize(deserializer)?;
        let mut nodes = BTreeMap::new();
        for entry in entries {
            nodes.insert(entry.position, entry.node);
        }
        Ok(nodes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub schema_version: u32,
    pub name: String,
    pub graph: Graph,
}

impl ProjectDocument {
    pub const SCHEMA_VERSION: u32 = 2;

    pub fn new(name: String, graph: Graph) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            name,
            graph,
        }
    }
}
