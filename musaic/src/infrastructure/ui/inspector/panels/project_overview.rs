//! Project overview panel: shown when nothing specific is focused.

use bevy::prelude::*;

use crate::domain::board::BoardSurfaceId;

pub(crate) fn spawn_project_overview_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    _surface: BoardSurfaceId,
    channels: u16,
) {
    super::tile_presentation::inspector_label(panel, &format!("Channels: {channels}"), 15.0, false);
    super::tile_presentation::inspector_label(
        panel,
        "Maximum output tiles in this project",
        12.0,
        true,
    );
    for (label, value) in [
        ("Fewer channels", channels.saturating_sub(1).max(1)),
        ("More channels", channels.saturating_add(1)),
    ] {
        panel
            .spawn(crate::infrastructure::ui::widgets::musaic_button(
                Node::default(),
                ChannelCount(value),
                label,
            ))
            .observe(change_channels);
    }
    super::tile_presentation::inspector_label(panel, "Compose with tiles", 18.0, false);
    super::tile_presentation::inspector_label(
        panel,
        "Select a tile to see its notes, modifiers and connections.",
        12.0,
        true,
    );
    super::tile_presentation::inspector_label(
        panel,
        "Open View → Tile library to add a note or pattern. Select a container again to open its contents.",
        12.0,
        true,
    );
}

#[derive(Component)]
struct ChannelCount(u16);
fn change_channels(
    event: On<Pointer<Click>>,
    buttons: Query<&ChannelCount>,
    mut bus: MessageWriter<crate::application::command::EditorCommandBus>,
) {
    use crate::application::command::{EditorCommand, EditorCommandBus, editing::TileEdit};
    if let Ok(button) = buttons.get(event.entity) {
        bus.write(EditorCommandBus(EditorCommand::EditTiles(
            TileEdit::SetChannels { channels: button.0 },
        )));
    }
}
