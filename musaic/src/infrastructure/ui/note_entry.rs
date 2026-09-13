//! A temporary form owns text input; only the command dispatcher may author notes.
use super::{
    theme::MusaicUiTheme,
    widgets::{musaic_chrome_button, spawn_dialog_overlay},
};
use crate::application::editor::preferences::keymap::Action;
use crate::{
    application::{
        command::{
            EditorCommand, EditorCommandBus,
            editing::TileEdit,
            note_entry::{self, NoteEntryReceipt, NoteEntryTarget},
        },
        editor::{
            EditorAttention,
            interaction::keyboard_navigation::{KeyboardNavigation, shortcuts_available},
            preferences::EditorPreferences,
        },
        session::MusaicProject,
    },
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
pub(super) struct NoteEntryForm {
    target: Option<NoteEntryTarget>,
    text: String,
    caret: usize,
    error: String,
    pending: Option<u64>,
    next_request: u64,
    step_entry: bool,
    inserted: usize,
}
impl NoteEntryForm {
    fn close(&mut self, navigation: &mut KeyboardNavigation) {
        if self.pending.is_some() {
            return;
        }
        self.target = None;
        navigation.status = if self.inserted > 0 {
            format!(
                "Inserted {} · Undo removes the last insertion",
                expression_count(self.inserted)
            )
        } else {
            "Note entry canceled · project stays open".into()
        };
    }
    fn begin(&mut self, target: NoteEntryTarget) {
        self.target = Some(target);
        self.text.clear();
        self.caret = 0;
        self.error.clear();
        self.pending = None;
        self.step_entry = false;
        self.inserted = 0;
    }
    fn submit(&mut self, bus: &mut MessageWriter<EditorCommandBus>) {
        if self.pending.is_some() {
            return;
        }
        if let Err(error) = validate_text(&self.text) {
            self.error = error;
            return;
        }
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(request) = self.next_request.checked_add(1) else {
            self.error = "Close and restart the editor before inserting more notes.".into();
            return;
        };
        self.next_request = request;
        self.pending = Some(request);
        self.error.clear();
        bus.write(EditorCommandBus(EditorCommand::EditTiles(
            TileEdit::InsertNotes {
                request,
                target,
                text: self.text.clone(),
            },
        )));
    }
}
#[derive(Component)]
struct NoteDialog;
#[derive(Component)]
struct NoteField;
#[derive(Component)]
struct NotePreview;
#[derive(Component)]
struct NoteDestination;
#[derive(Component)]
enum NoteAction {
    Step,
    Close,
}

fn accessible_field(text: &str) -> bevy::a11y::AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::TextInput);
    node.set_label("Notes to insert");
    node.set_description("Enter notes with octaves, for example C4 E4 G4, or a tilde for a rest. Enter inserts. With Step entry on, Enter advances and keeps this field open. Escape discards only unsubmitted text.");
    node.set_value(text);
    bevy::a11y::AccessibilityNode::from(node)
}

#[derive(Message)]
pub(super) struct OpenNoteEntry;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<NoteEntryForm>()
        .add_message::<OpenNoteEntry>()
        .add_systems(
            OnExit(AppState::Editor),
            |mut form: ResMut<NoteEntryForm>| {
                form.target = None;
                form.pending = None;
            },
        )
        .add_systems(
            PreUpdate,
            (open_shortcut.run_if(shortcuts_available), cancel_key)
                .after(bevy::input_focus::InputFocusSystems::Dispatch)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            Update,
            (receive_open, receive, sync)
                .chain()
                .in_set(MusaicSet::RenderUi)
                .after(MusaicSet::Commands)
                .run_if(in_state(AppState::Editor)),
        );
}
fn receive_open(
    mut requests: MessageReader<OpenNoteEntry>,
    project: Res<MusaicProject>,
    attention: Res<EditorAttention>,
    mut form: ResMut<NoteEntryForm>,
) {
    if requests.read().count() > 0 {
        form.begin(note_entry::target(&project.document, &attention, false));
    }
}
fn open_shortcut(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    preferences: Res<EditorPreferences>,
    project: Res<MusaicProject>,
    attention: Res<EditorAttention>,
    mut form: ResMut<NoteEntryForm>,
) {
    let after = preferences.shortcut(Action::InsertAfter, &keys);
    if let Some(key) = after.or_else(|| preferences.shortcut(Action::InsertBefore, &keys)) {
        form.begin(note_entry::target(
            &project.document,
            &attention,
            after.is_some(),
        ));
        keys.clear_just_pressed(key);
    }
}
pub(super) fn open(
    _: On<Activate>,
    project: Res<MusaicProject>,
    attention: Res<EditorAttention>,
    mut form: ResMut<NoteEntryForm>,
) {
    form.begin(note_entry::target(&project.document, &attention, false));
}
fn cancel_key(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut form: ResMut<NoteEntryForm>,
    mut navigation: ResMut<KeyboardNavigation>,
) {
    if form.target.is_some() && form.pending.is_none() && keys.just_pressed(KeyCode::Escape) {
        form.close(&mut navigation);
        keys.clear_just_pressed(KeyCode::Escape);
    }
}
fn cancel(
    _: On<Activate>,
    mut form: ResMut<NoteEntryForm>,
    mut navigation: ResMut<KeyboardNavigation>,
) {
    form.close(&mut navigation);
}
fn toggle_step(
    _: On<Activate>,
    mut form: ResMut<NoteEntryForm>,
    fields: Query<Entity, With<NoteField>>,
    mut focus: ResMut<InputFocus>,
) {
    if form.pending.is_none() {
        form.step_entry = !form.step_entry;
        form.error.clear();
        if let Ok(field) = fields.single() {
            focus.set(field);
        }
    }
}
fn submit(
    _: On<Activate>,
    mut form: ResMut<NoteEntryForm>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    form.submit(&mut bus);
}
fn focus_field(mut event: On<Pointer<Press>>, mut focus: ResMut<InputFocus>) {
    focus.set(event.entity);
    event.propagate(false);
}
fn type_notes(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut form: ResMut<NoteEntryForm>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut navigation: ResMut<KeyboardNavigation>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let key = event.input.key_code;
    event.propagate(false);
    keys.clear_just_pressed(key);
    if form.pending.is_some() {
        return;
    }
    let control = [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]
    .iter()
    .any(|key| keys.pressed(*key));
    let previous = form.text[..form.caret]
        .char_indices()
        .next_back()
        .map_or(0, |(i, _)| i);
    let next = form.text[form.caret..]
        .chars()
        .next()
        .map_or(form.caret, |c| form.caret + c.len_utf8());
    #[cfg(target_os = "macos")]
    if control && key == KeyCode::KeyV {
        match std::process::Command::new("/usr/bin/pbpaste").output() {
            Ok(output) if output.status.success() => match String::from_utf8(output.stdout) {
                Ok(text) if form.text.len() + text.len() <= 65_536 => {
                    let at = form.caret;
                    form.text.insert_str(at, &text);
                    form.caret += text.len();
                    form.error.clear();
                }
                Ok(_) => form.error = "Code exceeds 65536 bytes.".into(),
                Err(_) => form.error = "Clipboard text must be UTF-8.".into(),
            },
            _ => form.error = "Could not read clipboard text. Use Load code file instead.".into(),
        }
        return;
    }
    match key {
        KeyCode::Escape => form.close(&mut navigation),
        KeyCode::Enter if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) => {
            if form.text.len() < 65_536 {
                let at = form.caret;
                form.text.insert(at, '\n');
                form.caret += 1;
            }
        }
        KeyCode::Enter => form.submit(&mut bus),
        KeyCode::ArrowLeft => form.caret = previous,
        KeyCode::ArrowRight => form.caret = next,
        KeyCode::Home => form.caret = 0,
        KeyCode::End => form.caret = form.text.len(),
        KeyCode::Backspace => {
            let end = form.caret;
            form.text.replace_range(previous..end, "");
            form.caret = previous;
            form.error.clear();
        }
        KeyCode::Delete => {
            let start = form.caret;
            form.text.replace_range(start..next, "");
            form.error.clear();
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
                let text: String = text
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                    .collect();
                if form.text.len() + text.len() <= 65_536 {
                    let at = form.caret;
                    form.text.insert_str(at, &text);
                    form.caret += text.len();
                    form.error.clear();
                }
            }
        }
        _ => {}
    }
}
fn receive(
    mut receipts: MessageReader<NoteEntryReceipt>,
    mut form: ResMut<NoteEntryForm>,
    mut navigation: ResMut<KeyboardNavigation>,
    fields: Query<Entity, With<NoteField>>,
    mut focus: Option<ResMut<InputFocus>>,
) {
    for receipt in receipts.read() {
        if form.pending != Some(receipt.request) {
            continue;
        }
        form.pending = None;
        match &receipt.result {
            Ok(()) => {
                navigation.status = format!(
                    "Inserted {} expressions · Undo to restore",
                    note_entry::parse(&form.text).map_or(0, |n| n.len())
                );
                if form.step_entry {
                    if let Some(target) = &receipt.continuation {
                        form.inserted +=
                            note_entry::parse(&form.text).map_or(0, |notes| notes.len());
                        form.target = Some(target.clone());
                        form.text.clear();
                        form.caret = 0;
                        form.error.clear();
                        if let (Some(focus), Ok(field)) = (&mut focus, fields.single()) {
                            focus.set(field);
                        }
                        navigation.status = format!(
                            "Inserted {} this session · Enter adds the next step · Escape finishes",
                            expression_count(form.inserted)
                        );
                    } else {
                        form.target = None;
                    }
                } else {
                    form.target = None;
                }
            }
            Err(error) => form.error.clone_from(error),
        }
    }
}
fn validate_text(text: &str) -> Result<(), String> {
    note_entry::parse(text).map(|_| ())
}
fn expression_count(count: usize) -> String {
    format!(
        "{count} {}",
        if count == 1 {
            "expression"
        } else {
            "expressions"
        }
    )
}
fn preview(form: &NoteEntryForm) -> String {
    if !form.error.is_empty() {
        return form.error.clone();
    }
    if form.pending.is_some() {
        return "Inserting…".into();
    }
    if form.step_entry && form.text.trim().is_empty() {
        return format!(
            "Step entry · {} inserted\nType a note such as C4 or ~, then Enter to advance.\nEscape or Done discards the draft and keeps inserted notes. Each insertion has its own Undo.",
            expression_count(form.inserted)
        );
    }
    match note_entry::parse(&form.text) {
        Ok(notes) => format!(
            "{} expressions · equal relative weight\n{}\n{}",
            notes.len(),
            form.text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join("  →  "),
            if form.step_entry {
                "Enter inserts and advances · Escape keeps inserted notes · One Undo per insertion"
            } else {
                "Enter inserts once · Escape cancels · One undo restores the previous pattern"
            }
        ),
        Err(error) => error,
    }
}
fn destination(target: &NoteEntryTarget) -> String {
    format!("Notes: {}", note_entry::describe(target))
}
fn sync(
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
    form: Res<NoteEntryForm>,
    roots: Query<Entity, With<NoteDialog>>,
    mut fields: Query<
        (&mut Text, &mut bevy::a11y::AccessibilityNode),
        (
            With<NoteField>,
            Without<NotePreview>,
            Without<NoteDestination>,
        ),
    >,
    mut previews: Query<
        &mut Text,
        (
            With<NotePreview>,
            Without<NoteField>,
            Without<NoteDestination>,
        ),
    >,
    mut destinations: Query<
        &mut Text,
        (
            With<NoteDestination>,
            Without<NoteField>,
            Without<NotePreview>,
        ),
    >,
    mut actions: Query<(&NoteAction, &mut super::widgets::ButtonLabel)>,
) {
    let Some(target) = &form.target else {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    };
    if roots.is_empty() {
        spawn_dialog_overlay(
            &mut commands,
            &theme,
            (NoteDialog, DespawnOnExit(AppState::Editor)),
            |card| {
                card.spawn((
                    Text::new("Insert notes"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_main),
                ));
                card.spawn((
                    NoteDestination,
                    Text::new(destination(target)),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_dim),
                    Node {
                        max_width: px(520),
                        ..default()
                    },
                ));
                card.spawn((
                    NoteField,
                    accessible_field(""),
                    TabIndex(0),
                    Text::new("|"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_main),
                    Node {
                        width: px(520),
                        max_width: percent(100),
                        min_height: px(48),
                        max_height: px(260),
                        overflow: Overflow::scroll_y(),
                        padding: UiRect::all(px(10)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(theme.chrome.window_bg),
                    BorderColor::all(theme.chrome.accent),
                    Pickable::default(),
                ))
                .observe(focus_field)
                .observe(type_notes);
                card.spawn((
                    NotePreview,
                    Text::new(preview(&form)),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_main),
                    Node {
                        max_width: px(520),
                        ..default()
                    },
                ));
                card.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(8),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(musaic_chrome_button(&theme, "Insert", ()))
                        .observe(submit);
                    row.spawn(musaic_chrome_button(
                        &theme,
                        "Step entry: Off",
                        NoteAction::Step,
                    ))
                    .observe(toggle_step);
                    row.spawn(musaic_chrome_button(&theme, "Cancel", NoteAction::Close))
                        .observe(cancel);
                });
            },
        );
    } else if form.is_changed() {
        for mut text in &mut destinations {
            text.0 = destination(target);
        }
        for (mut field, mut accessible) in &mut fields {
            accessible.set_value(form.text.clone());
            field.0 = format!("{}|{}", &form.text[..form.caret], &form.text[form.caret..]);
        }
        for mut text in &mut previews {
            text.0 = preview(&form);
        }
        for (action, mut label) in &mut actions {
            let value = match action {
                NoteAction::Step if form.step_entry => "Step entry: On",
                NoteAction::Step => "Step entry: Off",
                NoteAction::Close if form.inserted > 0 => "Done",
                NoteAction::Close => "Cancel",
            };
            if label.0 != value {
                label.0 = value.into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn step_entry_advances_only_on_its_matching_receipt() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<NoteEntryForm>()
            .init_resource::<KeyboardNavigation>()
            .init_resource::<InputFocus>()
            .add_message::<EditorCommandBus>()
            .add_message::<NoteEntryReceipt>()
            .add_systems(Update, receive);
        let field = app.world_mut().spawn(NoteField).id();
        let target = NoteEntryTarget::End(crate::domain::board::BoardSurfaceId(10));
        {
            let mut form = app.world_mut().resource_mut::<NoteEntryForm>();
            form.begin(target.clone());
            form.step_entry = true;
            form.text = "C4 ~".into();
            form.caret = form.text.len();
            form.pending = Some(7);
        }
        let next = NoteEntryTarget::Expression {
            node: tessera::prelude::NodeId::new("rest"),
            after: true,
        };
        app.world_mut().write_message(NoteEntryReceipt {
            request: 8,
            result: Ok(()),
            continuation: Some(next.clone()),
        });
        app.update();
        assert_eq!(app.world().resource::<NoteEntryForm>().target, Some(target));
        assert_eq!(app.world().resource::<NoteEntryForm>().text, "C4 ~");
        app.world_mut().write_message(NoteEntryReceipt {
            request: 7,
            result: Ok(()),
            continuation: Some(next.clone()),
        });
        app.update();
        let form = app.world().resource::<NoteEntryForm>();
        assert_eq!(form.target, Some(next.clone()));
        assert_eq!(form.inserted, 2);
        assert!(form.text.is_empty());
        assert_eq!(app.world().resource::<InputFocus>().0, Some(field));
        app.world_mut().write_message(NoteEntryReceipt {
            request: 7,
            result: Ok(()),
            continuation: Some(next),
        });
        app.update();
        assert_eq!(app.world().resource::<NoteEntryForm>().inserted, 2);
        app.world_mut()
            .run_system_once(
                |mut form: ResMut<NoteEntryForm>, mut navigation: ResMut<KeyboardNavigation>| {
                    form.close(&mut navigation)
                },
            )
            .unwrap();
        assert!(app.world().resource::<NoteEntryForm>().target.is_none());
        assert_eq!(
            app.world().resource::<KeyboardNavigation>().status,
            "Inserted 2 expressions · Undo removes the last insertion"
        );
    }
    #[test]
    fn field_keeps_invalid_input_edits_unicode_and_waits_for_matching_receipt() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ))
        .init_resource::<NoteEntryForm>()
        .init_resource::<KeyboardNavigation>()
        .add_message::<EditorCommandBus>()
        .add_message::<NoteEntryReceipt>()
        .add_systems(Update, receive);
        app.configure_sets(
            PreUpdate,
            bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
        );
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let field = app.world_mut().spawn_empty().observe(type_notes).id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        app.world_mut()
            .resource_mut::<NoteEntryForm>()
            .begin(NoteEntryTarget::End(crate::domain::board::BoardSurfaceId(
                10,
            )));
        let press = |app: &mut App, key_code, text: Option<&str>| {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key: text.map(|s| Key::Character(s.into())).unwrap_or(Key::Enter),
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
        };
        press(&mut app, KeyCode::KeyC, Some("C♯"));
        press(&mut app, KeyCode::Enter, None);
        assert!(
            app.world()
                .resource::<NoteEntryForm>()
                .error
                .contains("complete note")
        );
        assert!(
            app.world()
                .resource::<Messages<EditorCommandBus>>()
                .is_empty()
        );
        // Delete the multibyte sharp safely, then finish the note.
        press(&mut app, KeyCode::Backspace, None);
        press(&mut app, KeyCode::Digit4, Some("4 E4 ~"));
        press(&mut app, KeyCode::Enter, None);
        let request = app.world().resource::<NoteEntryForm>().pending.unwrap();
        press(&mut app, KeyCode::Enter, None);
        assert!(app.world().resource::<NoteEntryForm>().target.is_some());
        app.world_mut().write_message(NoteEntryReceipt {
            request: request + 1,
            result: Ok(()),
            continuation: None,
        });
        app.update();
        assert_eq!(
            app.world().resource::<NoteEntryForm>().pending,
            Some(request)
        );
        app.world_mut().write_message(NoteEntryReceipt {
            request,
            result: Err("Target was deleted".into()),
            continuation: None,
        });
        app.update();
        let form = app.world().resource::<NoteEntryForm>();
        assert_eq!(form.text, "C4 E4 ~");
        assert_eq!(form.error, "Target was deleted");
        assert!(form.target.is_some());
        press(&mut app, KeyCode::Enter, None);
        let request = app.world().resource::<NoteEntryForm>().pending.unwrap();
        app.world_mut().write_message(NoteEntryReceipt {
            request,
            result: Ok(()),
            continuation: None,
        });
        app.update();
        assert!(app.world().resource::<NoteEntryForm>().target.is_none());
    }
}
