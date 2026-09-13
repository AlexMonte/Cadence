//! UI-local tile discovery; typing here consumes editor shortcuts.
use crate::infrastructure::ui::theme::MusaicUiTheme;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};

#[derive(Resource, Default)]
pub struct TileLibrarySearch {
    pub query: String,
    pub hovered: String,
    pub collection: LibraryCollection,
    pub collection_error: String,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LibraryCollection {
    #[default]
    All,
    Recent,
    Favorites,
}
impl TileLibrarySearch {
    pub fn matches_item(&self, item: &crate::application::editor::TileDrawerItem) -> bool {
        self.matches(&format!(
            "{} {} {}",
            item.label,
            item.category().label(),
            item.category().description()
        ))
    }
    pub fn matches(&self, label: &str) -> bool {
        let label = label.to_lowercase();
        self.query
            .split_whitespace()
            .all(|word| label.contains(&word.to_lowercase()))
    }
}
#[derive(Component)]
pub(crate) struct SearchField;
#[derive(Component)]
pub(crate) struct SearchHint;
#[derive(Component)]
pub(crate) struct SearchText;
#[derive(Component)]
pub struct TileLibraryLabel(pub String);

#[derive(Component, Clone, Copy)]
pub(crate) struct LibraryPosition {
    pub body: Entity,
    pub order: usize,
    pub row: usize,
    pub column: usize,
}

pub(crate) fn spawn(parent: &mut ChildSpawnerCommands<'_>) {
    let theme = MusaicUiTheme::default_dark();
    let mut accessible = accesskit::Node::new(accesskit::Role::TextInput);
    accessible.set_label("Search tiles");
    accessible.set_description(
        "Filter by tile name or category. Arrow Down enters the results; Tab reaches collection controls. Arrow keys browse tiles. F toggles a favorite while a tile is focused.",
    );
    parent
        .spawn((
            Node {
                width: percent(100),
                min_height: px(30),
                padding: UiRect::all(px(7)),
                border: UiRect::all(px(1)),
                ..default()
            },
            SearchField,
            bevy::a11y::AccessibilityNode::from(accessible),
            TabIndex(0),
            Pickable::default(),
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.button_border),
        ))
        .observe(focus_search)
        .observe(type_search)
        .with_children(|field| {
            field.spawn((
                SearchText,
                Text::new("Search tiles…"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
                Pickable::IGNORE,
            ));
        });
    parent.spawn((
        SearchHint,
        Text::new("Scroll to browse categories"),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(theme.chrome.text_dim),
        Node {
            min_height: px(34),
            ..default()
        },
    ));
}
fn focus_search(mut event: On<Pointer<Press>>, mut focus: ResMut<InputFocus>) {
    focus.set(event.entity);
    event.propagate(false);
}
fn type_search(
    mut event: On<FocusedInput<KeyboardInput>>,
    fields: Query<(), With<SearchField>>,
    mut search: ResMut<TileLibrarySearch>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    results: Query<(Entity, &LibraryPosition)>,
    mut indices: Query<&mut TabIndex, With<crate::infrastructure::ui::DrawerTileSource>>,
) {
    if !fields.contains(event.focused_entity)
        || event.input.state != ButtonState::Pressed
        || event.input.key_code == KeyCode::Tab
    {
        return;
    }
    let key = event.input.key_code;
    keyboard.clear_just_pressed(key);
    search.hovered.clear();
    let control = keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight)
        || keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight);
    match key {
        KeyCode::ArrowDown => {
            if let Some((next, position)) = results.iter().min_by_key(|(_, p)| p.order) {
                for (entity, p) in &results {
                    if p.body == position.body {
                        if let Ok(mut index) = indices.get_mut(entity) {
                            index.0 = if entity == next { 0 } else { -1 };
                        }
                    }
                }
                focus.set(next);
            }
        }
        KeyCode::Escape => {
            search.query.clear();
            focus.clear();
        }
        KeyCode::Enter => focus.clear(),
        KeyCode::Backspace | KeyCode::Delete => {
            if control {
                search.query.clear();
            } else {
                search.query.pop();
            }
        }
        _ if !control => {
            let text = event
                .input
                .text
                .as_deref()
                .or_else(|| match &event.input.logical_key {
                    Key::Character(value) => Some(value.as_str()),
                    _ => None,
                });
            if let Some(text) = text {
                for c in text.chars().filter(|c| !c.is_control()) {
                    if search.query.len() + c.len_utf8() <= 96 {
                        search.query.push(c);
                    }
                }
            }
        }
        _ => {}
    }
    event.propagate(false);
}
pub(crate) fn hover(
    event: On<Pointer<Over>>,
    labels: Query<&TileLibraryLabel>,
    mut search: ResMut<TileLibrarySearch>,
) {
    if let Ok(label) = labels.get(event.entity) {
        search.hovered = label.0.clone();
    }
}
pub(crate) fn sync(
    search: Res<TileLibrarySearch>,
    projection: Res<crate::application::pipeline::ui_projection::EditorUiProjection>,
    focus: Res<InputFocus>,
    preferences: Res<crate::application::editor::preferences::EditorPreferences>,
    labels: Query<&TileLibraryLabel>,
    mut accessible: Query<&mut bevy::a11y::AccessibilityNode, With<SearchField>>,
    mut fields: Query<(&ChildOf, &mut Text), With<SearchText>>,
    mut hints: Query<&mut Text, (With<SearchHint>, Without<SearchText>)>,
) {
    for mut field in &mut accessible {
        if field.value() != Some(search.query.as_str()) {
            field.set_value(search.query.clone());
        }
    }
    for (parent, mut text) in &mut fields {
        let entity = parent.parent();
        let value = if search.query.is_empty() {
            "Search tiles…".to_string()
        } else {
            format!("Search: {}", search.query)
        };
        let value = if focus.0 == Some(entity) {
            format!("{value} |")
        } else {
            value
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    let visible_item = |item: &&crate::application::editor::TileDrawerItem| {
        let keys = match search.collection {
            LibraryCollection::All => None,
            LibraryCollection::Recent => Some(preferences.library.recent()),
            LibraryCollection::Favorites => Some(preferences.library.favorites()),
        };
        search.matches_item(item)
            && keys.is_none_or(|keys| keys.iter().any(|key| key.matches(item)))
    };
    let count = projection
        .palette
        .options
        .iter()
        .filter(visible_item)
        .count();
    let hovered_is_visible = projection
        .palette
        .options
        .iter()
        .any(|item| item.label == search.hovered && visible_item(&item));
    let focused_label = focus.0.and_then(|entity| labels.get(entity).ok());
    let hint = if !search.collection_error.is_empty() {
        search.collection_error.clone()
    } else if let Some(label) = focused_label {
        format!(
            "{} · F toggles favorite · Enter chooses · then Enter places on the board",
            label.0
        )
    } else if count == 0 {
        "No matching tiles".into()
    } else if !hovered_is_visible {
        format!(
            "{count} {} · Tab selects controls · arrows browse tiles",
            if count == 1 { "tile" } else { "tiles" }
        )
    } else {
        search.hovered.clone()
    };
    for mut text in &mut hints {
        if text.0 != hint {
            text.0 = hint.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filtering_matches_all_words_case_insensitively() {
        let search = TileLibrarySearch {
            query: "HIGH cutoff".into(),
            hovered: String::new(),
            ..default()
        };
        assert!(search.matches("High-pass cutoff"));
        assert!(!search.matches("High-pass resonance"));
        assert!(!search.matches("Low-pass cutoff"));
    }
    #[test]
    fn focused_search_does_not_trigger_drawer_or_delete_shortcuts() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ))
        .init_resource::<TileLibrarySearch>()
        .init_resource::<crate::application::editor::preferences::EditorPreferences>()
        .init_resource::<crate::application::pipeline::ui_projection::EditorUiProjection>()
        .add_systems(Update, sync);
        app.configure_sets(
            PreUpdate,
            bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
        );
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let field = app
            .world_mut()
            .spawn((
                SearchField,
                bevy::a11y::AccessibilityNode::from(accesskit::Node::new(
                    accesskit::Role::TextInput,
                )),
            ))
            .observe(type_search)
            .id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        for (code, text) in [(KeyCode::KeyD, Some("delay")), (KeyCode::Backspace, None)] {
            app.world_mut().write_message(KeyboardInput {
                key_code: code,
                logical_key: text
                    .map(|v| Key::Character(v.into()))
                    .unwrap_or(Key::Backspace),
                state: ButtonState::Pressed,
                text: text.map(Into::into),
                repeat: false,
                window,
            });
            app.update();
            assert!(
                !app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .just_pressed(code)
            );
        }
        assert_eq!(app.world().resource::<TileLibrarySearch>().query, "dela");
        assert_eq!(
            app.world()
                .get::<bevy::a11y::AccessibilityNode>(field)
                .unwrap()
                .value(),
            Some("dela")
        );
    }
}
