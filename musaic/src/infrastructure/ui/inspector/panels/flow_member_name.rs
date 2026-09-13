//! Focused endpoint naming; Enter commits one atomic rename through history.
use crate::{
    application::command::{EditorCommand, EditorCommandBus, editing::TileEdit, flow::FlowEdit},
    infrastructure::ui::theme::MusaicUiTheme,
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
use tessera::prelude::{NodeId, PortGroupId, PortMemberId};
#[derive(Component)]
struct MemberName {
    node: NodeId,
    group: PortGroupId,
    member: PortMemberId,
    input: bool,
    draft: String,
    selected: bool,
}
pub(super) fn spawn(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    group: &PortGroupId,
    member: &PortMemberId,
    input: bool,
) {
    let theme = MusaicUiTheme::default_dark();
    parent
        .spawn((
            Node {
                width: percent(100),
                max_width: percent(100),
                min_height: px(27),
                padding: UiRect::axes(px(6), px(4)),
                border: UiRect::all(px(1)),
                flex_shrink: 0.,
                ..default()
            },
            TabIndex(0),
            Interaction::default(),
            Text::new(format!("Name: {}", member.0)),
            TextFont {
                font_size: 12.,
                ..default()
            },
            TextColor(theme.chrome.text_main),
            ThemedText,
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.button_border),
            MemberName {
                node: node.clone(),
                group: group.clone(),
                member: member.clone(),
                input,
                draft: member.0.clone(),
                selected: false,
            },
        ))
        .observe(focus)
        .observe(type_name);
}
fn focus(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut MemberName, &mut Text)>,
) {
    let Ok((mut field, mut text)) = fields.get_mut(event.entity) else {
        return;
    };
    field.selected = true;
    text.0 = format!("Name: [{}] · Enter to save", field.draft);
    focus.set(event.entity);
    event.propagate(false);
}
fn type_name(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut MemberName, &mut Text)>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let Ok((mut field, mut text)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    keyboard.clear_just_pressed(event.input.key_code);
    let control = [
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ]
    .into_iter()
    .any(|key| keyboard.pressed(key));
    match event.input.key_code {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            bus.write(EditorCommandBus(EditorCommand::EditTiles(TileEdit::Flow {
                node: field.node.clone(),
                edit: FlowEdit::RenameMember {
                    group: field.group.clone(),
                    member: field.member.clone(),
                    name: field.draft.clone(),
                    input: field.input,
                },
            })));
            field.selected = false;
            focus.clear();
        }
        KeyCode::Escape => {
            field.draft = field.member.0.clone();
            field.selected = false;
            focus.clear();
        }
        KeyCode::KeyA if control => field.selected = true,
        KeyCode::Backspace | KeyCode::Delete => {
            if field.selected {
                field.draft.clear();
            } else {
                field.draft.pop();
            }
            field.selected = false;
        }
        _ if !control => {
            let value = event.input.text.as_deref().map(str::to_owned).or_else(|| {
                match &event.input.logical_key {
                    Key::Character(text) => Some(text.to_string()),
                    Key::Space => Some(" ".into()),
                    _ => None,
                }
            });
            if let Some(value) = value.filter(|value| !value.chars().any(char::is_control)) {
                let capacity = if field.selected { 0 } else { field.draft.len() };
                if capacity + value.len() <= 128 {
                    if field.selected {
                        field.draft.clear();
                    }
                    field.draft.push_str(&value);
                    field.selected = false;
                }
            }
        }
        _ => {}
    }
    text.0 = if field.selected {
        format!("Name: [{}]", field.draft)
    } else {
        format!("Name: {}", field.draft)
    };
    event.propagate(false);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(app: &mut App, window: Entity, key_code: KeyCode, text: Option<&str>) {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: text.map(|s| Key::Character(s.into())).unwrap_or(Key::Enter),
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window,
        });
        app.update();
    }
    #[test]
    fn focused_member_name_commits_exact_group_and_consumes_editor_shortcuts() {
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
                MemberName {
                    node: NodeId::new("route"),
                    group: PortGroupId::new("routes"),
                    member: PortMemberId::new("left"),
                    input: false,
                    draft: "left".into(),
                    selected: true,
                },
                Text::default(),
            ))
            .observe(type_name)
            .id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        key(&mut app, window, KeyCode::KeyG, Some("gain"));
        assert_eq!(app.world().get::<MemberName>(field).unwrap().draft, "gain");
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyG)
        );
        key(&mut app, window, KeyCode::Enter, None);
        let messages = app
            .world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].0,
            EditorCommand::EditTiles(TileEdit::Flow {
                node: NodeId::new("route"),
                edit: FlowEdit::RenameMember {
                    group: PortGroupId::new("routes"),
                    member: PortMemberId::new("left"),
                    name: "gain".into(),
                    input: false
                }
            })
        );
        assert!(app.world().resource::<InputFocus>().0.is_none());
    }
}
