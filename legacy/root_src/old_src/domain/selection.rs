use serde::{Deserialize, Serialize};

use tessera::types::{EdgeId, GridPos};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelectionState {
    pub selected_nodes: Vec<GridPos>,
    pub selected_edge: Option<EdgeId>,
    pub primary_node: Option<GridPos>,
}
