//! A symbolic Bank modifier can choose any imported label or accept a typed label.
use super::*;
use crate::{
    domain::document::AtomValue,
    infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated},
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
};

#[derive(Component)]
pub(crate) struct BankChoices {
    node: NodeId,
    painted: Option<Vec<String>>,
}
#[derive(Component)]
struct BankLabelInput {
    node: NodeId,
    original: String,
    draft: String,
    selected: bool,
}

fn command(node: &NodeId, label: String) -> EditorCommand {
    EditorCommand::SetAtomValue {
        node: node.clone(),
        value: AtomValue::Modifier(tessera::prelude::AtomModifier::SampleBank(label)),
    }
}

pub(crate) fn spawn_bank_tile(parent: &mut ChildSpawnerCommands<'_>, node: &NodeId, value: &str) {
    label(parent, "Bank label · choose an imported label or type one");
    let theme = MusaicUiTheme::default_dark();
    parent
        .spawn((
            Node {
                min_height: px(28),
                width: percent(100),
                padding: UiRect::all(px(5)),
                border: UiRect::all(px(1)),
                ..default()
            },
            Text::new(value),
            TextFont {
                font_size: 13.0,
                ..default()
            },
            TextColor(theme.chrome.text_main),
            ThemeFontColor(TEXT_MAIN),
            ThemedText,
            TabIndex(0),
            Interaction::default(),
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.button_border),
            BankLabelInput {
                node: node.clone(),
                original: value.into(),
                draft: value.into(),
                selected: true,
            },
        ))
        .observe(focus)
        .observe(type_label);
    parent.spawn((
        Node {
            width: percent(100),
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(4),
            row_gap: px(4),
            ..default()
        },
        BankChoices {
            node: node.clone(),
            painted: None,
        },
    ));
    label(
        parent,
        "Enter saves · Esc cancels. Create labels in an imported sample bank's variant settings.",
    );
}

pub(crate) fn sync_bank_tile_choices(
    projection: Res<EditorUiProjection>,
    mut choices: Query<(Entity, &mut BankChoices)>,
    mut commands: Commands,
) {
    for (entity, mut choice) in &mut choices {
        if choice.painted.as_ref() == Some(&projection.sample_bank_labels) {
            continue;
        }
        choice.painted = Some(projection.sample_bank_labels.clone());
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|parent| {
                for name in &projection.sample_bank_labels {
                    parent
                        .spawn(musaic_button(
                            Node {
                                min_height: px(26),
                                padding: UiRect::axes(px(6), px(3)),
                                ..default()
                            },
                            InspectorButtonAction(command(&choice.node, name.clone())),
                            name.clone(),
                        ))
                        .observe(on_inspector_button_activated);
                }
                if projection.sample_bank_labels.is_empty() {
                    label(parent, "No imported bank labels yet.");
                }
            });
    }
}

fn focus(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut BankLabelInput, &mut Text)>,
) {
    let Ok((mut input, mut text)) = fields.get_mut(event.entity) else {
        return;
    };
    input.selected = true;
    focus.set(event.entity);
    text.0 = format!("[{}]", input.draft);
    event.propagate(false);
}

fn type_label(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut BankLabelInput, &mut Text, &mut BorderColor)>,
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
            if input.draft.trim().is_empty() {
                text.0 = "Enter a bank label".into();
                *border = BorderColor::all(Color::srgb(0.7, 0.2, 0.18));
                event.propagate(false);
                return;
            }
            bus.write(EditorCommandBus(command(
                &input.node,
                input.draft.trim().into(),
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
            if let Some(value) =
                event
                    .input
                    .text
                    .as_deref()
                    .or_else(|| match &event.input.logical_key {
                        Key::Character(value) => Some(value.as_str()),
                        _ => None,
                    })
            {
                if input.selected {
                    input.draft.clear();
                    input.selected = false;
                }
                for character in value.chars().filter(|character| !character.is_control()) {
                    if input.draft.len() + character.len_utf8() <= 64 {
                        input.draft.push(character);
                    }
                }
            }
        }
        _ => {}
    }
    text.0 = format!(
        "{}{}",
        input.draft,
        if focus.0 == Some(event.focused_entity) {
            "│"
        } else {
            ""
        }
    );
    *border = BorderColor::all(MusaicUiTheme::default_dark().chrome.button_border);
    event.propagate(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_bank_choices_emit_the_owned_bank_tile_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<EditorCommandBus>()
            .insert_resource(EditorUiProjection {
                sample_bank_labels: vec!["bright".into(), "soft".into()],
                ..default()
            })
            .add_systems(Update, sync_bank_tile_choices);
        app.world_mut().spawn((
            Node::default(),
            BankChoices {
                node: NodeId::new("bank-tile"),
                painted: None,
            },
        ));
        app.update();
        let button = app
            .world_mut()
            .query::<(Entity, &InspectorButtonAction)>()
            .iter(app.world())
            .find(|(_, action)| action.0 == command(&NodeId::new("bank-tile"), "soft".into()))
            .unwrap()
            .0;
        let mut cursor = app
            .world()
            .resource::<Messages<EditorCommandBus>>()
            .get_cursor_current();
        app.world_mut().trigger(Activate { entity: button });
        let messages = cursor
            .read(app.world().resource::<Messages<EditorCommandBus>>())
            .map(|message| message.0.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            [command(&NodeId::new("bank-tile"), "soft".into())]
        );
        app.world_mut()
            .resource_mut::<EditorUiProjection>()
            .sample_bank_labels = vec!["new".into()];
        app.update();
        assert!(
            app.world().get_entity(button).is_err(),
            "obsolete label choices must be removed"
        );
    }

    #[test]
    fn typing_a_bank_name_consumes_shortcuts_and_commits_on_enter() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ))
        .add_message::<EditorCommandBus>();
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let field = app
            .world_mut()
            .spawn((
                BankLabelInput {
                    node: NodeId::new("bank"),
                    original: "old".into(),
                    draft: "old".into(),
                    selected: true,
                },
                Text::default(),
                BorderColor::default(),
            ))
            .observe(type_label)
            .id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyB,
            logical_key: Key::Character("bright".into()),
            state: ButtonState::Pressed,
            text: Some("bright".into()),
            repeat: false,
            window,
        });
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyB)
        );
        let mut cursor = app
            .world()
            .resource::<Messages<EditorCommandBus>>()
            .get_cursor_current();
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Enter,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        app.update();
        let messages = cursor
            .read(app.world().resource::<Messages<EditorCommandBus>>())
            .map(|message| message.0.clone())
            .collect::<Vec<_>>();
        assert_eq!(messages, [command(&NodeId::new("bank"), "bright".into())]);
        assert_eq!(app.world().resource::<InputFocus>().0, None);
    }
}
