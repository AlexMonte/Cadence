//! Timeline event panel: focused projected event with jump-to-source action.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::{
    controls::{ButtonProps, button},
    theme::ThemedText,
};
use bevy_ui_widgets::observe;

use crate::application::command::EditorCommand;
use crate::application::pipeline::runtime::{ProjectedEventId, RuntimePreviewSnapshot};

use crate::infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated};

pub(crate) fn spawn_timeline_event_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    event: ProjectedEventId,
    preview_snapshot: &RuntimePreviewSnapshot,
) {
    panel.spawn((UiText::new(format!("Event {}", event.0)), ThemedText));
    if let Some(record) = preview_snapshot.event(event) {
        panel.spawn((UiText::new(record.label.clone()), ThemedText));
    }
    panel.spawn((
        button(
            ButtonProps::default(),
            InspectorButtonAction(EditorCommand::JumpToTimelineSource { event }),
            Spawn((UiText::new("Jump To Source"), ThemedText)),
        ),
        observe(on_inspector_button_activated),
    ));
}
