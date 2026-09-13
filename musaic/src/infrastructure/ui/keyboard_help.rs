//! Discoverable keyboard profile and feedback, scoped to the editor screen.
use super::{
    theme::MusaicUiTheme,
    widgets::{musaic_button, spawn_dialog_overlay},
};
use crate::application::editor::interaction::keyboard_navigation::{
    KeyboardFocusMoved, KeyboardNavigation,
};
use crate::application::editor::preferences::{EditorPreferences, keymap::Action};
use crate::infrastructure::app::{AppState, MusaicSet};
use bevy::{input_focus::InputFocus, prelude::*};
use bevy_ui_widgets::Activate;

#[derive(Component)]
struct KeyboardHelp;
#[derive(Component)]
pub(super) struct KeyboardStatus;
#[derive(Component)]
struct ProfileLabel;
#[derive(Component)]
struct MotionLabel;
#[derive(Component)]
struct BindingSummary;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (sync_help, sync_status)
            .in_set(MusaicSet::RenderUi)
            .run_if(in_state(AppState::Editor)),
    )
    .add_systems(
        PreUpdate,
        close_help.after(bevy::input_focus::InputFocusSystems::Dispatch),
    )
    .add_systems(
        Update,
        follow_keyboard
            .before(super::camera_rig::CameraRigSet::Arbitrate)
            .after(MusaicSet::Input)
            .run_if(in_state(AppState::Editor)),
    );
}

fn follow_keyboard(
    mut moves: MessageReader<KeyboardFocusMoved>,
    mut camera: MessageWriter<super::camera_rig::CameraRequest>,
) {
    if let Some(last) = moves.read().last() {
        camera.write(super::camera_rig::CameraRequest::FocusAddress(last.0));
    }
}

pub(super) fn spawn_button(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn(musaic_button(
            Node {
                height: px(32),
                padding: UiRect::axes(px(9), px(5)),
                align_items: AlignItems::Center,
                ..default()
            },
            (),
            "Keys",
        ))
        .observe(open_help);
}

fn open_help(_: On<Activate>, mut state: ResMut<KeyboardNavigation>) {
    state.help_open = !state.help_open;
}

fn close_help(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    state: Option<ResMut<KeyboardNavigation>>,
    preferences: Res<EditorPreferences>,
    mut focus: ResMut<InputFocus>,
) {
    let Some(mut state) = state else {
        return;
    };
    let help_key = preferences.shortcut(Action::Help, &keys);
    if state.help_open && (keys.just_pressed(KeyCode::Escape) || help_key.is_some()) {
        state.help_open = false;
        keys.clear_just_pressed(KeyCode::Escape);
        if let Some(key) = help_key {
            keys.clear_just_pressed(key);
        }
        focus.clear();
    }
}

fn toggle_profile(
    _: On<Activate>,
    mut state: ResMut<KeyboardNavigation>,
    mut preferences: ResMut<EditorPreferences>,
) {
    preferences.vim_navigation = !preferences.vim_navigation;
    state.count = 0;
    state.selection_anchor = None;
    state.status.clear();
}

fn toggle_motion(_: On<Activate>, mut preferences: ResMut<EditorPreferences>) {
    preferences.reduced_motion = !preferences.reduced_motion;
}

fn sync_help(
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
    state: Res<KeyboardNavigation>,
    roots: Query<Entity, With<KeyboardHelp>>,
) {
    if !state.help_open {
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
        (KeyboardHelp, DespawnOnExit(AppState::Editor)),
        |card| {
            card.spawn((
                Text::new("Keyboard navigation"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            card.spawn(musaic_button(
                Node {
                    height: px(36),
                    align_items: AlignItems::Center,
                    ..default()
                },
                ProfileLabel,
                "Enable Vim navigation",
            ))
            .observe(toggle_profile);
            card.spawn(musaic_button(
                Node {
                    height: px(36),
                    align_items: AlignItems::Center,
                    ..default()
                },
                MotionLabel,
                "Reduce motion",
            ))
            .observe(toggle_motion);
            card.spawn(musaic_button(
                Node {
                    height: px(36),
                    ..default()
                },
                (),
                "Customize shortcuts…",
            ))
            .observe(super::shortcut_settings::open);
            card.spawn((
                BindingSummary,
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
                Node {
                    max_width: px(680),
                    ..default()
                },
            ));
            card.spawn(musaic_button(
                Node {
                    height: px(36),
                    align_items: AlignItems::Center,
                    ..default()
                },
                (),
                "Close",
            ))
            .observe(open_help);
        },
    );
}

fn sync_status(
    state: Res<KeyboardNavigation>,
    preferences: Res<EditorPreferences>,
    region: Res<super::keyboard_regions::RegionNavigation>,
    mut text: Query<&mut Text, (With<KeyboardStatus>, Without<BindingSummary>)>,
    mut summaries: Query<&mut Text, (With<BindingSummary>, Without<KeyboardStatus>)>,
    mut profiles: Query<
        &mut super::widgets::ButtonLabel,
        (With<ProfileLabel>, Without<MotionLabel>),
    >,
    mut motions: Query<
        &mut super::widgets::ButtonLabel,
        (With<MotionLabel>, Without<ProfileLabel>),
    >,
) {
    let mode = if state.selection_anchor.is_some() {
        "Select"
    } else {
        "Navigate"
    };
    let profile = if preferences.vim_navigation {
        "Vim"
    } else {
        "Standard"
    };
    let hint = if state.count > 0 {
        format!("{} · awaiting motion", state.count)
    } else if state.status.is_empty() {
        format!(
            "{} · keyboard help",
            preferences
                .keymap
                .describe(Action::Help, preferences.vim_navigation)
        )
    } else {
        state.status.clone()
    };
    let label = if region.current == super::keyboard_regions::RegionKind::Board {
        format!("{profile} · {mode} · {hint}")
    } else {
        format!(
            "{} region · Tab selects controls · {} changes region · Escape returns to board",
            region.current.label(),
            preferences
                .keymap
                .describe(Action::NextRegion, preferences.vim_navigation)
        )
    };
    for mut text in &mut text {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
    for mut summary in &mut summaries {
        let mut lines: Vec<String> = [
            Action::Left,
            Action::Right,
            Action::Up,
            Action::Down,
            Action::Search,
            Action::NextRegion,
            Action::Inspect,
            Action::InsertBefore,
            Action::PlayPause,
            Action::Stop,
            Action::Undo,
            Action::Redo,
            Action::Copy,
            Action::Paste,
        ]
        .into_iter()
        .map(|a| {
            format!(
                "{} · {}",
                preferences.keymap.describe(a, preferences.vim_navigation),
                a.label()
            )
        })
        .collect();
        lines.push("Tab / Shift+Tab · controls     Escape · cancel locally".into());
        if preferences.vim_navigation && preferences.keymap.character_shortcuts {
            lines.push("Vim: digits count the next motion. Customize shortcuts lists every active binding.".into());
        }
        let value = lines.join("\n");
        if summary.0 != value {
            summary.0 = value;
        }
    }
    for mut motion in &mut motions {
        let label = if preferences.reduced_motion {
            "Reduced motion: on · click to disable"
        } else {
            "Reduced motion: off · click to enable"
        };
        if motion.0 != label {
            motion.0 = label.into();
        }
    }
    for mut profile in &mut profiles {
        let label = if preferences.vim_navigation {
            "Vim navigation: on · click to disable"
        } else {
            "Vim navigation: off · click to enable"
        };
        if profile.0 != label {
            profile.0 = label.into();
        }
    }
}
