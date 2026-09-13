//! Multi-selection summary panel.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

pub(crate) fn spawn_selection_summary_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    count: usize,
    primary: Option<&str>,
) {
    panel.spawn((UiText::new(format!("{count} tiles selected")), ThemedText));
    if let Some(primary) = primary {
        panel.spawn((UiText::new(format!("Primary: {primary}")), ThemedText));
    }
}
