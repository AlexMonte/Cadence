//! Shortcut editing owns transient recording state; only validated preferences survive it.
use super::{
    theme::MusaicUiTheme,
    widgets::{ButtonLabel, musaic_button, spawn_dialog_overlay},
};
use crate::{
    application::editor::{
        interaction::keyboard_navigation::KeyboardNavigation,
        preferences::{
            EditorPreferences,
            keymap::{Action, Chord},
        },
    },
    infrastructure::app::{AppState, MusaicSet},
};
use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};
use bevy_ui_widgets::Activate;

#[derive(Resource)]
pub(super) struct Settings {
    open: bool,
    selected: Action,
    recording: Option<Entity>,
    message: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            open: false,
            selected: Action::PlayPause,
            recording: None,
            message: String::new(),
        }
    }
}
#[derive(Component)]
struct Dialog;
#[derive(Component)]
struct Capture;
#[derive(Component)]
struct Status;
#[derive(Component)]
struct Choice(Action);
#[derive(Component)]
struct CharacterToggle;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Settings>()
        .add_systems(OnExit(AppState::Editor), reset)
        .add_systems(
            PreUpdate,
            close_on_escape
                .after(bevy::input_focus::InputFocusSystems::Dispatch)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            Update,
            (sync, paint)
                .chain()
                .in_set(MusaicSet::RenderUi)
                .run_if(in_state(AppState::Editor)),
        );
}
fn reset(mut state: ResMut<Settings>) {
    *state = Settings::default();
}
pub(super) fn open(
    _: On<Activate>,
    mut state: ResMut<Settings>,
    mut help: ResMut<KeyboardNavigation>,
) {
    *state = Settings {
        open: true,
        ..default()
    };
    help.help_open = false;
}
fn close(_: On<Activate>, mut state: ResMut<Settings>) {
    state.open = false;
    state.recording = None;
}
fn close_on_escape(mut keys: ResMut<ButtonInput<KeyCode>>, mut state: ResMut<Settings>) {
    if state.open && keys.just_pressed(KeyCode::Escape) {
        state.open = false;
        state.recording = None;
        keys.clear_just_pressed(KeyCode::Escape);
    }
}
fn record(
    event: On<Activate>,
    choices: Query<&Choice>,
    fields: Query<Entity, With<Capture>>,
    mut state: ResMut<Settings>,
    mut focus: ResMut<InputFocus>,
) {
    let (Ok(choice), Ok(field)) = (choices.get(event.entity), fields.single()) else {
        return;
    };
    state.selected = choice.0;
    state.recording = Some(event.entity);
    state.message = format!(
        "Press a shortcut for {}. Escape cancels recording.",
        choice.0.label()
    );
    focus.set(field);
}
fn record_key(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut state: ResMut<Settings>,
    mut preferences: ResMut<EditorPreferences>,
    mut focus: ResMut<InputFocus>,
) {
    let Some(button) = state.recording else {
        return;
    };
    event.propagate(false);
    let key = event.input.key_code;
    keys.clear_just_pressed(key);
    if event.input.state != ButtonState::Pressed || event.input.repeat {
        return;
    }
    if matches!(
        key,
        KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
    ) {
        return;
    }
    if key == KeyCode::Escape {
        state.recording = None;
        state.message = "Recording canceled; bindings unchanged.".into();
        focus.set(button);
        return;
    }
    let chord = Chord::from_input(key, &keys);
    match preferences.keymap.replace(state.selected, vec![chord]) {
        Ok(()) => {
            state.recording = None;
            state.message = format!(
                "{}: {}. Saved automatically.",
                state.selected.label(),
                chord.label()
            );
            focus.set(button);
        }
        Err(error) => {
            state.message = format!("{error}. Try another shortcut, or Escape to cancel.");
        }
    }
}
fn toggle_characters(
    _: On<Activate>,
    mut preferences: ResMut<EditorPreferences>,
    mut state: ResMut<Settings>,
) {
    state.recording = None;
    preferences.keymap.character_shortcuts = !preferences.keymap.character_shortcuts;
    state.message = "Character shortcut preference saved. Tab, Escape and field editing keep their built-in roles.".into();
}
fn unbind(
    _: On<Activate>,
    mut preferences: ResMut<EditorPreferences>,
    mut state: ResMut<Settings>,
) {
    state.recording = None;
    state.message = match preferences.keymap.replace(state.selected, Vec::new()) {
        Ok(()) => format!("{} is unbound.", state.selected.label()),
        Err(e) => e,
    };
}
fn reset_selected(
    _: On<Activate>,
    mut preferences: ResMut<EditorPreferences>,
    mut state: ResMut<Settings>,
) {
    state.recording = None;
    state.message = match preferences.keymap.reset(state.selected) {
        Ok(()) => format!(
            "{} restored to its profile defaults.",
            state.selected.label()
        ),
        Err(e) => e,
    };
}
fn reset_all(
    _: On<Activate>,
    mut preferences: ResMut<EditorPreferences>,
    mut state: ResMut<Settings>,
) {
    preferences.keymap = Default::default();
    state.recording = None;
    state.message = "All shortcut defaults restored.".into();
}
#[cfg(not(target_arch = "wasm32"))]
fn import(
    _: On<Activate>,
    #[cfg(target_os = "macos")] _main_thread: bevy::ecs::system::NonSendMarker,
    mut preferences: ResMut<EditorPreferences>,
    mut state: ResMut<Settings>,
) {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Import Musaic shortcuts")
        .add_filter("Shortcut settings", &["json"])
        .pick_file()
    else {
        return;
    };
    state.recording = None;
    match crate::adapter::preferences::import_keymap(&path) {
        Ok(map) => {
            preferences.keymap = map;
            state.message = "Shortcut settings imported and saved.".into();
        }
        Err(e) => state.message = format!("Import rejected; shortcuts unchanged. {e}"),
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn export(
    _: On<Activate>,
    #[cfg(target_os = "macos")] _main_thread: bevy::ecs::system::NonSendMarker,
    preferences: Res<EditorPreferences>,
    mut state: ResMut<Settings>,
) {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Export Musaic shortcuts")
        .set_file_name("musaic-shortcuts.json")
        .add_filter("Shortcut settings", &["json"])
        .save_file()
    else {
        return;
    };
    state.message = match crate::adapter::preferences::export_keymap(&path, &preferences.keymap) {
        Ok(()) => "Shortcut settings exported.".into(),
        Err(e) => format!("Export failed. {e}"),
    };
}
fn button() -> Node {
    Node {
        min_height: px(36),
        padding: UiRect::axes(px(10), px(6)),
        align_items: AlignItems::Center,
        flex_shrink: 0.0,
        ..default()
    }
}
fn action_button(theme: &MusaicUiTheme, marker: impl Bundle, label: &str) -> impl Bundle {
    let mut node = button();
    node.border = UiRect::all(px(1));
    node.border_radius = BorderRadius::all(px(theme.radii.md));
    (
        musaic_button(node, marker, label),
        BackgroundColor(theme.chrome.button_bg),
        BorderColor::all(theme.chrome.button_border),
    )
}
fn row() -> Node {
    Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: px(8),
        row_gap: px(4),
        flex_shrink: 0.0,
        ..default()
    }
}
fn sync(
    mut commands: Commands,
    state: Res<Settings>,
    theme: Res<MusaicUiTheme>,
    roots: Query<Entity, With<Dialog>>,
) {
    if !state.open {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }
    spawn_dialog_overlay(
        &mut commands,
        &theme,
        (Dialog, DespawnOnExit(AppState::Editor)),
        |card| {
            card.spawn((
                Text::new("Keyboard shortcuts"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            card.spawn((Text::new("Choose an action, then press its new shortcut. Changes apply to both profiles.\nConflicts are checked in Standard and Vim. Preferences stay separate from music."), TextFont { font_size: 14.0, ..default() }, TextColor(theme.chrome.text_main), Node { max_width: px(650), ..default() }));
            card.spawn(action_button(
                &theme,
                CharacterToggle,
                "Character shortcuts: on",
            ))
            .observe(toggle_characters);
            let mut status_node = accesskit::Node::new(accesskit::Role::Status);
            status_node.set_live(accesskit::Live::Polite);
            card.spawn((
                Capture,
                Status,
                TabIndex(-1),
                bevy::a11y::AccessibilityNode(status_node),
                Text::new("Choose an action below to change its shortcut."),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
                Node {
                    width: px(640),
                    max_width: percent(100),
                    min_height: px(60),
                    flex_shrink: 0.0,
                    ..default()
                },
            ))
            .observe(record_key);
            card.spawn(row()).with_children(|row| {
                row.spawn(action_button(&theme, (), "Unbind selected"))
                    .observe(unbind);
                row.spawn(action_button(&theme, (), "Reset selected"))
                    .observe(reset_selected);
                row.spawn(action_button(&theme, (), "Reset all shortcuts"))
                    .observe(reset_all);
            });
            card.spawn((
                Node {
                    width: px(640),
                    max_width: percent(100),
                    height: px(220),
                    min_height: px(100),
                    overflow: Overflow::scroll_y(),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(theme.chrome.border),
            ))
            .with_children(|list| {
                for &action in Action::ALL {
                    list.spawn((
                        musaic_button(button(), Choice(action), action.label()),
                        BackgroundColor(theme.chrome.panel_bg),
                    ))
                    .observe(record);
                }
            })
            .observe(super::widgets::scroll_dialog);
            card.spawn(row()).with_children(|row| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    row.spawn(action_button(&theme, (), "Import shortcuts…"))
                        .observe(import);
                    row.spawn(action_button(&theme, (), "Export shortcuts…"))
                        .observe(export);
                }
                row.spawn(action_button(&theme, (), "Done")).observe(close);
            });
        },
    );
}
fn paint(
    state: Res<Settings>,
    preferences: Res<EditorPreferences>,
    theme: Res<MusaicUiTheme>,
    mut choices: Query<(&Choice, &mut ButtonLabel, &mut BackgroundColor), Without<CharacterToggle>>,
    mut toggles: Query<&mut ButtonLabel, With<CharacterToggle>>,
    mut status: Query<(&mut Text, &mut bevy::a11y::AccessibilityNode), With<Status>>,
) {
    if !state.is_changed() && !preferences.is_changed() && !theme.is_changed() {
        return;
    }
    for (choice, mut label, mut color) in &mut choices {
        let value = format!(
            "{} · {}",
            choice.0.label(),
            preferences
                .keymap
                .describe(choice.0, preferences.vim_navigation)
        );
        if label.0 != value {
            label.0 = value;
        }
        color.0 = if choice.0 == state.selected {
            theme.chrome.crumb_selected_bg
        } else {
            theme.chrome.panel_bg
        };
    }
    for mut toggle in &mut toggles {
        toggle.0 = format!(
            "Character shortcuts: {} · click to {}",
            if preferences.keymap.character_shortcuts {
                "on"
            } else {
                "off"
            },
            if preferences.keymap.character_shortcuts {
                "disable"
            } else {
                "enable"
            }
        );
    }
    let message = format!(
        "Selected: {}\n{}",
        state.selected.label(),
        if state.message.is_empty() {
            "Choose an action below to change its shortcut."
        } else {
            &state.message
        }
    );
    for (mut text, mut accessible) in &mut status {
        if text.0 != message {
            text.0.clone_from(&message);
            accessible.set_value(message.clone());
        }
    }
}
