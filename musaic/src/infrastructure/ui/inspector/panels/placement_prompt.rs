//! Placement prompt panel: empty slot / stack insert focused with the drawer closed.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use crate::application::editor::TileLibraryContextKind;

pub(crate) fn spawn_placement_prompt_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    target_label: Option<&str>,
    context: TileLibraryContextKind,
) {
    panel.spawn((
        UiText::new("Press D or Tiles to open the drawer."),
        ThemedText,
    ));
    if let Some(label) = target_label {
        panel.spawn((UiText::new(label.to_string()), ThemedText));
    }
    panel.spawn((UiText::new(placement_hint(context)), ThemedText));
}

fn placement_hint(context: TileLibraryContextKind) -> &'static str {
    match context {
        TileLibraryContextKind::RootBoard => "Opens the macro tile drawer.",
        TileLibraryContextKind::ContainerBody => "Opens the atom tile drawer.",
    }
}
