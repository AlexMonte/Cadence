use std::collections::BTreeSet;

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionState {
    pub nodes: BTreeSet<NodeId>,
    pub anchor: Option<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    Replace,
    Add,
    Toggle,
}
