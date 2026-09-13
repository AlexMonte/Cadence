use bevy::prelude::*;

use crate::application::pipeline::ui_projection::{EditorUiProjection, UiDirty};
use crate::infrastructure::ui::theme::MusaicUiTheme;

#[derive(Component)]
pub struct DiagnosticsBanner;

#[derive(Component)]
pub(crate) struct PlaybackStatus;

pub(crate) fn spawn_playback_status(parent: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    let mut accessible = accesskit::Node::new(accesskit::Role::Status);
    accessible.set_live(accesskit::Live::Polite);
    accessible.set_live_atomic();
    parent.spawn((
        PlaybackStatus,
        bevy::a11y::AccessibilityNode(accessible),
        Text::new("Checking edits…"),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(theme.chrome.text_dim),
    ));
}

pub(crate) fn sync_playback_status(
    dirty: Res<UiDirty>,
    projection: Res<EditorUiProjection>,
    mut labels: Query<(&mut Text, &mut bevy::a11y::AccessibilityNode), With<PlaybackStatus>>,
) {
    for (mut text, mut accessible) in &mut labels {
        if !dirty.diagnostics && !text.is_added() {
            continue;
        }
        let feedback = &projection.playback_feedback;
        let summary = feedback.summary();
        if text.0 != summary {
            text.0.clone_from(&summary);
        }
        if accessible.label() != Some(summary.as_str()) {
            accessible.set_label(summary);
        }
        let detail = feedback.detail.as_str();
        if accessible.description() != Some(detail) {
            accessible.set_description(detail);
        }
    }
}

pub fn sync_diagnostics_banner(
    dirty: Res<'_, UiDirty>,
    projection: Res<'_, EditorUiProjection>,
    mut banner: Query<(&mut Text, &mut Visibility), With<DiagnosticsBanner>>,
) {
    if !dirty.diagnostics {
        return;
    }
    let summary = &projection.diagnostics_summary;
    for (mut text, mut visibility) in banner.iter_mut() {
        if summary.is_empty() {
            *visibility = Visibility::Hidden;
        } else {
            *visibility = Visibility::Visible;
            **text = summary.clone();
        }
    }
}

pub fn spawn_diagnostics_banner(mut commands: Commands, theme: Res<MusaicUiTheme>) {
    // Only actionable errors appear above the status footer.
    commands.spawn((
        DiagnosticsBanner,
        DespawnOnExit(crate::infrastructure::app::AppState::Editor),
        Visibility::Hidden,
        Text::new(""),
        TextColor(theme.chrome.text_main),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            right: Val::Px(12.0),
            max_width: Val::Percent(60.0),
            padding: UiRect::all(Val::Px(theme.spacing.sm)),
            ..default()
        },
        BackgroundColor(theme.chrome.diagnostics_bg),
        GlobalZIndex(200),
    ));
}
