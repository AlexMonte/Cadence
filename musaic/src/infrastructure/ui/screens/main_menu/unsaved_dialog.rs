//! Modal prompt when leaving the editor with unsaved changes.
//!
//! Save is requested via [`EditorCommandBus`] only — this module never
//! `ResMut`s document or session resources.

use bevy::prelude::*;
use bevy_ui_widgets::Activate;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::session::MusaicProject;
use crate::infrastructure::app::{AppState, MusaicSet};
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::widgets::spawn_dialog_overlay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitTarget {
    MainMenu,
    NewProject,
    LoadProject,
}

#[derive(Resource, Debug, Default)]
pub struct UnsavedChangesPrompt {
    pub active: Option<ExitTarget>,
    /// After emitting [`EditorCommand::SaveProject`], leave once the document is clean.
    pub exit_after_save: bool,
}

#[derive(Component)]
struct UnsavedDialogRoot;

#[derive(Component, Clone, Copy)]
enum UnsavedDialogAction {
    Save,
    Discard,
    Cancel,
}

pub struct UnsavedDialogPlugin;

impl Plugin for UnsavedDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnsavedChangesPrompt>()
            .init_resource::<PendingUnsavedDialogAction>()
            .add_systems(
                Update,
                (
                    sync_unsaved_dialog,
                    handle_unsaved_dialog_actions.before(MusaicSet::Commands),
                    finish_exit_after_save.after(MusaicSet::Commands),
                )
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

pub fn request_exit(
    project: &MusaicProject,
    prompt: &mut UnsavedChangesPrompt,
    target: ExitTarget,
) -> bool {
    if project.metadata.dirty {
        prompt.active = Some(target);
        prompt.exit_after_save = false;
        false
    } else {
        true
    }
}

fn sync_unsaved_dialog(
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
    prompt: Res<UnsavedChangesPrompt>,
    existing: Query<Entity, With<UnsavedDialogRoot>>,
) {
    let show = prompt.active.is_some();
    let has_dialog = !existing.is_empty();

    if show && !has_dialog {
        spawn_dialog_overlay(
            &mut commands,
            &theme,
            (UnsavedDialogRoot, DespawnOnExit(AppState::Editor)),
            |card| {
                card.spawn((
                    Text::new("Save changes?"),
                    TextFont {
                        font_size: theme.typography.title,
                        ..default()
                    },
                    TextColor(theme.chrome.text_main),
                ));
                card.spawn((
                    Text::new("Your project has unsaved changes."),
                    TextColor(theme.chrome.text_main),
                ));
                card.spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(theme.spacing.lg),
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                })
                .with_children(|row| {
                    spawn_dialog_button(row, &theme, "Cancel", UnsavedDialogAction::Cancel);
                    spawn_dialog_button(row, &theme, "Don't save", UnsavedDialogAction::Discard);
                    spawn_dialog_button(row, &theme, "Save", UnsavedDialogAction::Save);
                });
            },
        );
    } else if !show && has_dialog {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_dialog_button(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    label: &str,
    action: UnsavedDialogAction,
) {
    parent
        .spawn((
            action,
            crate::infrastructure::ui::widgets::MusaicClickable,
            Node {
                height: Val::Px(36.0),
                padding: UiRect::horizontal(Val::Px(theme.spacing.lg + 2.0)),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme.chrome.button_bg),
            BorderColor::all(theme.chrome.button_border),
            Pickable::default(),
        ))
        .observe(on_unsaved_dialog_click)
        .with_children(|btn| {
            btn.spawn((
                Text::new(label.to_string()),
                TextColor(theme.chrome.text_main),
                Pickable::IGNORE,
            ));
        });
}

fn on_unsaved_dialog_click(
    click: On<Activate>,
    buttons: Query<&UnsavedDialogAction>,
    mut pending: ResMut<PendingUnsavedDialogAction>,
) {
    let Ok(action) = buttons.get(click.entity) else {
        return;
    };
    pending.0 = Some(*action);
}

#[derive(Resource, Default)]
struct PendingUnsavedDialogAction(pub Option<UnsavedDialogAction>);

fn handle_unsaved_dialog_actions(
    #[cfg(target_os = "macos")] _main_thread: bevy::ecs::system::NonSendMarker,
    mut pending: ResMut<PendingUnsavedDialogAction>,
    mut prompt: ResMut<UnsavedChangesPrompt>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(action) = pending.0.take() else {
        return;
    };
    let Some(target) = prompt.active else {
        return;
    };

    match action {
        UnsavedDialogAction::Cancel => {
            prompt.active = None;
            prompt.exit_after_save = false;
        }
        UnsavedDialogAction::Discard => {
            prompt.active = None;
            prompt.exit_after_save = false;
            perform_exit(target, &mut next, &mut bus);
        }
        UnsavedDialogAction::Save => {
            bus.write(EditorCommandBus(EditorCommand::SaveProject));
            prompt.exit_after_save = true;
        }
    }
}

/// Completes dialog exit after [`EditorCommand::SaveProject`] clears dirty.
///
/// Runs after `MusaicSet::Commands` so the same-frame save is visible.
fn finish_exit_after_save(
    #[cfg(target_os = "macos")] _main_thread: bevy::ecs::system::NonSendMarker,
    mut prompt: ResMut<UnsavedChangesPrompt>,
    project: Res<MusaicProject>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !prompt.exit_after_save {
        return;
    }
    if project.metadata.dirty {
        // Picker cancelled or save failed — keep the dialog open.
        prompt.exit_after_save = false;
        return;
    }
    let Some(target) = prompt.active.take() else {
        prompt.exit_after_save = false;
        return;
    };
    prompt.exit_after_save = false;
    perform_exit(target, &mut next, &mut bus);
}

pub(crate) fn perform_exit(
    target: ExitTarget,
    next: &mut NextState<AppState>,
    bus: &mut MessageWriter<EditorCommandBus>,
) {
    match target {
        ExitTarget::MainMenu => next.set(AppState::MainMenu),
        ExitTarget::NewProject => {
            bus.write(EditorCommandBus(EditorCommand::NewProject));
        }
        ExitTarget::LoadProject => {
            use crate::adapter::persistence::{EditorOpenResult, editor_open_project};
            if let Some(result) = editor_open_project() {
                bus.write(EditorCommandBus(match result {
                    EditorOpenResult::Path(path) => EditorCommand::OpenProject { path },
                    EditorOpenResult::Project(project) => EditorCommand::AdoptProject {
                        project: *project,
                        path: None,
                    },
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .insert_state(AppState::Editor)
            .init_resource::<MusaicUiTheme>()
            .init_resource::<MusaicProject>()
            .add_message::<EditorCommandBus>()
            .add_plugins(UnsavedDialogPlugin);
        app.world_mut().resource_mut::<MusaicProject>().mark_dirty();
        app.world_mut()
            .resource_mut::<UnsavedChangesPrompt>()
            .active = Some(ExitTarget::NewProject);
        app.update();
        app
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_open_after_discard_or_save_stays_on_the_main_thread() {
        use bevy::ecs::system::System;
        let mut discard = IntoSystem::into_system(handle_unsaved_dialog_actions);
        let mut save = IntoSystem::into_system(finish_exit_after_save);
        discard.initialize(&mut World::new());
        save.initialize(&mut World::new());
        assert!(!discard.is_send());
        assert!(!save.is_send());
    }

    #[test]
    fn prompt_labels_have_visible_colors() {
        let mut app = prompt_app();
        let expected = app.world().resource::<MusaicUiTheme>().chrome.text_main;
        let labels: Vec<_> = app
            .world_mut()
            .query::<(&Text, &TextColor)>()
            .iter(app.world())
            .map(|(text, color)| {
                assert_eq!(color.0, expected);
                text.0.clone()
            })
            .collect();
        for label in [
            "Save changes?",
            "Your project has unsaved changes.",
            "Cancel",
            "Don't save",
            "Save",
        ] {
            assert!(labels.iter().any(|text| text == label), "missing {label}");
        }
    }

    #[test]
    fn cancel_keeps_the_project_and_emits_no_new_command() {
        let mut app = prompt_app();
        app.world_mut()
            .resource_mut::<PendingUnsavedDialogAction>()
            .0 = Some(UnsavedDialogAction::Cancel);
        app.update();
        assert!(app.world().resource::<MusaicProject>().metadata.dirty);
        assert!(
            app.world()
                .resource::<UnsavedChangesPrompt>()
                .active
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Messages<EditorCommandBus>>()
                .is_empty()
        );
    }

    #[test]
    fn cancelled_or_failed_save_keeps_the_prompt_open() {
        let mut app = prompt_app();
        app.world_mut()
            .resource_mut::<PendingUnsavedDialogAction>()
            .0 = Some(UnsavedDialogAction::Save);
        app.update();
        let prompt = app.world().resource::<UnsavedChangesPrompt>();
        assert_eq!(prompt.active, Some(ExitTarget::NewProject));
        assert!(!prompt.exit_after_save);
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .drain()
            .collect();
        assert_eq!(commands.len(), 1);
        assert!(matches!(commands[0].0, EditorCommand::SaveProject));
    }

    #[test]
    fn successful_save_completes_new_project_after_commands() {
        let mut app = prompt_app();
        app.add_systems(
            Update,
            (|mut bus: MessageReader<EditorCommandBus>, mut project: ResMut<MusaicProject>| {
                for command in bus.read() {
                    if matches!(command.0, EditorCommand::SaveProject) {
                        project.metadata.dirty = false;
                    }
                }
            })
            .in_set(MusaicSet::Commands),
        );
        app.world_mut()
            .resource_mut::<PendingUnsavedDialogAction>()
            .0 = Some(UnsavedDialogAction::Save);
        app.update();
        assert!(
            app.world()
                .resource::<UnsavedChangesPrompt>()
                .active
                .is_none()
        );
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .drain()
            .collect();
        assert_eq!(commands.len(), 2);
        assert!(matches!(commands[0].0, EditorCommand::SaveProject));
        assert!(matches!(commands[1].0, EditorCommand::NewProject));
    }
}
