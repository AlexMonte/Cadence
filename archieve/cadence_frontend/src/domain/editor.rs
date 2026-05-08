use std::collections::BTreeMap;

use serde_json::Value;

use crate::adapter::{GridPos, backend::PatternSurface};

pub const CELL_W: i32 = 64;
pub const CELL_H: i32 = 64;
pub const GRID_ORIGIN_X: i32 = 16;
pub const GRID_ORIGIN_Y: i32 = 16;
pub const NODE_W: i32 = 60;
pub const NODE_H: i32 = 60;
pub const DEFAULT_GRID_COLS: u32 = 24;
pub const DEFAULT_GRID_ROWS: u32 = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct GraphNodeView {
    pub position: GridPos,
    pub piece_id: String,
    pub inline_params: BTreeMap<String, Value>,
    pub pattern_source: Option<PatternSurface>,
    pub input_sides: BTreeMap<String, String>,
    pub output_side: Option<String>,
    pub label: Option<String>,
    pub node_state: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphEdgeView {
    pub id: String,
    pub from: GridPos,
    pub to_node: GridPos,
    pub to_param: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphView {
    pub name: String,
    pub cols: u32,
    pub rows: u32,
    pub nodes: Vec<GraphNodeView>,
    pub edges: Vec<GraphEdgeView>,
}

pub fn first_free_position(graph: &GraphView) -> Option<GridPos> {
    for row in 0..graph.rows as i32 {
        for col in 0..graph.cols as i32 {
            let position = GridPos { col, row };
            if graph.nodes.iter().all(|node| node.position != position) {
                return Some(position);
            }
        }
    }
    None
}

pub fn is_cell_occupied(graph: &GraphView, position: &GridPos) -> bool {
    graph.nodes.iter().any(|node| node.position == *position)
}

pub fn remap_edges_for_position(edges: &mut [GraphEdgeView], from: &GridPos, to: &GridPos) {
    for edge in edges {
        if edge.from == *from {
            edge.from = *to;
        }
        if edge.to_node == *from {
            edge.to_node = *to;
        }
    }
}

pub fn swap_edges_for_positions(edges: &mut [GraphEdgeView], a: &GridPos, b: &GridPos) {
    for edge in edges {
        if edge.from == *a {
            edge.from = *b;
        } else if edge.from == *b {
            edge.from = *a;
        }
        if edge.to_node == *a {
            edge.to_node = *b;
        } else if edge.to_node == *b {
            edge.to_node = *a;
        }
    }
}

pub fn canonical_side(side: &str) -> &'static str {
    match side {
        "east" | "right" => "right",
        "west" | "left" => "left",
        "north" | "top" => "top",
        "south" | "bottom" => "bottom",
        _ => "right",
    }
}
