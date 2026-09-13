//! Timeline event panel: focused projected event with jump-to-source action.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use crate::application::command::EditorCommand;
use crate::application::pipeline::runtime::ProjectedEventId;

use crate::infrastructure::ui::widgets::musaic_button;
use crate::infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated};

pub(crate) fn spawn_timeline_event_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    event: ProjectedEventId,
    label: &str,
) {
    panel.spawn((UiText::new(format!("Event {}", event.0)), ThemedText));
    if !label.is_empty() {
        panel.spawn((UiText::new(label.to_string()), ThemedText));
    }
    panel
        .spawn(musaic_button(
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            InspectorButtonAction(EditorCommand::JumpToTimelineSource { event }),
            "Jump To Source",
        ))
        .observe(on_inspector_button_activated);
}
