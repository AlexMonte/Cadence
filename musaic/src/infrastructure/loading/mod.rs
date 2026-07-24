//! Loading state UI while load_up prepares Editor resources.

use bevy::prelude::*;
use bevy_feathers::theme::ThemedText;
use load_up::prelude::{ActiveStateLoad, StateLoadBlocked, StateLoadReady};

use crate::infrastructure::app::AppState;

#[derive(Component)]
struct LoadingScreenRoot;

pub struct LoadingUiPlugin;

impl Plugin for LoadingUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingStatusText>()
            .add_systems(OnEnter(AppState::Loading), spawn_loading_screen)
            .add_systems(OnExit(AppState::Loading), despawn_loading_screen)
            .add_systems(
                Update,
                (update_loading_status, log_load_blocked).run_if(in_state(AppState::Loading)),
            )
            .add_observer(on_state_load_ready)
            .add_observer(on_state_load_blocked);
    }
}

#[derive(Resource, Default)]
struct LoadingStatusText {
    message: String,
}

fn spawn_loading_screen(mut commands: Commands, mut status: ResMut<LoadingStatusText>) {
    status.message = "Loading editor…".into(); // shown while load_up holds Loading before Editor
    commands
        .spawn((
            LoadingScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Musaic"),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                ThemedText,
            ));
            root.spawn((
                LoadingStatusLabel,
                Text::new(status.message.clone()),
                ThemedText,
            ));
        });
}

#[derive(Component)]
struct LoadingStatusLabel;

fn despawn_loading_screen(mut commands: Commands, roots: Query<Entity, With<LoadingScreenRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

fn update_loading_status(
    active: Option<Res<ActiveStateLoad<AppState>>>,
    mut status: ResMut<LoadingStatusText>,
    mut labels: Query<&mut Text, With<LoadingStatusLabel>>,
) {
    if let Some(active) = active {
        status.message = match active.last_report {
            Some(load_up::state::ActiveStateLoadReport::Blocked {
                resource,
                dependency,
            }) => format!(
                "Failed to load assets.\nresource {:?}, dependency {:?}\nCheck the log for details.",
                resource, dependency
            ),
            _ => format!("Loading editor… ({:?})", active.target),
        };
    }
    for mut text in labels.iter_mut() {
        **text = status.message.clone();
    }
}

fn log_load_blocked(active: Option<Res<ActiveStateLoad<AppState>>>) {
    let Some(active) = active else {
        return;
    };
    if let Some(load_up::state::ActiveStateLoadReport::Blocked {
        resource,
        dependency,
    }) = active.last_report
    {
        bevy::log::error!(
            "load_up blocked entering {:?}: resource {:?}, dependency {:?}",
            active.target,
            resource,
            dependency
        );
    }
}

fn on_state_load_ready(trigger: On<StateLoadReady<AppState>>) {
    bevy::log::info!("load_up ready for {:?}", trigger.target);
}

fn on_state_load_blocked(trigger: On<StateLoadBlocked<AppState>>) {
    bevy::log::error!(
        "load_up blocked {:?}: resource {:?}, dependency {:?}",
        trigger.target,
        trigger.resource,
        trigger.dependency
    );
}
