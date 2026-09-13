//! Full-width unsigned seed entry, separate from bounded rational operands.
use super::*;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, tab_navigation::TabIndex},
};

#[derive(Component)]
pub(crate) struct SeedInput {
    pub(super) node: NodeId,
    value: u64,
    draft: String,
    selected: bool,
}
pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, node: &NodeId, value: u64) {
    let theme = MusaicUiTheme::default_dark();
    parent
        .spawn((
            Node {
                min_height: px(28),
                width: percent(100),
                padding: UiRect::axes(px(7), px(4)),
                border: UiRect::all(px(1)),
                ..default()
            },
            TabIndex(0),
            Text::new(format!("Seed: {value}")),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(theme.chrome.text_main),
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.button_border),
            SeedInput {
                node: node.clone(),
                value,
                draft: value.to_string(),
                selected: false,
            },
        ))
        .observe(focus)
        .observe(type_seed);
}
fn focus(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut SeedInput, &mut Text)>,
) {
    if let Ok((mut input, mut text)) = fields.get_mut(event.entity) {
        input.selected = true;
        text.0 = format!("Seed: [{}]", input.draft);
        focus.set(event.entity);
        event.propagate(false);
    }
}
pub(super) fn update(field: &mut SeedInput, text: &mut Text, value: u64, focused: bool) {
    field.value = value;
    if !focused {
        field.draft = value.to_string();
        text.0 = format!("Seed: {value}");
    }
}
fn type_seed(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut SeedInput, &mut Text)>,
    project: Res<MusaicProject>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let Ok((mut input, mut text)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    let key = event.input.key_code;
    keyboard.clear_just_pressed(key);
    let control = [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]
    .into_iter()
    .any(|key| keyboard.pressed(key));
    match key {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            let result = input
                .draft
                .trim()
                .parse::<u64>()
                .map_err(|_| "Use a whole seed from 0 to 18446744073709551615")
                .and_then(|seed| {
                    let (parameter, mut value) = current(&project, &input.node)
                        .ok_or("This modulation tile is no longer available")?;
                    value.seed = seed;
                    command(&input.node, parameter, value)
                });
            match result {
                Ok(command) => {
                    bus.write(EditorCommandBus(command));
                    input.selected = false;
                    focus.clear();
                }
                Err(message) => {
                    text.0 = message.into();
                    event.propagate(false);
                    return;
                }
            }
        }
        KeyCode::Escape => {
            input.draft = input.value.to_string();
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
                for character in value.chars().filter(|c| !c.is_control()) {
                    if input.draft.len() + character.len_utf8() <= 64 {
                        input.draft.push(character);
                    }
                }
            }
        }
        _ => {}
    }
    text.0 = format!(
        "Seed: {}{}",
        input.draft,
        if focus.0 == Some(event.focused_entity) {
            " ▏"
        } else {
            ""
        }
    );
    event.propagate(false);
}
