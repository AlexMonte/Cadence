//! Multi-selection summary panel.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;
use tessera::prelude::NodeId;

pub(crate) fn spawn_selection_summary_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    count: usize,
    primary: Option<&NodeId>,
) {
    panel.spawn((UiText::new(format!("{count} tiles selected")), ThemedText));
    if let Some(primary) = primary {
        panel.spawn((UiText::new(format!("Primary: {}", primary.0)), ThemedText));
    }
}
