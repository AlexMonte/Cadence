//! Editor UX overlays and chrome that sit above canvas semantics.

use bevy::prelude::*;

pub mod menubar;
pub mod node_chrome;
pub mod pattern_node;

pub use menubar::{BreadcrumbsLabel, EditorMenubar, EditorStatusBar, EditorUiRoot, StatusLabel};
pub use node_chrome::{HoveredNode, NodeChrome, NodeCloseButton};

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((menubar::plugin, node_chrome::plugin, pattern_node::plugin));
}
