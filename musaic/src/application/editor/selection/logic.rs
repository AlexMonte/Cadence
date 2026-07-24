use tessera::prelude::NodeId;

use super::types::{SelectionMode, SelectionState};

pub fn apply_selection(state: &mut SelectionState, node: NodeId, mode: SelectionMode) {
    match mode {
        SelectionMode::Replace => {
            state.nodes.clear();
            state.nodes.insert(node.clone());
            state.anchor = Some(node);
        }
        SelectionMode::Add => {
            state.nodes.insert(node.clone());
            state.anchor = Some(node);
        }
        SelectionMode::Toggle => {
            if state.nodes.remove(&node) {
                if state.anchor.as_ref() == Some(&node) {
                    state.anchor = state.nodes.iter().next().cloned();
                }
            } else {
                state.nodes.insert(node.clone());
                state.anchor = Some(node);
            }
        }
    }
}
