//! Ergonomic selection facade over canonical [`SelectionState`](super::types::SelectionState).

use tessera::prelude::NodeId;

use super::logic::apply_selection;
use super::types::{SelectionMode, SelectionState};

impl SelectionState {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.anchor = None;
    }

    pub fn contains(&self, node: NodeId) -> bool {
        self.nodes.contains(&node)
    }

    pub fn select(&mut self, node: NodeId, mode: SelectionMode) {
        apply_selection(self, node, mode);
    }

    pub fn retain_existing<F>(&mut self, mut exists: F)
    where
        F: FnMut(&NodeId) -> bool,
    {
        self.nodes.retain(|node| exists(node));
        if let Some(anchor) = &self.anchor {
            if !self.nodes.contains(anchor) {
                self.anchor = self.nodes.iter().next().cloned();
            }
        }
    }
}
