use bevy::prelude::*;
use bevy_ui_widgets::Activate;

use super::launch::{EditorLaunchIntent, MenuUiRoot};
#[cfg(not(target_arch = "wasm32"))]
use crate::adapter::persistence::load_recent_projects;
use crate::infrastructure::app::AppState;
use crate::infrastructure::ui::theme::MusaicUiTheme;

#[derive(Component)]
struct MainMenuScreen;

#[derive(Component)]
struct MenuMessage;

#[derive(Component, Clone)]
struct MenuButton(MainMenuAction);

#[derive(Clone, Debug)]
pub enum MainMenuAction {
    FirstLoop,
    NewProject,
    OpenExample,
    OpenProject,
    #[cfg(not(target_arch = "wasm32"))]
    OpenRecent(std::path::PathBuf),
    #[cfg(not(target_arch = "wasm32"))]
    Recover(std::path::PathBuf),
}

pub struct MainMenuUiPlugin;

impl Plugin for MainMenuUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorLaunchIntentHolder>()
            .init_resource::<MenuUiRoot>()
            .init_resource::<PendingMainMenuAction>()
            .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(AppState::MainMenu), despawn_main_menu)
            .add_systems(
                Update,
                handle_menu_clicks.run_if(in_state(AppState::MainMenu)),
            );
    }
}

/// Wrapper so we can `init_resource` before the first menu action.
#[derive(Resource, Debug, Clone, Default)]
pub struct EditorLaunchIntentHolder(pub Option<EditorLaunchIntent>);

impl EditorLaunchIntentHolder {
    pub fn take(&mut self) -> Option<EditorLaunchIntent> {
        self.0.take()
    }
}

fn spawn_main_menu(
    mut commands: Commands,
    mut root: ResMut<MenuUiRoot>,
    theme: Res<MusaicUiTheme>,
) {
    let screen = commands
        .spawn((
            MainMenuScreen,
            bevy::input_focus::tab_navigation::TabGroup::new(0),
            TextColor(theme.chrome.text_main),
            bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(theme.spacing.lg),
                padding: UiRect::all(Val::Px(theme.spacing.xl * 2.0)),
                ..default()
            },
            BackgroundColor(theme.chrome.window_bg),
        ))
        .with_children(|menu| {
            menu.spawn((
                Text::new("Musaic"),
                TextFont {
                    font_size: theme.typography.brand,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            menu.spawn((
                MenuMessage,
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
            ));
            spawn_menu_button(
                menu,
                &theme,
                "Start a first loop".into(),
                MainMenuAction::FirstLoop,
            );
            spawn_menu_button(
                menu,
                &theme,
                "Open example".to_string(),
                MainMenuAction::OpenExample,
            );
            spawn_menu_button(
                menu,
                &theme,
                "New project".to_string(),
                MainMenuAction::NewProject,
            );
            spawn_menu_button(
                menu,
                &theme,
                "Open project…".to_string(),
                MainMenuAction::OpenProject,
            );
            #[cfg(not(target_arch = "wasm32"))]
            for recovery in crate::adapter::persistence::recovery::list_recoveries()
                .into_iter()
                .take(3)
            {
                spawn_menu_button(
                    menu,
                    &theme,
                    format!("Recover: {}", recovery.name),
                    MainMenuAction::Recover(recovery.path),
                );
            }
            #[cfg(not(target_arch = "wasm32"))]
            for path in load_recent_projects().into_iter().take(6) {
                let label = format!(
                    "Recent: {}",
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                );
                spawn_menu_button(menu, &theme, label, MainMenuAction::OpenRecent(path));
            }
        })
        .id();
    root.0 = Some(screen);
}

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    label: String,
    action: MainMenuAction,
) {
    parent
        .spawn(crate::infrastructure::ui::widgets::musaic_chrome_button(
            theme,
            label,
            MenuButton(action),
        ))
        .insert(Node {
            min_width: px(280),
            height: px(40),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(px(theme.spacing.xl)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(theme.radii.md)),
            ..default()
        })
        .observe(on_menu_button_click);
}

fn on_menu_button_click(
    click: On<Activate>,
    buttons: Query<&MenuButton>,
    mut pending: ResMut<PendingMainMenuAction>,
) {
    let Ok(MenuButton(action)) = buttons.get(click.entity) else {
        return;
    };
    pending.0 = Some(action.clone());
}

#[derive(Resource, Default)]
struct PendingMainMenuAction(pub Option<MainMenuAction>);

fn handle_menu_clicks(
    mut pending: ResMut<PendingMainMenuAction>,
    mut holder: ResMut<EditorLaunchIntentHolder>,
    mut next: ResMut<NextState<AppState>>,
    #[cfg(not(target_arch = "wasm32"))] mut diagnostics: ResMut<
        crate::infrastructure::diagnostics::DiagnosticStore,
    >,
    #[cfg(not(target_arch = "wasm32"))] mut message: Query<&mut Text, With<MenuMessage>>,
) {
    let Some(action) = pending.0.take() else {
        return;
    };
    match action {
        MainMenuAction::FirstLoop => {
            holder.0 = Some(EditorLaunchIntent::FirstLoop);
            next.set(AppState::Editor);
        }
        MainMenuAction::NewProject => {
            holder.0 = Some(EditorLaunchIntent::NewProject);
            next.set(AppState::Editor);
        }
        MainMenuAction::OpenExample => {
            holder.0 = Some(EditorLaunchIntent::Example);
            next.set(AppState::Editor);
        }
        MainMenuAction::OpenProject => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Musaic project", &["json"])
                    .pick_file()
                {
                    holder.0 = Some(EditorLaunchIntent::OpenProject(path));
                    next.set(AppState::Editor);
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                crate::adapter::persistence::wasm_io::request_open_project();
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        MainMenuAction::Recover(path) => {
            match crate::adapter::persistence::recovery::load_recovery(&path) {
                Ok(project) => {
                    holder.0 = Some(EditorLaunchIntent::LoadedProject(Box::new(project)));
                    next.set(AppState::Editor);
                }
                Err(error) => {
                    for mut text in &mut message {
                        **text = format!("Could not recover project: {error}");
                    }
                    use crate::infrastructure::diagnostics::*;
                    diagnostics.push(LayeredDiagnostic {
                        phase: DiagnosticPhase::Transaction,
                        diagnostic: AppDiagnostic::Host(HostDiagnostic::TransactionRejected {
                            message: format!("Could not recover project: {error}"),
                        }),
                    });
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        MainMenuAction::OpenRecent(path) => {
            holder.0 = Some(EditorLaunchIntent::OpenProject(path));
            next.set(AppState::Editor);
        }
    }
}

fn despawn_main_menu(
    mut commands: Commands,
    mut root: ResMut<MenuUiRoot>,
    screens: Query<Entity, With<MainMenuScreen>>,
) {
    root.0 = None;
    for entity in screens.iter() {
        commands.entity(entity).despawn();
    }
}
