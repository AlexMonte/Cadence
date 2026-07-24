//! Modal prompt when leaving the editor with unsaved changes.
//!
//! Save is requested via [`EditorCommandBus`] only — this module never
//! `ResMut`s document or session resources.

use bevy::{picking::prelude::*, prelude::*};
use bevy_feathers::theme::ThemedText;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::session::MusaicProject;
use crate::infrastructure::app::{AppState, MusaicSet};
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::widgets::spawn_dialog_overlay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitTarget {
    MainMenu,
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
        spawn_dialog_overlay(&mut commands, &theme, UnsavedDialogRoot, |card| {
            card.spawn((
                Text::new("Save changes?"),
                TextFont {
                    font_size: theme.typography.title,
                    ..default()
                },
                ThemedText,
            ));
            card.spawn((Text::new("Your project has unsaved changes."), ThemedText));
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
        });
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
            btn.spawn((Text::new(label.to_string()), ThemedText));
        });
}

fn on_unsaved_dialog_click(
    mut click: On<Pointer<Click>>,
    buttons: Query<&UnsavedDialogAction>,
    mut pending: ResMut<PendingUnsavedDialogAction>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(action) = buttons.get(click.event_target()) else {
        return;
    };
    pending.0 = Some(*action);
    click.propagate(false);
}

#[derive(Resource, Default)]
struct PendingUnsavedDialogAction(pub Option<UnsavedDialogAction>);

fn handle_unsaved_dialog_actions(
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
            perform_exit(target, &mut next);
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
    mut prompt: ResMut<UnsavedChangesPrompt>,
    project: Res<MusaicProject>,
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
    perform_exit(target, &mut next);
}

fn perform_exit(target: ExitTarget, next: &mut NextState<AppState>) {
    match target {
        ExitTarget::MainMenu => next.set(AppState::MainMenu),
    }
}
