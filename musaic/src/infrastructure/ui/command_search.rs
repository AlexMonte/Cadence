//! Search existing editor actions without leaving the board or learning a key binding.
use super::{
    theme::MusaicUiTheme,
    widgets::{musaic_button, spawn_dialog_overlay},
};
use crate::application::editor::preferences::keymap::Action;
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus, editing::TileEdit},
        editor::interaction::keyboard_navigation::shortcuts_available,
    },
    domain::document::ContainerKind,
    infrastructure::app::{AppState, MusaicSet},
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};
use bevy_ui_widgets::Activate;

#[derive(Resource, Default)]
pub(super) struct CommandSearch {
    pub open: bool,
    query: String,
    selected: usize,
}
#[derive(Component)]
struct SearchDialog;
#[derive(Component)]
struct SearchField;
#[derive(Component)]
struct Results;
#[derive(Component)]
struct SearchResult(SearchAction);

#[derive(Clone)]
enum SearchAction {
    Command(Box<EditorCommand>),
    InsertNotes,
    Connect,
    View(super::camera_rig::BoardViewAction),
}

fn execute(
    action: &SearchAction,
    bus: &mut MessageWriter<EditorCommandBus>,
    insertion: &mut MessageWriter<super::note_entry::OpenNoteEntry>,
    connection: &mut MessageWriter<super::connection_search::OpenConnectionSearch>,
    view: &mut MessageWriter<super::camera_rig::BoardViewAction>,
) {
    match action {
        SearchAction::View(action) => {
            view.write(*action);
        }
        SearchAction::Connect => {
            connection.write(super::connection_search::OpenConnectionSearch(None));
        }
        SearchAction::Command(command) => {
            bus.write(EditorCommandBus(command.as_ref().clone()));
        }
        SearchAction::InsertNotes => {
            insertion.write(super::note_entry::OpenNoteEntry);
        }
    }
}

fn catalog() -> Vec<(&'static str, SearchAction)> {
    let commands = vec![
        ("Play / pause", EditorCommand::TransportToggle),
        ("Stop and return to start", EditorCommand::TransportStop),
        ("Panic · silence all voices", EditorCommand::TransportPanic),
        ("Undo", EditorCommand::Undo),
        ("Redo", EditorCommand::Redo),
        (
            "Duplicate as independent variation",
            EditorCommand::EditTiles(TileEdit::IndependentVariation),
        ),
        ("Copy tiles", EditorCommand::EditTiles(TileEdit::Copy)),
        (
            "Paste tiles at cursor",
            EditorCommand::EditTiles(TileEdit::Paste),
        ),
        (
            "Duplicate tiles",
            EditorCommand::EditTiles(TileEdit::Duplicate),
        ),
        ("Delete tiles", EditorCommand::DeleteSelection),
        (
            "Group as sequence",
            EditorCommand::EditTiles(TileEdit::Group(ContainerKind::Sequence)),
        ),
        (
            "Group as layer",
            EditorCommand::EditTiles(TileEdit::Group(ContainerKind::Parallel)),
        ),
        (
            "Show tile library",
            EditorCommand::SetDrawerOpen { open: true },
        ),
        (
            "Hide tile library",
            EditorCommand::SetDrawerOpen { open: false },
        ),
        ("Show / hide minimap", EditorCommand::ToggleMinimap),
        ("Show pattern timing", EditorCommand::EnterTimelineMode),
        ("Hide pattern timing", EditorCommand::EnterCompose),
        ("Save project", EditorCommand::SaveProject),
    ];
    commands
        .into_iter()
        .map(|(label, command)| (label, SearchAction::Command(Box::new(command))))
        .chain(
            crate::application::command::editing::TRANSPOSE_ACTIONS
                .into_iter()
                .map(|(label, action)| {
                    (
                        label,
                        SearchAction::Command(Box::new(EditorCommand::EditTiles(action))),
                    )
                }),
        )
        .chain(std::iter::once((
            "Insert notes…",
            SearchAction::InsertNotes,
        )))
        .chain(std::iter::once((
            "Connect selected tile…",
            SearchAction::Connect,
        )))
        .chain(
            super::camera_rig::BOARD_VIEW_ACTIONS
                .into_iter()
                .map(|(label, action)| (label, SearchAction::View(action))),
        )
        .collect()
}
fn matches(query: &str) -> Vec<(&'static str, SearchAction)> {
    catalog()
        .into_iter()
        .filter(|(label, _)| {
            let label = label.to_lowercase();
            query
                .split_whitespace()
                .all(|word| label.contains(&word.to_lowercase()))
        })
        .collect()
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CommandSearch>()
        .add_systems(
            OnExit(AppState::Editor),
            |mut state: ResMut<CommandSearch>| *state = default(),
        )
        .add_systems(
            PreUpdate,
            open_shortcut
                .after(bevy::input_focus::InputFocusSystems::Dispatch)
                .run_if(in_state(AppState::Editor))
                .run_if(shortcuts_available),
        )
        .add_systems(
            Update,
            sync.in_set(MusaicSet::RenderUi)
                .run_if(in_state(AppState::Editor)),
        );
}
fn open_shortcut(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut search: ResMut<CommandSearch>,
    preferences: Res<crate::application::editor::preferences::EditorPreferences>,
) {
    if let Some(key) = preferences.shortcut(Action::Search, &keys) {
        *search = CommandSearch {
            open: true,
            ..default()
        };
        keys.clear_just_pressed(key);
    }
}

pub(super) fn open(_: On<Activate>, mut search: ResMut<CommandSearch>) {
    *search = CommandSearch {
        open: true,
        ..default()
    };
}
fn close(_: On<Activate>, mut search: ResMut<CommandSearch>) {
    search.open = false;
}
fn focus_field(mut event: On<Pointer<Press>>, mut focus: ResMut<InputFocus>) {
    focus.set(event.entity);
    event.propagate(false);
}
fn type_query(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut search: ResMut<CommandSearch>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut insertion: MessageWriter<super::note_entry::OpenNoteEntry>,
    mut connection: MessageWriter<super::connection_search::OpenConnectionSearch>,
    mut view: MessageWriter<super::camera_rig::BoardViewAction>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let key = event.input.key_code;
    keys.clear_just_pressed(key);
    event.propagate(false);
    let control = [
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ]
    .iter()
    .any(|key| keys.pressed(*key));
    match key {
        KeyCode::Escape => search.open = false,
        KeyCode::ArrowDown => {
            search.selected =
                (search.selected + 1).min(matches(&search.query).len().saturating_sub(1))
        }
        KeyCode::ArrowUp => search.selected = search.selected.saturating_sub(1),
        KeyCode::Enter => {
            if let Some((_, command)) = matches(&search.query).get(search.selected) {
                execute(
                    command,
                    &mut bus,
                    &mut insertion,
                    &mut connection,
                    &mut view,
                );
                search.open = false;
            }
        }
        KeyCode::Backspace | KeyCode::Delete => {
            if control {
                search.query.clear();
            } else {
                search.query.pop();
            }
            search.selected = 0;
        }
        _ if !control => {
            if let Some(text) =
                event
                    .input
                    .text
                    .as_deref()
                    .or_else(|| match &event.input.logical_key {
                        Key::Character(value) => Some(value.as_str()),
                        _ => None,
                    })
            {
                for c in text.chars().filter(|c| !c.is_control()) {
                    if search.query.len() + c.len_utf8() <= 96 {
                        search.query.push(c);
                    }
                }
                search.selected = 0;
            }
        }
        _ => {}
    }
}
fn activate(
    event: On<Activate>,
    results: Query<&SearchResult>,
    mut search: ResMut<CommandSearch>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut insertion: MessageWriter<super::note_entry::OpenNoteEntry>,
    mut connection: MessageWriter<super::connection_search::OpenConnectionSearch>,
    mut view: MessageWriter<super::camera_rig::BoardViewAction>,
) {
    if let Ok(action) = results.get(event.entity) {
        execute(
            &action.0,
            &mut bus,
            &mut insertion,
            &mut connection,
            &mut view,
        );
        search.open = false;
    }
}
fn spawn_results(
    parent: &mut ChildSpawnerCommands<'_>,
    search: &CommandSearch,
    theme: &MusaicUiTheme,
) {
    let results = matches(&search.query);
    if results.is_empty() {
        parent.spawn((
            Text::new("No matching commands · try a shorter search"),
            TextColor(theme.chrome.text_dim),
        ));
    }
    // Show a moving window of six results; all results remain reachable with arrows.
    let start = search.selected.saturating_sub(5);
    for (index, (label, command)) in results.into_iter().enumerate().skip(start).take(6) {
        parent
            .spawn((
                musaic_button(
                    Node {
                        width: percent(100),
                        min_height: px(34),
                        padding: UiRect::axes(px(10), px(6)),
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(px(theme.radii.sm)),
                        ..default()
                    },
                    SearchResult(command),
                    label,
                ),
                BackgroundColor(if index == search.selected {
                    theme.chrome.crumb_selected_bg
                } else {
                    theme.chrome.panel_bg
                }),
            ))
            .observe(activate);
    }
}
fn sync(
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
    search: Res<CommandSearch>,
    roots: Query<Entity, With<SearchDialog>>,
    results: Query<Entity, With<Results>>,
    mut fields: Query<&mut Text, With<SearchField>>,
) {
    if !search.open {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }
    if roots.is_empty() {
        let root = spawn_dialog_overlay(
            &mut commands,
            &theme,
            (SearchDialog, DespawnOnExit(AppState::Editor)),
            |card| {
                card.spawn((
                    Text::new("Search commands"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_main),
                ));
                card.spawn((
                    SearchField,
                    TabIndex(0),
                    Text::new("Type a command… |"),
                    TextColor(theme.chrome.text_main),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    Node {
                        width: px(430),
                        max_width: percent(100),
                        min_height: px(40),
                        padding: UiRect::all(px(10)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(theme.chrome.window_bg),
                    BorderColor::all(theme.chrome.accent),
                    Pickable::default(),
                ))
                .observe(focus_field);
                card.spawn((
                    Results,
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(3),
                        ..default()
                    },
                ))
                .with_children(|parent| spawn_results(parent, &search, &theme));
                card.spawn((
                    Text::new("↑ ↓ choose · Enter runs · Escape cancels"),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_dim),
                ));
                card.spawn(musaic_button(
                    Node {
                        height: px(32),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    (),
                    "Close",
                ))
                .observe(close);
            },
        );
        commands.entity(root).observe(type_query);
    } else if search.is_changed() {
        for mut text in &mut fields {
            text.0 = if search.query.is_empty() {
                "Type a command… |".into()
            } else {
                format!("{} |", search.query)
            };
        }
        for root in &results {
            commands
                .entity(root)
                .despawn_children()
                .with_children(|parent| spawn_results(parent, &search, &theme));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_matches_all_words_and_returns_the_existing_command() {
        let found = matches("TIMING show");
        assert_eq!(found.len(), 1);
        assert!(matches!(
            &found[0].1,
            SearchAction::Command(command)
                if matches!(command.as_ref(), EditorCommand::EnterTimelineMode)
        ));
        assert!(matches("no such command").is_empty());
    }
    #[test]
    fn typing_and_enter_produce_one_command_without_background_keys() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ))
        .init_resource::<CommandSearch>()
        .add_message::<EditorCommandBus>()
        .add_message::<super::super::camera_rig::BoardViewAction>()
        .add_message::<super::super::connection_search::OpenConnectionSearch>()
        .add_message::<super::super::note_entry::OpenNoteEntry>();
        app.configure_sets(
            PreUpdate,
            bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
        );
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let field = app.world_mut().spawn_empty().observe(type_query).id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        app.world_mut().resource_mut::<CommandSearch>().open = true;
        for (key_code, text) in [(KeyCode::KeyD, Some("show timing")), (KeyCode::Enter, None)] {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key: text
                    .map(|text| Key::Character(text.into()))
                    .unwrap_or(Key::Enter),
                state: ButtonState::Pressed,
                text: text.map(Into::into),
                repeat: false,
                window,
            });
            app.update();
            assert!(
                !app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .just_pressed(key_code)
            );
        }
        assert!(!app.world().resource::<CommandSearch>().open);
        let events: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .drain()
            .collect();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].0, EditorCommand::EnterTimelineMode));
    }
}
