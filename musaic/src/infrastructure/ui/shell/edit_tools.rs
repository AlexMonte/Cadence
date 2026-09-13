//! Always-visible structural editing commands; the command dispatcher owns edits.
use bevy::prelude::*;
use crate::{application::command::{EditorCommand, editing::TileEdit}, domain::document::ContainerKind, infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated, theme::MusaicUiTheme, widgets::musaic_button}};

pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    parent.spawn(Node {
        width: percent(100), min_height: px(34), flex_shrink: 0.0,
        flex_wrap: FlexWrap::Wrap, column_gap: px(4), row_gap: px(4),
        padding: UiRect::axes(px(16), px(3)), align_items: AlignItems::Center,
        ..default()
    }).with_children(|row| {
        for (label, action) in [
            ("Copy", TileEdit::Copy), ("Paste", TileEdit::Paste),
            ("Duplicate", TileEdit::Duplicate), ("Move here", TileEdit::MoveHere),
            ("Group sequence", TileEdit::Group(ContainerKind::Sequence)),
            ("Group layer", TileEdit::Group(ContainerKind::Parallel)),
            ("Group arrangement", TileEdit::Group(ContainerKind::Arrangement)),
        ] {
            row.spawn(musaic_button(Node {
                height: px(26), padding: UiRect::axes(px(8), px(3)),
                border: UiRect::all(px(1)), border_radius: BorderRadius::all(px(3)),
                align_items: AlignItems::Center, justify_content: JustifyContent::Center,
                ..default()
            }, (BackgroundColor(theme.chrome.button_bg), BorderColor::all(theme.chrome.button_border), InspectorButtonAction(EditorCommand::EditTiles(action))), label))
                .observe(on_inspector_button_activated);
        }
        row.spawn((Text::new("Select tiles · click an empty destination to move or paste"), TextFont { font_size: 10.0, ..default() }, TextColor(theme.chrome.text_main)));
    });
}
