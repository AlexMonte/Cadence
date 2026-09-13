//! Independent, UI-local filters for sound snapshots and imported recordings.
use crate::infrastructure::ui::theme::MusaicUiTheme;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};

#[derive(Component)]
struct NameFilter {
    prompt: String,
    query: String,
    selected: bool,
}
#[derive(Component)]
struct FilterCount(Entity);
#[derive(Component)]
pub(super) struct FilterList(pub Entity);
#[derive(Component)]
pub(super) struct FilterItem {
    field: Entity,
    name: String,
}
impl FilterItem {
    pub(super) fn new(field: Entity, name: &str) -> Self {
        Self {
            field,
            name: name.to_lowercase(),
        }
    }
}
pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, prompt: &str) -> Entity {
    let theme = MusaicUiTheme::default_dark();
    let field = parent
        .spawn((
            Node {
                width: percent(100),
                min_height: px(28),
                padding: UiRect::all(px(5)),
                border: UiRect::all(px(1)),
                flex_shrink: 0.0,
                ..default()
            },
            NameFilter {
                prompt: prompt.into(),
                query: String::new(),
                selected: false,
            },
            TabIndex(0),
            Interaction::default(),
            Text::new(prompt),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(theme.chrome.text_main),
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.button_border),
        ))
        .observe(focus_filter)
        .observe(type_filter)
        .id();
    parent.spawn((
        FilterCount(field),
        Text::new("All names · type to filter"),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        TextColor(theme.chrome.text_dim),
    ));
    field
}
fn focus_filter(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<&mut NameFilter>,
) {
    let Ok(mut field) = fields.get_mut(event.entity) else {
        return;
    };
    field.selected = true;
    focus.set(event.entity);
    event.propagate(false);
}
fn type_filter(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut NameFilter, &mut Text), Without<FilterCount>>,
    mut items: Query<(&FilterItem, &mut Node)>,
    mut lists: Query<(&FilterList, &mut ScrollPosition)>,
    mut counts: Query<(&FilterCount, &mut Text), Without<NameFilter>>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let Ok((mut field, mut text)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    let key = event.input.key_code;
    keyboard.clear_just_pressed(key);
    let control = keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight)
        || keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight);
    match key {
        KeyCode::Escape => {
            field.query.clear();
            field.selected = false;
            focus.clear();
        }
        KeyCode::Enter | KeyCode::NumpadEnter => {
            field.selected = false;
            focus.clear();
        }
        KeyCode::KeyA if control => field.selected = true,
        KeyCode::Backspace | KeyCode::Delete => {
            if field.selected || control {
                field.query.clear();
            } else {
                field.query.pop();
            }
            field.selected = false;
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
                if field.selected {
                    field.query.clear();
                    field.selected = false;
                }
                for c in value.chars().filter(|c| !c.is_control()) {
                    if field.query.len() + c.len_utf8() <= 96 {
                        field.query.push(c);
                    }
                }
            }
        }
        _ => {}
    }
    text.0 = if field.query.is_empty() {
        field.prompt.clone()
    } else {
        format!("Find: {}", field.query)
    };
    if focus.0 == Some(event.focused_entity) {
        text.0.push('│');
    }
    let mut matched = 0;
    let mut total = 0;
    for (item, mut node) in &mut items {
        if item.field != event.focused_entity {
            continue;
        }
        total += 1;
        let visible = matches(&field.query, &item.name);
        if visible {
            matched += 1;
        }
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (owner, mut scroll) in &mut lists {
        if owner.0 == event.focused_entity {
            scroll.y = 0.0;
        }
    }
    for (owner, mut count) in &mut counts {
        if owner.0 == event.focused_entity {
            count.0 = if matched == 0 {
                "No matching names".into()
            } else {
                format!("{matched} of {total} names")
            };
        }
    }
    event.propagate(false);
}
fn matches(query: &str, name: &str) -> bool {
    let name = name.to_lowercase();
    query
        .split_whitespace()
        .all(|term| name.contains(&term.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(app: &mut App, window: Entity, key_code: KeyCode, text: Option<&str>) {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: text
                .map(|value| Key::Character(value.into()))
                .unwrap_or(Key::Escape),
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window,
        });
        app.update();
    }
    #[test]
    fn focused_filter_matches_all_terms_keeps_other_lists_and_consumes_editor_shortcuts() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ));
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let samples = app
            .world_mut()
            .spawn((
                NameFilter {
                    prompt: "Search samples".into(),
                    query: String::new(),
                    selected: false,
                },
                Text::default(),
            ))
            .observe(type_filter)
            .id();
        let presets = app
            .world_mut()
            .spawn((
                NameFilter {
                    prompt: "Search sounds".into(),
                    query: String::new(),
                    selected: false,
                },
                Text::default(),
            ))
            .observe(type_filter)
            .id();
        let list = app
            .world_mut()
            .spawn((FilterList(samples), ScrollPosition(Vec2::new(0.0, 70.0))))
            .id();
        let piano = app
            .world_mut()
            .spawn((FilterItem::new(samples, "Soft Piano.wav"), Node::default()))
            .id();
        let kick = app
            .world_mut()
            .spawn((FilterItem::new(samples, "Hard Kick.wav"), Node::default()))
            .id();
        let bass = app
            .world_mut()
            .spawn((FilterItem::new(presets, "Warm bass"), Node::default()))
            .id();
        let count = app
            .world_mut()
            .spawn((FilterCount(samples), Text::default()))
            .id();
        app.world_mut().resource_mut::<InputFocus>().set(samples);
        key(&mut app, window, KeyCode::KeyP, Some("PI soft"));
        assert_eq!(
            app.world().get::<Node>(piano).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(kick).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(bass).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world()
                .get::<NameFilter>(presets)
                .unwrap()
                .query
                .is_empty()
        );
        assert_eq!(app.world().get::<Text>(count).unwrap().0, "1 of 2 names");
        assert_eq!(app.world().get::<ScrollPosition>(list).unwrap().y, 0.0);
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyP)
        );
        key(&mut app, window, KeyCode::Space, Some(" "));
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Space)
        );
        key(&mut app, window, KeyCode::Delete, None);
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Delete)
        );
        key(&mut app, window, KeyCode::Escape, None);
        assert!(app.world().resource::<InputFocus>().0.is_none());
        assert_eq!(
            app.world().get::<Node>(kick).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world()
                .get::<NameFilter>(samples)
                .unwrap()
                .query
                .is_empty()
        );
    }
}
