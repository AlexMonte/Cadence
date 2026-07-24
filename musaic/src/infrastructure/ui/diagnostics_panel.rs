use bevy::prelude::*;

use crate::application::pipeline::ui_projection::{EditorUiProjection, UiDirty};
use crate::infrastructure::ui::theme::MusaicUiTheme;

#[derive(Component)]
pub struct DiagnosticsBanner;

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
    // Anchored above the bottom tab bar, right-aligned, so it never covers
    // the breadcrumbs.
    commands.spawn((
        DiagnosticsBanner,
        Visibility::Hidden,
        Text::new("No diagnostics"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(58.0),
            right: Val::Px(12.0),
            max_width: Val::Percent(60.0),
            padding: UiRect::all(Val::Px(theme.spacing.sm)),
            ..default()
        },
        BackgroundColor(theme.chrome.diagnostics_bg),
        GlobalZIndex(200),
    ));
}
