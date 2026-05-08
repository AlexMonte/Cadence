use std::collections::BTreeSet;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use tessera::prelude::{ContainerId, NodeId};

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct EditorState {
    pub active_tool: EditorTool,
    pub focus: EditorFocus,
    pub selection: SelectionState,
    pub drag: Option<DragState>,
    pub inspector: InspectorState,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            active_tool: EditorTool::Select,
            focus: EditorFocus::RootSurface,
            selection: SelectionState::default(),
            drag: None,
            inspector: InspectorState::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorTool {
    Select,
    PlaceTile(TilePrototypeId),
    Connect,
    Pan,
    Erase,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TilePrototypeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorFocus {
    RootSurface,
    Container(ContainerId),
    Inspector,
    Palette,
    Transport,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelectionState {
    pub selected_nodes: BTreeSet<NodeId>,
    pub selected_containers: BTreeSet<ContainerId>,
    pub selected_atoms: BTreeSet<AtomSelectionId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DragState {
    pub pointer_world: Vec2Def,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InspectorState {
    pub target: Option<InspectorTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectorTarget {
    Node(NodeId),
    Container(ContainerId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AtomSelectionId {
    pub container: ContainerId,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vec2Def {
    pub x: f32,
    pub y: f32,
}

impl From<Vec2> for Vec2Def {
    fn from(value: Vec2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorState>();
    }
}
