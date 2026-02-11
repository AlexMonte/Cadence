//! Project hub menu for new/open/recent project selection.

use std::path::PathBuf;

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::core::Project;
use crate::editor::AppState;
use crate::editor::camera::CANVAS_RENDER_LAYER;
use crate::editor::canvas::projection::NodeEntityMap;
use crate::editor::canvas::stacking::NodeStackingOrder;
use crate::editor::menus::Menu;
use crate::editor::state::ProjectSession;
use crate::editor::ui::FontAssets;
use crate::runtime::{RuntimeCommitReason, RuntimeCommitRequested};
use crate::store::ProjectStore;

const HUB_NODE_SIZE: Vec2 = Vec2::new(190.0, 82.0);
const HUB_RECENT_RADIUS_BASE: f32 = 320.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::ProjectHub), spawn_project_hub);
    app.add_systems(
        Update,
        animate_project_hub_nodes.run_if(in_state(Menu::ProjectHub)),
    );
    app.add_observer(on_project_hub_click);
}

#[derive(Component)]
struct ProjectHubRoot;

#[derive(Component, Clone)]
pub struct ProjectHubNode {
    action: ProjectHubAction,
}

#[derive(Component)]
struct ProjectHubDrift {
    center: Vec2,
    phase: f32,
    amplitude: Vec2,
    speed: f32,
}

#[derive(Clone)]
struct ProjectHubEntry {
    label: String,
    action: ProjectHubAction,
    position: Vec2,
    color: LinearRgba,
}

#[derive(Clone)]
enum ProjectHubAction {
    New,
    Open,
    Recent(PathBuf),
}

fn spawn_project_hub(mut commands: Commands, fonts: Res<FontAssets>) {
    let entries = build_project_hub_entries();

    let root = commands
        .spawn((
            Name::new("ProjectHubRoot"),
            ProjectHubRoot,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
            DespawnOnExit(Menu::ProjectHub),
        ))
        .id();

    for (index, entry) in entries.iter().enumerate() {
        let node_entity = commands
            .spawn((
                Name::new(format!("ProjectHubNode::{}", entry.label)),
                ProjectHubNode {
                    action: entry.action.clone(),
                },
                ProjectHubDrift {
                    center: entry.position,
                    phase: index as f32 * 0.8,
                    amplitude: Vec2::new(8.0, 6.0),
                    speed: 0.6 + (index as f32 * 0.07),
                },
                Sprite::from_color(entry.color, HUB_NODE_SIZE),
                Transform::from_translation(entry.position.extend(0.9)),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::default(),
            ))
            .id();

        let label_entity = commands
            .spawn((
                Name::new(format!("ProjectHubLabel::{}", entry.label)),
                Text2d::new(entry.label.clone()),
                TextFont {
                    font: fonts.default_font_bold.clone(),
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                Transform::from_translation(Vec3::new(0.0, 0.0, 0.1)),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
            ))
            .id();

        commands.entity(node_entity).add_child(label_entity);
        commands.entity(root).add_child(node_entity);
    }
}

fn build_project_hub_entries() -> Vec<ProjectHubEntry> {
    let mut entries = vec![
        ProjectHubEntry {
            label: "New".to_string(),
            action: ProjectHubAction::New,
            position: Vec2::ZERO,
            color: LinearRgba::rgb(0.16, 0.58, 0.36),
        },
        ProjectHubEntry {
            label: "Open".to_string(),
            action: ProjectHubAction::Open,
            position: Vec2::new(220.0, 0.0),
            color: LinearRgba::rgb(0.17, 0.38, 0.60),
        },
    ];

    let recents = ProjectStore::list_projects(".").unwrap_or_else(|err| {
        warn!("Failed to list project directories: {}", err);
        Vec::new()
    });

    let recent_count = recents.len();
    for (index, path) in recents.into_iter().enumerate() {
        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let t = (index as f32) / (recent_count.max(1) as f32);
        let angle = std::f32::consts::TAU * t + 0.7;
        let radius = HUB_RECENT_RADIUS_BASE + if index % 2 == 0 { 0.0 } else { 32.0 };
        let position = Vec2::new(angle.cos() * radius, angle.sin() * radius);

        entries.push(ProjectHubEntry {
            label,
            action: ProjectHubAction::Recent(path),
            position,
            color: LinearRgba::rgb(0.49, 0.31, 0.18),
        });
    }

    entries
}

fn on_project_hub_click(
    trigger: On<Pointer<Click>>,
    hub_nodes: Query<&ProjectHubNode>,
    mut app_state: ResMut<AppState>,
    mut node_map: ResMut<NodeEntityMap>,
    mut stacking: ResMut<NodeStackingOrder>,
    mut next_menu: ResMut<NextState<Menu>>,
    mut next_session: ResMut<NextState<ProjectSession>>,
    mut runtime_commits: MessageWriter<RuntimeCommitRequested>,
) {
    let Ok(hub_node) = hub_nodes.get(trigger.entity) else {
        return;
    };

    match &hub_node.action {
        ProjectHubAction::New => {
            apply_loaded_project(
                Project::new("Untitled".to_string()),
                &mut app_state,
                &mut node_map,
                &mut stacking,
                &mut next_menu,
                &mut next_session,
                &mut runtime_commits,
            );
        }
        ProjectHubAction::Open => {
            next_menu.set(Menu::LoadProject);
        }
        ProjectHubAction::Recent(path) => match ProjectStore::load_project(path) {
            Ok(project) => {
                apply_loaded_project(
                    project,
                    &mut app_state,
                    &mut node_map,
                    &mut stacking,
                    &mut next_menu,
                    &mut next_session,
                    &mut runtime_commits,
                );
            }
            Err(err) => {
                warn!("Failed to load project from '{}': {}", path.display(), err);
            }
        },
    }
}

fn animate_project_hub_nodes(
    time: Res<Time>,
    mut nodes: Query<(&ProjectHubDrift, &mut Transform), With<ProjectHubNode>>,
) {
    let t = time.elapsed_secs();
    for (drift, mut transform) in &mut nodes {
        let x = drift.center.x + (t * drift.speed + drift.phase).sin() * drift.amplitude.x;
        let y = drift.center.y + (t * drift.speed + drift.phase * 0.9).cos() * drift.amplitude.y;
        transform.translation.x = x;
        transform.translation.y = y;
    }
}

fn apply_loaded_project(
    project: Project,
    app_state: &mut AppState,
    node_map: &mut NodeEntityMap,
    stacking: &mut NodeStackingOrder,
    next_menu: &mut NextState<Menu>,
    next_session: &mut NextState<ProjectSession>,
    runtime_commits: &mut MessageWriter<RuntimeCommitRequested>,
) {
    app_state.current_scope = project.root_scope();
    app_state.project = project;
    node_map.clear();
    stacking.clear();
    runtime_commits.write(RuntimeCommitRequested {
        scope: app_state.current_scope,
        force: true,
        reason: RuntimeCommitReason::ProjectSwap,
    });
    next_session.set(ProjectSession::Loaded);
    next_menu.set(Menu::None);
}
