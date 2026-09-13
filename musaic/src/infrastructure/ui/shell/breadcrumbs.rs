//! Surface navigation above the board and a quiet, live status footer.

use bevy::{prelude::*, ui::widget::Text as UiText};

use crate::{
    application::command::EditorCommand, application::pipeline::ui_projection::BreadcrumbPaint,
    domain::board::BoardSurfaceId,
};

use super::menu::{
    InspectorButtonAction, MenuAudioLabel, MenuClockLabel, on_inspector_button_activated,
};
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::widgets::musaic_button;

pub(crate) fn spawn_board_breadcrumbs(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    breadcrumbs: &BreadcrumbPaint,
    active_surface: Option<BoardSurfaceId>,
) {
    parent
        .spawn((
            Node {
                width: percent(100),
                min_height: px(45.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                flex_wrap: FlexWrap::Wrap,
                padding: UiRect::axes(px(22.0), px(8.0)),
                column_gap: px(12.0),
                row_gap: px(4.0),
                ..default()
            },
            BackgroundColor(theme.chrome.window_bg),
        ))
        .with_children(|header| {
            header
                .spawn(Node {
                    min_width: px(0),
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    column_gap: px(9.0),
                    row_gap: px(4.0),
                    ..default()
                })
                .with_children(|trail| {
                    for (index, (label, surface)) in breadcrumbs.entries.iter().enumerate() {
                        if index > 0 {
                            trail.spawn((
                                UiText::new("/"),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(theme.chrome.text_dim),
                            ));
                        }
                        let selected = active_surface == Some(*surface);
                        trail
                            .spawn(musaic_button(
                                Node {
                                    min_height: px(22.0),
                                    align_items: AlignItems::Center,
                                    padding: UiRect::axes(px(2.0), px(2.0)),
                                    ..default()
                                },
                                InspectorButtonAction(EditorCommand::NavigateToSurface {
                                    surface: *surface,
                                }),
                                label,
                            ))
                            .insert((
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(if selected {
                                    theme.chrome.text_main
                                } else {
                                    theme.chrome.text_dim
                                }),
                            ))
                            .observe(on_inspector_button_activated);
                    }
                });
            header.spawn((
                UiText::new("Tessera"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(theme.chrome.text_dim),
            ));
        });
}

pub(crate) fn spawn_shell_footer(parent: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    parent
        .spawn((
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(22), px(7)),
                border: UiRect::top(px(1)),
                row_gap: px(4),
                ..default()
            },
            BackgroundColor(theme.chrome.panel_inset),
            BorderColor::all(theme.chrome.border),
        ))
        .with_children(|footer| {
            footer
                .spawn(Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(14),
                    row_gap: px(4),
                    ..default()
                })
                .with_children(|messages| {
                    messages.spawn((
                        crate::infrastructure::ui::keyboard_help::KeyboardStatus,
                        UiText::new("Standard · Navigate · F1 keyboard help"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.chrome.text_dim),
                    ));
                    messages
                        .spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: px(16),
                            row_gap: px(4),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|status| {
                            crate::infrastructure::ui::diagnostics_panel::spawn_playback_status(
                                status, theme,
                            );
                            status.spawn((
                                MenuClockLabel,
                                UiText::new("Cycle 1"),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(theme.chrome.text_dim),
                            ));
                            status.spawn((
                                MenuAudioLabel,
                                UiText::new("Audio starting"),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(theme.chrome.text_dim),
                            ));
                        });
                });
            // Keep actionable audio controls at the bottom. Focus/status text can
            // wrap above without moving a pressed button before its release.
            crate::infrastructure::ui::audio_meter::spawn(footer, theme);
        });
}
