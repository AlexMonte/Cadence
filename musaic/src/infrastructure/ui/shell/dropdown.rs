//! File, Edit and View menus share the editor's existing command path.
use crate::application::editor::preferences::keymap::Action as Shortcut;
use bevy::prelude::*;
use bevy_ui_widgets::Activate;

use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus, editing::TileEdit},
        editor::{MinimapPanelState, TimelinePanelState},
        pipeline::ui_projection::EditorUiProjection,
        session::MusaicProject,
    },
    domain::document::ContainerKind,
    infrastructure::{
        app::AppState,
        ui::{
            screens::main_menu::unsaved_dialog::{
                ExitTarget, UnsavedChangesPrompt, perform_exit, request_exit,
            },
            theme::MusaicUiTheme,
            widgets::musaic_button,
        },
    },
};

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Menu {
    File,
    Edit,
    View,
}
#[derive(Component)]
pub(super) struct Popup(Menu);
#[derive(Component, Clone)]
enum Action {
    Command(Box<EditorCommand>),
    View(super::super::camera_rig::BoardViewAction),
    New,
    Load,
    Close,
}

impl Action {
    fn command(command: EditorCommand) -> Self {
        Self::Command(Box::new(command))
    }
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        dismiss_with_escape.after(bevy::input::InputSystems),
    )
    .add_systems(
        Update,
        project_shortcuts
            .run_if(
                crate::application::editor::interaction::keyboard_navigation::shortcuts_available,
            )
            .before(crate::infrastructure::app::MusaicSet::Commands)
            .run_if(in_state(AppState::Editor)),
    );
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    parent
        .spawn(Node {
            column_gap: px(2),
            ..default()
        })
        .with_children(|bar| {
            for (label, menu) in [
                ("File", Menu::File),
                ("Edit", Menu::Edit),
                ("View", Menu::View),
            ] {
                bar.spawn(musaic_button(
                    Node {
                        height: px(32),
                        padding: UiRect::axes(px(9), px(5)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    menu,
                    label,
                ))
                .insert((
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(theme.chrome.text_main),
                ))
                .observe(open);
            }
            crate::infrastructure::ui::keyboard_help::spawn_button(bar);
        });
}

fn open(
    event: On<Activate>,
    triggers: Query<(&Menu, &ComputedNode, &UiGlobalTransform)>,
    popups: Query<(Entity, &Popup)>,
    theme: Res<MusaicUiTheme>,
    projection: Res<EditorUiProjection>,
    timeline: Res<TimelinePanelState>,
    minimap: Res<MinimapPanelState>,
    mut commands: Commands,
) {
    let Ok((menu, computed, transform)) = triggers.get(event.entity) else {
        return;
    };
    let same = popups.iter().any(|(_, popup)| popup.0 == *menu);
    for (entity, _) in &popups {
        commands.entity(entity).despawn();
    }
    if same {
        return;
    }
    let position = (transform.translation
        - Vec2::new(computed.size().x * 0.5, -computed.size().y * 0.5))
        * computed.inverse_scale_factor();
    commands
        .spawn((
            Popup(*menu),
            crate::application::editor::interaction::keyboard_navigation::KeyboardModal,
            bevy::input_focus::tab_navigation::TabGroup::modal(),
            DespawnOnExit(AppState::Editor),
            GlobalZIndex(700),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(68),
                        bottom: px(0),
                        width: percent(100),
                        ..default()
                    },
                    Pickable::default(),
                ))
                .observe(dismiss);
            overlay
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(position.x),
                        top: px(position.y + 5.0),
                        width: px(224),
                        padding: UiRect::all(px(5)),
                        flex_direction: FlexDirection::Column,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(theme.chrome.panel_bg),
                    BorderColor::all(theme.chrome.border),
                    Pickable::default(),
                ))
                .with_children(|list| match menu {
                    Menu::File => {
                        item(list, "New project", "", Action::New);
                        item(list, "Open project…", "", Action::Load);
                        item(list, "Close project", "", Action::Close);
                        item(
                            list,
                            "Save",
                            "",
                            Action::command(EditorCommand::SaveProject),
                        );
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            item(
                                list,
                                "Show project file",
                                "",
                                Action::command(EditorCommand::RevealProjectFile),
                            );
                            super::export_dialog::spawn_button(list);
                        }
                    }
                    Menu::Edit => {
                        list.spawn(musaic_button(
                            Node {
                                width: percent(100),
                                height: px(31),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(px(10)),
                                ..default()
                            },
                            (),
                            "Insert notes…",
                        ))
                        .observe(crate::infrastructure::ui::note_entry::open)
                        .observe(close_for_command_search);
                        list.spawn(musaic_button(
                            Node {
                                width: percent(100),
                                height: px(31),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(px(10)),
                                ..default()
                            },
                            (),
                            "Search commands…",
                        ))
                        .observe(crate::infrastructure::ui::command_search::open)
                        .observe(close_for_command_search);
                        item(list, "Undo", "", Action::command(EditorCommand::Undo));
                        item(list, "Redo", "", Action::command(EditorCommand::Redo));
                        for (label, action) in [
                            ("Copy", TileEdit::Copy),
                            ("Paste", TileEdit::Paste),
                            ("Duplicate", TileEdit::Duplicate),
                            (
                                "Duplicate as independent variation",
                                TileEdit::IndependentVariation,
                            ),
                            ("Move here", TileEdit::MoveHere),
                            ("Group sequence", TileEdit::Group(ContainerKind::Sequence)),
                            ("Group layer", TileEdit::Group(ContainerKind::Parallel)),
                            (
                                "Group arrangement",
                                TileEdit::Group(ContainerKind::Arrangement),
                            ),
                        ]
                        .into_iter()
                        .chain(crate::application::command::editing::TRANSPOSE_ACTIONS)
                        {
                            item(
                                list,
                                label,
                                "",
                                Action::command(EditorCommand::EditTiles(action)),
                            );
                        }
                        item(
                            list,
                            "Delete",
                            "",
                            Action::command(EditorCommand::DeleteSelection),
                        );
                    }
                    Menu::View => {
                        for (label, action) in super::super::camera_rig::BOARD_VIEW_ACTIONS {
                            item(list, label, "", Action::View(action));
                        }
                        for (label, checked, action) in [
                            (
                                "Tile library",
                                projection.chrome.drawer_open,
                                EditorCommand::ToggleDrawer,
                            ),
                            (
                                "Timeline",
                                timeline.open,
                                if timeline.open {
                                    EditorCommand::EnterCompose
                                } else {
                                    EditorCommand::EnterTimelineMode
                                },
                            ),
                            ("Minimap", minimap.open, EditorCommand::ToggleMinimap),
                        ] {
                            item(
                                list,
                                label,
                                if checked { "· on" } else { "" },
                                Action::command(action),
                            );
                        }
                    }
                });
        });
}

fn item(parent: &mut ChildSpawnerCommands<'_>, label: &str, suffix: &str, action: Action) {
    parent
        .spawn(musaic_button(
            Node {
                width: percent(100),
                height: px(31),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(px(10)),
                ..default()
            },
            action,
            format!("{label}  {suffix}"),
        ))
        .insert(TextFont {
            font_size: 13.0,
            ..default()
        })
        .observe(activate);
}

fn activate(
    event: On<Activate>,
    actions: Query<&Action>,
    popups: Query<Entity, With<Popup>>,
    mut commands: Commands,
    project: Res<MusaicProject>,
    mut prompt: ResMut<UnsavedChangesPrompt>,
    mut next: ResMut<NextState<AppState>>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut view: MessageWriter<super::super::camera_rig::BoardViewAction>,
) {
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    for entity in &popups {
        commands.entity(entity).despawn();
    }
    match action {
        Action::View(action) => {
            view.write(*action);
        }
        Action::Command(command) => {
            bus.write(EditorCommandBus(command.as_ref().clone()));
        }
        Action::New | Action::Load | Action::Close => {
            let target = match action {
                Action::New => ExitTarget::NewProject,
                Action::Close => ExitTarget::MainMenu,
                _ => ExitTarget::LoadProject,
            };
            if prompt.active.is_none() && request_exit(&project, &mut prompt, target) {
                perform_exit(target, &mut next, &mut bus);
            }
        }
    }
}

fn dismiss(
    mut event: On<Pointer<Press>>,
    popups: Query<Entity, With<Popup>>,
    mut commands: Commands,
) {
    for entity in &popups {
        commands.entity(entity).despawn();
    }
    event.propagate(false);
}

fn close_for_command_search(
    _: On<Activate>,
    popups: Query<Entity, With<Popup>>,
    mut commands: Commands,
) {
    for entity in &popups {
        commands.entity(entity).despawn();
    }
}

fn dismiss_with_escape(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    popups: Query<Entity, With<Popup>>,
    mut commands: Commands,
) {
    if keyboard.just_pressed(KeyCode::Escape) && !popups.is_empty() {
        for entity in &popups {
            commands.entity(entity).despawn();
        }
        keyboard.clear_just_pressed(KeyCode::Escape);
    }
}

// Opening a native macOS picker must not block a worker waiting on AppKit.
fn project_shortcuts(
    #[cfg(target_os = "macos")] _main_thread: bevy::ecs::system::NonSendMarker,
    keyboard: Res<ButtonInput<KeyCode>>,
    project: Res<MusaicProject>,
    preferences: Res<crate::application::editor::preferences::EditorPreferences>,
    mut prompt: ResMut<UnsavedChangesPrompt>,
    mut next: ResMut<NextState<AppState>>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if prompt.active.is_some() {
        return;
    }
    let target = if preferences.shortcut(Shortcut::New, &keyboard).is_some() {
        ExitTarget::NewProject
    } else if preferences.shortcut(Shortcut::Open, &keyboard).is_some() {
        ExitTarget::LoadProject
    } else {
        return;
    };
    if request_exit(&project, &mut prompt, target) {
        perform_exit(target, &mut next, &mut bus);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod native_picker_tests {
    use super::*;
    use bevy::ecs::system::System;

    #[test]
    fn native_open_shortcut_stays_on_the_main_thread() {
        let mut system = IntoSystem::into_system(project_shortcuts);
        system.initialize(&mut World::new());
        assert!(!system.is_send());
    }
}
