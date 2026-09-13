//! The form previews the current project; the command owner captures the export snapshot.
use crate::{
    application::{
        audio_export::{AudioExportStatus, FileLocationFeedback, export_duration},
        command::{EditorCommand, EditorCommandBus},
        session::MusaicProject,
    },
    infrastructure::{
        app::AppState,
        ui::{
            theme::MusaicUiTheme,
            widgets::{
                exact_number::{self, ExactNumber},
                musaic_button, musaic_chrome_button, spawn_dialog_overlay,
            },
        },
    },
};
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
    ui::InteractionDisabled,
};
use bevy_ui_widgets::{Activate, ValueChange};
use tessera::prelude::Rational;
#[derive(Component)]
struct ExportDialog {
    cycles: u32,
}
#[derive(Component)]
struct ExportMessage;
#[derive(Component)]
struct Scope;
#[derive(Component)]
struct Cycles;
#[derive(Component)]
enum Action {
    Export,
    Reveal,
    Close,
}
pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        (sync, report_location)
            .after(crate::infrastructure::app::MusaicSet::Commands)
            .run_if(in_state(AppState::Editor)),
    )
    .add_systems(
        PreUpdate,
        close_on_escape
            .after(bevy::input_focus::InputFocusSystems::Dispatch)
            .run_if(in_state(AppState::Editor)),
    );
}
pub(super) fn spawn_button(menu: &mut ChildSpawnerCommands<'_>) {
    menu.spawn(musaic_button(
        Node {
            width: percent(100),
            height: px(31),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(px(10)),
            ..default()
        },
        (),
        "Export audio…",
    ))
    .insert(TextFont {
        font_size: 13.0,
        ..default()
    })
    .observe(open_dialog);
}
fn open_dialog(
    _: On<Activate>,
    popups: Query<Entity, With<super::dropdown::Popup>>,
    existing: Query<(), With<ExportDialog>>,
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
) {
    if !existing.is_empty() {
        return;
    }
    for popup in &popups {
        commands.entity(popup).despawn();
    }
    spawn_dialog_overlay(
        &mut commands,
        &theme,
        (ExportDialog { cycles: 8 }, DespawnOnExit(AppState::Editor)),
        |panel| {
            panel.spawn((
                Text::new("Export stereo WAV"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            panel.spawn((Text::new("Render the current project, including unsaved edits and changes queued for playback. Export starts at cycle 1 and does not move live playback."),TextFont {font_size:14.0,..default()},TextColor(theme.chrome.text_main),Node{max_width:px(600),..default()}));
            panel
                .spawn(Node {
                    column_gap: px(12),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new("Cycles"),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(theme.chrome.text_main),
                    ));
                    let field = exact_number::spawn(row, Rational::from_integer(8), Cycles);
                    let mut accessible = accesskit::Node::new(accesskit::Role::TextInput);
                    accessible.set_label("Export cycle count");
                    row.commands()
                        .entity(field)
                        .insert((
                            AccessibilityNode(accessible),
                            Name::new("Export cycle count"),
                        ))
                        .observe(change_cycles);
                    row.spawn((
                        Text::new("1–4096 · Enter to apply"),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(theme.chrome.text_dim),
                    ));
                });
            panel.spawn((
                Scope,
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
                Node {
                    max_width: px(600),
                    ..default()
                },
            ));
            panel.spawn((Text::new("48 kHz stereo WAV. The file ends at the selected cycle boundary; reverb and delay tails are cut there. Closing this dialog lets an export finish in the background."),TextFont{font_size:13.0,..default()},TextColor(theme.chrome.text_dim),Node{max_width:px(600),..default()}));
            let mut accessible = accesskit::Node::new(accesskit::Role::Status);
            accessible.set_live(accesskit::Live::Polite);
            accessible.set_live_atomic();
            panel.spawn((
                ExportMessage,
                AccessibilityNode(accessible),
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
                Node {
                    max_width: px(600),
                    ..default()
                },
            ));
            panel
                .spawn(Node {
                    column_gap: px(8),
                    row_gap: px(8),
                    flex_wrap: FlexWrap::Wrap,
                    max_width: px(600),
                    ..default()
                })
                .with_children(|row| {
                    for (title, action) in [
                        ("Choose file and export…", Action::Export),
                        ("Show export file", Action::Reveal),
                        ("Close", Action::Close),
                    ] {
                        row.spawn((musaic_chrome_button(&theme, title, ()), action))
                            .observe(activate);
                    }
                });
        },
    );
}
fn cycle_count(value: Rational) -> Result<u32, &'static str> {
    if value.denominator != 1 || !(1..=4096).contains(&value.numerator) {
        Err("Choose a whole number from 1 to 4096 cycles.")
    } else {
        Ok(value.numerator as u32)
    }
}
fn ready(
    dialog: &ExportDialog,
    field: &ExactNumber,
    project: &MusaicProject,
) -> Result<f64, String> {
    let cycles = cycle_count(field.draft_value().map_err(str::to_owned)?).map_err(str::to_owned)?;
    if cycles != dialog.cycles {
        return Err("Press Enter to apply the cycle count.".into());
    }
    export_duration(project, cycles)
}
fn change_cycles(
    event: On<ValueChange<Rational>>,
    mut dialogs: Query<&mut ExportDialog>,
    mut fields: Query<&mut ExactNumber, With<Cycles>>,
    mut focus: ResMut<InputFocus>,
) {
    let Ok(mut field) = fields.get_mut(event.source) else {
        return;
    };
    if let Ok(cycles) = cycle_count(event.value) {
        for mut dialog in &mut dialogs {
            dialog.cycles = cycles;
        }
        field.value = event.value;
    } else {
        focus.set(event.source);
    }
}
fn close_on_escape(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    dialogs: Query<Entity, With<ExportDialog>>,
    mut commands: Commands,
) {
    if !dialogs.is_empty() && keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        for dialog in &dialogs {
            commands.entity(dialog).despawn();
        }
    }
}
fn activate(
    event: On<Activate>,
    actions: Query<&Action>,
    dialogs: Query<(Entity, &ExportDialog)>,
    fields: Query<&ExactNumber, With<Cycles>>,
    project: Res<MusaicProject>,
    status: Res<AudioExportStatus>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut commands: Commands,
) {
    let Ok((root, dialog)) = dialogs.single() else {
        return;
    };
    match actions.get(event.entity) {
        Ok(Action::Close) => {
            commands.entity(root).despawn();
        }
        Ok(Action::Reveal) if status.completed.is_some() => {
            bus.write(EditorCommandBus(EditorCommand::RevealLastExport));
        }
        Ok(Action::Export)
            if !status.busy
                && fields
                    .single()
                    .is_ok_and(|field| ready(dialog, field, &project).is_ok()) =>
        {
            bus.write(EditorCommandBus(EditorCommand::ChooseAudioExport {
                cycles: dialog.cycles,
            }));
        }
        _ => {}
    }
}
fn sync(
    dialogs: Query<&ExportDialog>,
    fields: Query<&ExactNumber, With<Cycles>>,
    project: Res<MusaicProject>,
    status: Res<AudioExportStatus>,
    mut texts: Query<
        (&mut Text, Has<Scope>, Option<&mut AccessibilityNode>),
        Or<(With<ExportMessage>, With<Scope>)>,
    >,
    buttons: Query<(Entity, &Action, Has<InteractionDisabled>)>,
    mut commands: Commands,
) {
    let Ok(dialog) = dialogs.single() else {
        return;
    };
    let Ok(field) = fields.single() else {
        return;
    };
    let result = ready(dialog, field, &project);
    let scope = match &result {
        Ok(seconds) => format!(
            "{} · {} cycles · {:.2} seconds\n{} BPM · {} beats per cycle",
            project.metadata.display_name,
            dialog.cycles,
            seconds,
            project.document.playback.bpm,
            project.document.playback.beats_per_cycle
        ),
        Err(_) => format!(
            "{} · check the cycle count below",
            project.metadata.display_name
        ),
    };
    let message = if status.busy {
        status.message.clone()
    } else {
        result
            .as_ref()
            .err()
            .cloned()
            .unwrap_or_else(|| status.message.clone())
    };
    for (mut text, is_scope, accessible) in &mut texts {
        let value = if is_scope { &scope } else { &message };
        if text.0 != *value {
            text.0.clone_from(value);
        }
        if let Some(mut accessible) = accessible {
            if accessible.label() != Some(value.as_str()) {
                accessible.set_label(value.clone());
            }
        }
    }
    for (entity, action, disabled) in &buttons {
        let next = match action {
            Action::Export => status.busy || result.is_err(),
            Action::Reveal => status.completed.is_none(),
            Action::Close => false,
        };
        if next != disabled {
            commands
                .entity(entity)
                .insert(bevy_feathers::theme::ThemeFontColor(if next {
                    bevy_feathers::tokens::BUTTON_TEXT_DISABLED
                } else {
                    bevy_feathers::tokens::TEXT_MAIN
                }));
            if next {
                commands
                    .entity(entity)
                    .insert((InteractionDisabled, TabIndex(-1)));
            } else {
                commands
                    .entity(entity)
                    .remove::<InteractionDisabled>()
                    .insert(TabIndex(0));
            }
        }
    }
}

fn report_location(
    mut messages: MessageReader<FileLocationFeedback>,
    mut navigation: ResMut<
        crate::application::editor::interaction::keyboard_navigation::KeyboardNavigation,
    >,
) {
    for message in messages.read() {
        navigation.status.clone_from(&message.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_cycle_form_rejects_fractional_out_of_range_and_uncommitted_drafts() {
        for value in [
            Rational::new(3, 2),
            Rational::from_integer(0),
            Rational::from_integer(4097),
        ] {
            assert!(cycle_count(value).is_err());
        }
        let mut world = World::new();
        let mut entity = Entity::PLACEHOLDER;
        world.commands().spawn_empty().with_children(|parent| {
            entity = exact_number::spawn(parent, Rational::from_integer(8), Cycles);
        });
        world.flush();
        let project = MusaicProject::new_empty();
        let dialog = ExportDialog { cycles: 8 };
        assert!(ready(&dialog, world.get::<ExactNumber>(entity).unwrap(), &project).is_ok());
        let mut query = world.query::<(&mut ExactNumber, &mut Text)>();
        let (mut field, mut text) = query.get_mut(&mut world, entity).unwrap();
        exact_number::update(&mut field, &mut text, Rational::from_integer(9), false);
        assert_eq!(
            ready(&dialog, &field, &project).unwrap_err(),
            "Press Enter to apply the cycle count."
        );
        exact_number::update(&mut field, &mut text, Rational::new(3, 2), false);
        assert!(
            ready(&dialog, &field, &project)
                .unwrap_err()
                .contains("whole number")
        );
    }
}
