use bevy::prelude::*;
use bevy_feathers::theme::ThemedText;

use super::launch::{EditorLaunchIntent, MenuUiRoot};
use crate::adapter::persistence::load_recent_projects;
use crate::infrastructure::app::AppState;
use crate::infrastructure::ui::theme::MusaicUiTheme;

#[derive(Component)]
struct MainMenuScreen;

#[derive(Component, Clone)]
struct MenuButton(MainMenuAction);

#[derive(Clone, Debug)]
pub enum MainMenuAction {
    NewProject,
    OpenProject,
    OpenRecent(std::path::PathBuf),
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
                ThemedText,
            ));
            spawn_menu_button(menu, &theme, "New project".to_string(), MainMenuAction::NewProject);
            spawn_menu_button(
                menu,
                &theme,
                "Open project…".to_string(),
                MainMenuAction::OpenProject,
            );
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
        .spawn((
            MenuButton(action),
            Node {
                min_width: Val::Px(280.0),
                height: Val::Px(40.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(theme.spacing.xl)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme.chrome.button_bg),
            BorderColor::all(theme.chrome.border),
            Pickable::default(),
        ))
        .observe(on_menu_button_click)
        .with_children(|btn| {
            btn.spawn((Text::new(label), ThemedText));
        });
}

fn on_menu_button_click(
    mut click: On<Pointer<Click>>,
    buttons: Query<&MenuButton>,
    mut pending: ResMut<PendingMainMenuAction>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(MenuButton(action)) = buttons.get(click.event_target()) else {
        return;
    };
    pending.0 = Some(action.clone());
    click.propagate(false);
}

#[derive(Resource, Default)]
struct PendingMainMenuAction(pub Option<MainMenuAction>);

fn handle_menu_clicks(
    mut pending: ResMut<PendingMainMenuAction>,
    mut holder: ResMut<EditorLaunchIntentHolder>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(action) = pending.0.take() else {
        return;
    };
    match action {
        MainMenuAction::NewProject => {
            holder.0 = Some(EditorLaunchIntent::NewProject);
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
    if let Some(entity) = root.0.take() {
        commands.entity(entity).despawn();
    }
    for entity in screens.iter() {
        commands.entity(entity).despawn();
    }
}
