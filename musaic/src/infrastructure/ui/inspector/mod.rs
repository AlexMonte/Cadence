//! Right-rail inspector: panel column chrome plus the stacked panel bodies
//! projected from [`EditorUiProjection`] paint DTOs.

pub mod panels;

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;
use bevy_ui_widgets::Activate;

use crate::{
    application::board_view_settings::{AtomDisplayMode, AtomDisplayScope, BoardViewSettings},
    application::command::{EditorCommand, EditorCommandBus},
    application::editor::{InspectorLayout, InspectorPanelKind},
    application::pipeline::runtime::RuntimePreviewSnapshot,
    application::pipeline::ui_projection::{
        EditorUiProjection, InspectorPaint, InspectorPanelPaint,
    },
};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::controls::UiTilePaletteCamera;
use crate::infrastructure::ui::theme::{
    INSPECTOR_SLIDE_DISTANCE_FALLBACK, InspectorPanelHost, InspectorSlideLayer,
    InspectorTransitionQueue, MusaicUiTheme, SlideDirection, UiInspectorBody, UiInspectorHeader,
    UiInspectorViewport, finish_active_slides,
};
use crate::infrastructure::ui::widgets::{PanelBackdrop, musaic_button, spawn_shell_panel};

const RIGHT_PANEL_PERCENT: f32 = 35.0;
const INSPECTOR_MIN_WIDTH: f32 = 300.0;
const PANEL_PADDING: f32 = 14.0;

#[derive(Component)]
struct UiShellInspector;

pub(crate) fn spawn_inspector_column(
    parent: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    layout: &InspectorLayout,
    paint: &InspectorPaint,
    preview_snapshot: &RuntimePreviewSnapshot,
    palette_camera: Option<Entity>,
    host: &mut InspectorPanelHost,
    sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
    view_settings: BoardViewSettings,
) {
    spawn_shell_panel(
        parent,
        (
            UiShellInspector,
            Node {
                width: percent(RIGHT_PANEL_PERCENT),
                height: percent(100),
                flex_shrink: 0.0,
                min_width: px(INSPECTOR_MIN_WIDTH),
                padding: UiRect::all(px(PANEL_PADDING + 4.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(theme.chrome.panel_bg),
        ),
        images,
        sprites,
        PanelBackdrop::Panel,
        |panel| {
            let header_id = panel
                .spawn((
                    UiInspectorHeader,
                    Node {
                        width: percent(100),
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ))
                .with_children(|header| spawn_inspector_header(header, paint))
                .id();
            host.header = Some(header_id);

            let viewport_id = panel
                .spawn((
                    UiInspectorViewport,
                    Node {
                        width: percent(100),
                        flex_grow: 1.0,
                        min_height: px(0),
                        position_type: PositionType::Relative,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ))
                .with_children(|viewport| {
                    let body_id = viewport
                        .spawn(inspector_body_bundle())
                        .with_children(|body| {
                            spawn_inspector_body(
                                body,
                                theme,
                                images,
                                layout,
                                paint,
                                preview_snapshot,
                                view_settings,
                                sprites,
                                palette_camera,
                            );
                        })
                        .id();
                    host.active_body = Some(body_id);
                })
                .id();
            host.viewport = Some(viewport_id);
        },
    );
}

fn inspector_body_bundle() -> impl Bundle {
    (
        UiInspectorBody,
        Node {
            width: percent(100),
            height: percent(100),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            overflow: Overflow::clip(),
            ..default()
        },
        UiTransform::IDENTITY,
    )
}

fn spawn_inspector_header(panel: &mut ChildSpawnerCommands<'_>, paint: &InspectorPaint) {
    panel.spawn((UiText::new(paint.header_title.clone()), ThemedText));
}

/// Spawn the inspector body as a stack of panels (top of stack rendered first).
///
/// Each panel gets shared chrome from
/// [`widgets`](crate::infrastructure::ui::widgets); the drawer panel
/// grows to fill remaining space, other panels hug their content.
pub(crate) fn spawn_inspector_body(
    panel: &mut ChildSpawnerCommands<'_>,
    theme: &MusaicUiTheme,
    images: &Assets<Image>,
    layout: &InspectorLayout,
    paint: &InspectorPaint,
    preview_snapshot: &RuntimePreviewSnapshot,
    view_settings: BoardViewSettings,
    ui_sprites: Option<&UiSpriteAssets>,
    palette_camera: Option<Entity>,
) {
    if layout.is_empty() {
        panel.spawn((UiText::new("Focus a slot or tile to inspect."), ThemedText));
        spawn_view_settings_controls(panel, view_settings);
        return;
    }

    debug_assert_eq!(
        layout.panels.len(),
        paint.panels.len(),
        "inspector layout and paint must stay 1:1"
    );

    let top_index = layout.panels.len() - 1;
    for (index, (kind, panel_paint)) in layout
        .panels
        .iter()
        .zip(paint.panels.iter())
        .enumerate()
        .rev()
    {
        let grows = matches!(kind, InspectorPanelKind::DrawerPanel { .. });
        let panel_node = Node {
            width: percent(100),
            flex_grow: if grows { 1.0 } else { 0.0 },
            flex_shrink: 0.0,
            min_height: px(0.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(6.0),
            padding: UiRect::all(px(8.0)),
            position_type: PositionType::Relative,
            ..default()
        };
        spawn_shell_panel(
            panel,
            (panel_node, BackgroundColor(theme.chrome.panel_inset)),
            images,
            ui_sprites,
            PanelBackdrop::Section,
            |content| {
                // Panels below the top carry their own caption (the shell
                // header names only the topmost panel).
                if index != top_index {
                    content.spawn((
                        UiText::new(panel_paint_title(panel_paint)),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        ThemedText,
                    ));
                }
                spawn_inspector_panel(
                    content,
                    images,
                    kind,
                    panel_paint,
                    preview_snapshot,
                    ui_sprites,
                    palette_camera,
                );
            },
        );
    }
}

fn panel_paint_title(paint: &InspectorPanelPaint) -> String {
    match paint {
        InspectorPanelPaint::Drawer { panel_title, .. }
        | InspectorPanelPaint::PlacementPrompt { panel_title, .. }
        | InspectorPanelPaint::TileInspect { panel_title, .. }
        | InspectorPanelPaint::SelectionSummary { panel_title, .. }
        | InspectorPanelPaint::TimelineEvent { panel_title, .. }
        | InspectorPanelPaint::ProjectOverview { panel_title, .. } => panel_title.clone(),
    }
}

fn spawn_inspector_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    kind: &InspectorPanelKind,
    paint: &InspectorPanelPaint,
    preview_snapshot: &RuntimePreviewSnapshot,
    ui_sprites: Option<&UiSpriteAssets>,
    palette_camera: Option<Entity>,
) {
    match (kind, paint) {
        (
            InspectorPanelKind::DrawerPanel { .. },
            InspectorPanelPaint::Drawer {
                target_label,
                armed_label,
                ..
            },
        ) => {
            panels::drawer::spawn_tile_drawer(
                panel,
                target_label.as_deref(),
                armed_label.as_deref(),
                palette_camera,
            );
        }
        (
            InspectorPanelKind::PlacementPromptPanel { context, .. },
            InspectorPanelPaint::PlacementPrompt { target_label, .. },
        ) => {
            panels::placement_prompt::spawn_placement_prompt_panel(
                panel,
                target_label.as_deref(),
                *context,
            );
        }
        (
            InspectorPanelKind::TileInspectPanel { node },
            InspectorPanelPaint::TileInspect {
                description, ports, ..
            },
        ) => {
            panels::tile_inspect::spawn_tile_inspect_panel(
                panel,
                node,
                description,
                ports.as_ref(),
                images,
                ui_sprites,
            );
        }
        (
            InspectorPanelKind::ProjectOverviewPanel { surface },
            InspectorPanelPaint::ProjectOverview { .. },
        ) => {
            panels::project_overview::spawn_project_overview_panel(panel, *surface);
        }
        (
            InspectorPanelKind::SelectionSummaryPanel { count, primary },
            InspectorPanelPaint::SelectionSummary { .. },
        ) => {
            panels::selection::spawn_selection_summary_panel(panel, *count, primary.as_ref());
        }
        (
            InspectorPanelKind::TimelineEventPanel { event },
            InspectorPanelPaint::TimelineEvent { .. },
        ) => {
            panels::timeline_event::spawn_timeline_event_panel(panel, *event, preview_snapshot);
        }
        _ => {
            panic!("inspector layout/paint variant mismatch — projection writer must stay 1:1");
        }
    }
}

pub(crate) fn process_inspector_transitions(
    mut queue: ResMut<InspectorTransitionQueue>,
    mut host: ResMut<InspectorPanelHost>,
    theme: Res<MusaicUiTheme>,
    projection: Res<EditorUiProjection>,
    preview_snapshot: Res<RuntimePreviewSnapshot>,
    view_settings: Res<BoardViewSettings>,
    ui_sprites: Option<Res<UiSpriteAssets>>,
    images: Res<Assets<Image>>,
    palette_camera: Query<'_, '_, Entity, With<UiTilePaletteCamera>>,
    viewport_nodes: Query<&ComputedNode, With<UiInspectorViewport>>,
    slides: Query<(Entity, &InspectorSlideLayer)>,
    mut commands: Commands,
) {
    let Some(pending) = queue.pending.take() else {
        return;
    };
    let Some(viewport) = host.viewport else {
        return;
    };

    refresh_inspector_chrome(&mut commands, &host, &projection.inspector_paint);

    let distance = viewport_nodes
        .get(viewport)
        .ok()
        .map(|node| node.size.x)
        .filter(|width| *width > 1.0)
        .unwrap_or(INSPECTOR_SLIDE_DISTANCE_FALLBACK);

    finish_active_slides(&mut commands, &slides, &mut host);

    if let Some(active) = host.active_body {
        if pending.animate {
            commands.entity(active).insert((
                UiTransform::from_translation(Val2::ZERO),
                InspectorSlideLayer {
                    direction: SlideDirection::Exit,
                    elapsed: 0.0,
                    distance,
                },
            ));
        } else {
            commands.entity(active).despawn();
            host.active_body = None;
        }
    }

    let palette_camera = palette_camera.iter().next();

    let mut incoming = Entity::PLACEHOLDER;
    commands.entity(viewport).with_children(|viewport| {
        incoming = viewport
            .spawn(inspector_body_bundle())
            .with_children(|body| {
                spawn_inspector_body(
                    body,
                    &theme,
                    &images,
                    &pending.layout,
                    &projection.inspector_paint,
                    &preview_snapshot,
                    *view_settings,
                    ui_sprites.as_deref(),
                    palette_camera,
                );
            })
            .id();
    });

    if pending.animate {
        commands.entity(incoming).insert((
            UiTransform::from_translation(Val2::px(-distance, 0.0)),
            InspectorSlideLayer {
                direction: SlideDirection::Enter,
                elapsed: 0.0,
                distance,
            },
        ));
    } else {
        host.active_body = Some(incoming);
    }
    host.displayed_layout = Some(pending.layout);
}

fn refresh_inspector_chrome(
    commands: &mut Commands,
    host: &InspectorPanelHost,
    paint: &InspectorPaint,
) {
    if let Some(header) = host.header {
        commands.entity(header).despawn_children();
        commands.entity(header).with_children(|panel| {
            spawn_inspector_header(panel, paint);
        });
    }
}

#[derive(Component, Clone, Copy)]
pub(crate) struct ViewSettingsCycleButton(pub AtomDisplayScope);

fn spawn_view_settings_controls(panel: &mut ChildSpawnerCommands<'_>, settings: BoardViewSettings) {
    panel.spawn((UiText::new("View"), ThemedText));
    for (scope, label) in [
        (AtomDisplayScope::RootBoard, "Root board"),
        (
            AtomDisplayScope::ContainerPreviewOnRoot,
            "Container preview",
        ),
        (AtomDisplayScope::ContainerInterior, "Container interior"),
    ] {
        let mode = settings.label_for_scope(scope);
        panel
            .spawn(musaic_button(
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                ViewSettingsCycleButton(scope),
                format!("{label}: {mode}"),
            ))
            .observe(on_view_settings_cycle_activated);
    }
}

pub(crate) fn on_view_settings_cycle_activated(
    activate: On<'_, '_, Activate>,
    query: Query<'_, '_, &ViewSettingsCycleButton>,
    settings: Res<BoardViewSettings>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = query.get(activate.entity) else {
        return;
    };
    let scope = button.0;
    let next = match settings.mode_for_scope(scope) {
        AtomDisplayMode::Stack => AtomDisplayMode::CompoundTile,
        AtomDisplayMode::CompoundTile => AtomDisplayMode::Stack,
    };
    bus.write(EditorCommandBus(EditorCommand::SetViewSettings {
        scope,
        mode: next,
    }));
}
