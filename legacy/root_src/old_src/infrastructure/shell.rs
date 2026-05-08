use bevy::prelude::*;

use crate::adapter::platform::{CadencePlatformTarget, PlatformCapabilities};
use crate::application::board::{BoardFocus, BoardStore, SelectionStore};
use crate::application::editor::EditorSnapshot;
use crate::application::board::runtime::LastBoardError;
use crate::infrastructure::theme::CadenceTheme;

#[derive(Component)]
struct TopbarText;
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct InspectorText;
#[derive(Component)]
struct ActivityText;

pub struct ShellUiPlugin;

impl Plugin for ShellUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_shell_ui)
            .add_systems(Update, refresh_shell_ui);
    }
}

fn spawn_shell_ui(mut commands: Commands, theme: Res<CadenceTheme>) {
    commands
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    right: Val::Px(16.0),
                    top: Val::Px(16.0),
                    height: Val::Px(72.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.panel_background),
                BorderColor::all(theme.panel_border),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Cadence"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(theme.text),
                    TopbarText,
                ));
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(16.0),
                    top: Val::Px(104.0),
                    width: Val::Px(320.0),
                    bottom: Val::Px(64.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.panel_background),
                BorderColor::all(theme.panel_border),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Inspector"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(theme.text),
                    InspectorText,
                ));
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    right: Val::Px(352.0),
                    bottom: Val::Px(16.0),
                    height: Val::Px(40.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.panel_background),
                BorderColor::all(theme.panel_border),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Status"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(theme.text),
                    StatusText,
                ));
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    width: Val::Px(320.0),
                    top: Val::Px(104.0),
                    bottom: Val::Px(64.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.panel_background),
                BorderColor::all(theme.panel_border),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Activity"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(theme.text),
                    ActivityText,
                ));
            });
        });
}

fn refresh_shell_ui(
    snapshot: Res<EditorSnapshot>,
    board_store: Res<BoardStore>,
    board_focus: Res<BoardFocus>,
    selection_store: Res<SelectionStore>,
    board_error: Res<LastBoardError>,
    platform: Res<PlatformCapabilities>,
    mut text_sets: ParamSet<(
        Query<&mut Text, With<TopbarText>>,
        Query<&mut Text, With<StatusText>>,
        Query<&mut Text, With<InspectorText>>,
        Query<&mut Text, With<ActivityText>>,
    )>,
) {
    let platform_label = match platform.target {
        CadencePlatformTarget::NativeDesktop => "native",
        CadencePlatformTarget::BrowserWasm => "wasm",
    };

    if let Ok(mut text) = text_sets.p0().single_mut() {
        *text = Text::new(format!(
            "Cadence | {} | {} nodes | {} edges",
            platform_label,
            snapshot.sync.graph.nodes.len(),
            snapshot.sync.graph.edges.len()
        ));
    }

    if let Ok(mut text) = text_sets.p1().single_mut() {
        let viewport = board_focus.active_viewport();
        *text = Text::new(format!(
            "Selection: {} | Zoom: {:.2} | Pan: ({:.0}, {:.0}) | Runtime program: {:?}",
            selection_store
                .selected_slot(&board_focus.active_board)
                .map(|pos| format!("{},{}", pos.col, pos.row))
                .unwrap_or_else(|| "none".to_string()),
            viewport.zoom,
            viewport.pan.x,
            viewport.pan.y,
            snapshot.sync.runtime_status.program_state
        ));
    }

    if let Ok(mut text) = text_sets.p2().single_mut() {
        let selected = selection_store.selected_slot(&board_focus.active_board).and_then(|slot| {
            let surface = board_store.surface(&board_focus.active_board)?;
            let tile_id = surface.occupancy.get(slot)?;
            let tile = board_store.tile(tile_id)?;
            Some(format!("{} at {},{}", tile.piece_id, slot.col, slot.row))
        });
        *text = Text::new(format!(
            "Inspector\n{}\n\nGraph: {}\nPreview playable: {}\nWasm note: {}\nBoard error: {}",
            selected.unwrap_or_else(|| "No tile selected".to_string()),
            snapshot.sync.graph.name,
            snapshot.sync.project_preview.can_play,
            platform.notes,
            board_error
                .0
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ));
    }

    if let Ok(mut text) = text_sets.p3().single_mut() {
        let latest = snapshot
            .sync
            .diagnostics
            .entries
            .last()
            .map(|entry| format!("{}: {}", entry.kind, entry.message))
            .unwrap_or_else(|| "No diagnostics yet".to_string());
        *text = Text::new(format!(
            "Activity\nHistory: undo={} redo={}\nLatest diagnostic: {}",
            snapshot.sync.history_status.can_undo,
            snapshot.sync.history_status.can_redo,
            latest
        ));
    }
}
