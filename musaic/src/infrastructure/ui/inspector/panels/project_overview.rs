//! Project overview panel: shown when nothing specific is focused.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use crate::domain::board::BoardSurfaceId;

pub(crate) fn spawn_project_overview_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
) {
    panel.spawn((UiText::new("Project Workspace"), ThemedText));
    panel.spawn((
        UiText::new(format!("Surface {} is active.", surface.0)),
        ThemedText,
    ));
}
