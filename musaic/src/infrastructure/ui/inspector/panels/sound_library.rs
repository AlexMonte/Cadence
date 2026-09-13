//! Project-owned sound snapshots with focused name editing and command-based use.
use super::sound::{choice_row, label};
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus, sound_library::SoundLibraryCommand},
        pipeline::sound::SoundPaint,
    },
    infrastructure::ui::{
        InspectorButtonAction, on_inspector_button_activated, theme::MusaicUiTheme,
        widgets::musaic_button,
    },
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};
use bevy_feathers::theme::ThemedText;
use bevy_ui_widgets::Activate;
use tessera::prelude::NodeId;

fn row() -> Node {
    Node {
        width: percent(100),
        flex_wrap: FlexWrap::Wrap,
        column_gap: px(4),
        row_gap: px(4),
        align_items: AlignItems::Center,
        ..default()
    }
}
#[derive(Clone)]
enum NameTarget {
    NewSound(NodeId),
    Existing(String),
}

#[derive(Component)]
struct SoundNameInput {
    target: NameTarget,
    original: String,
    draft: String,
    selected: bool,
}

impl SoundNameInput {
    fn command(&self) -> SoundLibraryCommand {
        match &self.target {
            NameTarget::NewSound(sound) => SoundLibraryCommand::Save {
                sound: sound.clone(),
                name: self.draft.clone(),
            },
            NameTarget::Existing(name) => SoundLibraryCommand::Rename {
                name: name.clone(),
                new_name: self.draft.clone(),
            },
        }
    }
}

#[derive(Component)]
struct SaveSoundName(Entity);

fn name_editor(
    parent: &mut ChildSpawnerCommands<'_>,
    target: NameTarget,
    name: &str,
    create: bool,
) {
    parent.spawn(row()).with_children(|row| {
        let theme = MusaicUiTheme::default_dark();
        let field = row
            .spawn((
                Node {
                    min_width: px(120),
                    max_width: percent(100),
                    flex_grow: 1.0,
                    min_height: px(28),
                    padding: UiRect::all(px(5)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                TabIndex(0),
                Interaction::default(),
                BackgroundColor(theme.chrome.panel_bg),
                BorderColor::all(theme.chrome.button_border),
                Text::new(format!("Name: {name}")),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                ThemedText,
                TextColor(MusaicUiTheme::default_dark().chrome.text_main),
                bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
                SoundNameInput {
                    target,
                    original: name.to_owned(),
                    draft: name.to_owned(),
                    selected: true,
                },
            ))
            .observe(focus_name)
            .observe(type_name)
            .id();
        row.spawn(musaic_button(
            Node {
                min_height: px(28),
                padding: UiRect::axes(px(6), px(4)),
                ..default()
            },
            (
                SaveSoundName(field),
                BackgroundColor(theme.chrome.button_bg),
            ),
            if create { "Save sound" } else { "Rename" },
        ))
        .observe(save_name);
    });
    label(
        parent,
        "Click name to type · Enter saves · Esc cancels",
        11.0,
    );
}

fn focus_name(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut SoundNameInput, &mut Text, &mut BorderColor)>,
) {
    let Ok((mut input, mut text, mut border)) = fields.get_mut(event.entity) else {
        return;
    };
    focus.set(event.entity);
    input.selected = true;
    text.0 = format!("Name: [{}]", input.draft);
    *border = BorderColor::all(Color::srgb(0.16, 0.42, 0.31));
    event.propagate(false);
}

/// Focus dispatch runs in PreUpdate. Consuming just-pressed here prevents global
/// editor shortcuts from deleting tiles or starting playback while naming.
fn type_name(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut SoundNameInput, &mut Text, &mut BorderColor)>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let Ok((mut input, mut text, mut border)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    let key = event.input.key_code;
    keyboard.clear_just_pressed(key);
    let control = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight);
    match key {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            bus.write(EditorCommandBus(EditorCommand::SoundLibrary(
                input.command(),
            )));
            input.selected = false;
            focus.clear();
        }
        KeyCode::Escape => {
            input.draft = input.original.clone();
            input.selected = false;
            focus.clear();
        }
        KeyCode::KeyA if control => input.selected = true,
        KeyCode::Backspace | KeyCode::Delete => {
            if input.selected {
                input.draft.clear();
            } else {
                input.draft.pop();
            }
            input.selected = false;
        }
        _ if !control => {
            let value = event
                .input
                .text
                .as_deref()
                .or_else(|| match &event.input.logical_key {
                    Key::Character(value) => Some(value.as_str()),
                    _ => None,
                });
            if let Some(value) = value {
                if input.selected {
                    input.draft.clear();
                    input.selected = false;
                }
                for character in value.chars().filter(|character| !character.is_control()) {
                    if input.draft.len() + character.len_utf8() <= 128 {
                        input.draft.push(character);
                    }
                }
            }
        }
        _ => {}
    }
    let editing = focus.0 == Some(event.focused_entity);
    text.0 = if input.selected {
        format!("Name: [{}]", input.draft)
    } else if editing {
        format!("Name: {}│", input.draft)
    } else {
        format!("Name: {}", input.draft)
    };
    if !editing {
        *border = BorderColor::all(MusaicUiTheme::default_dark().chrome.button_border);
    }
    event.propagate(false);
}

fn save_name(
    event: On<Activate>,
    buttons: Query<&SaveSoundName>,
    fields: Query<&SoundNameInput>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let Ok(input) = fields.get(button.0) else {
        return;
    };
    bus.write(EditorCommandBus(EditorCommand::SoundLibrary(
        input.command(),
    )));
}

pub(super) fn spawn_library(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    sound: &SoundPaint,
) {
    label(parent, "Saved sounds", 14.0);
    let mut index = 1;
    while sound
        .presets
        .iter()
        .any(|(name, _)| name == &format!("Sound {index}"))
    {
        index += 1;
    }
    name_editor(
        parent,
        NameTarget::NewSound(output.clone()),
        &format!("Sound {index}"),
        true,
    );
    if sound.presets.is_empty() {
        return;
    }
    label(parent, "Snapshots · edits to a Sound tile stay local", 10.0);
    let filter = super::name_filter::spawn(parent, "Search saved sounds…");
    parent
        .spawn((
            Node {
                max_height: px(220),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            super::name_filter::FilterList(filter),
        ))
        .observe(super::sound::on_sample_list_scroll)
        .with_children(|list| {
            for (name, instrument) in &sound.presets {
                list.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    super::name_filter::FilterItem::new(filter, name),
                ))
                .with_children(|list| {
                    name_editor(list, NameTarget::Existing(name.clone()), name, false);
                    choice_row(list, "", |row| {
                        for (title, command) in [
                            (
                                "Apply",
                                EditorCommand::SetSound {
                                    sound: output.clone(),
                                    definition: instrument.clone(),
                                },
                            ),
                            (
                                "Delete",
                                EditorCommand::SoundLibrary(SoundLibraryCommand::Delete {
                                    name: name.clone(),
                                }),
                            ),
                        ] {
                            row.spawn(musaic_button(
                                Node {
                                    min_height: px(26),
                                    padding: UiRect::horizontal(px(8)),
                                    ..default()
                                },
                                InspectorButtonAction(command),
                                title,
                            ))
                            .observe(on_inspector_button_activated);
                        }
                    });
                });
            }
        });
}
