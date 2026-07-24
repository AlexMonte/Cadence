//! Right-rail inspector: panel column chrome plus the stacked panel bodies
//! projected from the application-layer [`InspectorLayout`].

pub mod panels;

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::{
    controls::{ButtonProps, button},
    theme::ThemedText,
};
use bevy_ui_widgets::{Activate, observe};

use crate::{
    application::board_view_settings::{AtomDisplayMode, AtomDisplayScope, BoardViewSettings},
    application::command::{EditorCommand, EditorCommandBus},
    application::editor::{
        EditorSession, InspectorLayout, InspectorPanelKind, inspector_panel_title,
    },
    application::pipeline::runtime::RuntimePreviewSnapshot,
    application::pipeline::scene_sync::VisibleBoardState,
    application::session::MusaicProject,
    domain::document::DocumentQueries,
};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::controls::UiTilePaletteCamera;
use crate::infrastructure::ui::tile_shell::{
    PANEL_BG, PANEL_INSET_BG, PanelBackdrop, spawn_shell_panel,
};

pub(crate) use crate::infrastructure::ui::inspector_transition::InspectorPanelHost;
use crate::infrastructure::ui::inspector_transition::{
    INSPECTOR_SLIDE_DISTANCE_FALLBACK, InspectorSlideLayer, InspectorTransitionQueue,
    SlideDirection, UiInspectorBody, UiInspectorHeader, UiInspectorViewport, finish_active_slides,
};

const RIGHT_PANEL_PERCENT: f32 = 35.0;
const INSPECTOR_MIN_WIDTH: f32 = 300.0;
const PANEL_PADDING: f32 = 14.0;

#[derive(Component)]
struct UiShellInspector;

pub(crate) fn spawn_inspector_column(
    parent: &mut ChildSpawnerCommands<'_>,
    queries: &DocumentQueries<'_>,
    layout: &InspectorLayout,
    session: &EditorSession,
    preview_snapshot: &RuntimePreviewSnapshot,
    visible: &VisibleBoardState,
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
            BackgroundColor(PANEL_BG),
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
                .with_children(|header| spawn_inspector_header(header, layout, queries))
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
                                images,
                                queries,
                                layout,
                                session,
                                preview_snapshot,
                                visible,
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

fn spawn_inspector_header(
    panel: &mut ChildSpawnerCommands<'_>,
    layout: &InspectorLayout,
    queries: &DocumentQueries<'_>,
) {
    let title = crate::application::editor::inspector_title(layout, queries);
    panel.spawn((UiText::new(title), ThemedText));
}

/// Spawn the inspector body as a stack of panels (top of stack rendered first).
///
/// Each panel gets shared chrome from
/// [`tile_shell`](crate::infrastructure::ui::tile_shell); the drawer panel
/// grows to fill remaining space, other panels hug their content.
pub(crate) fn spawn_inspector_body(
    panel: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    queries: &DocumentQueries<'_>,
    layout: &InspectorLayout,
    session: &EditorSession,
    preview_snapshot: &RuntimePreviewSnapshot,
    visible: &VisibleBoardState,
    view_settings: BoardViewSettings,
    ui_sprites: Option<&UiSpriteAssets>,
    palette_camera: Option<Entity>,
) {
    if layout.is_empty() {
        panel.spawn((UiText::new("Focus a slot or tile to inspect."), ThemedText));
        spawn_view_settings_controls(panel, view_settings);
        return;
    }

    let top_index = layout.panels.len() - 1;
    for (index, kind) in layout.panels.iter().enumerate().rev() {
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
            (panel_node, BackgroundColor(PANEL_INSET_BG)),
            images,
            ui_sprites,
            PanelBackdrop::Section,
            |content| {
                // Panels below the top carry their own caption (the shell
                // header names only the topmost panel).
                if index != top_index {
                    content.spawn((
                        UiText::new(inspector_panel_title(kind, queries)),
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
                    queries,
                    kind,
                    session,
                    preview_snapshot,
                    visible,
                    ui_sprites,
                    palette_camera,
                );
            },
        );
    }
}

fn spawn_inspector_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    images: &Assets<Image>,
    queries: &DocumentQueries<'_>,
    kind: &InspectorPanelKind,
    session: &EditorSession,
    preview_snapshot: &RuntimePreviewSnapshot,
    visible: &VisibleBoardState,
    ui_sprites: Option<&UiSpriteAssets>,
    palette_camera: Option<Entity>,
) {
    match kind {
        InspectorPanelKind::DrawerPanel { target, context } => {
            panels::drawer::spawn_tile_drawer(
                panel,
                session,
                target.as_ref(),
                *context,
                palette_camera,
            );
        }
        InspectorPanelKind::PlacementPromptPanel { target, context } => {
            panels::placement_prompt::spawn_placement_prompt_panel(
                panel,
                target.as_ref(),
                *context,
            );
        }
        InspectorPanelKind::TileInspectPanel { node } => {
            panels::tile_inspect::spawn_tile_inspect_panel(
                panel, queries, node, visible, images, ui_sprites,
            );
        }
        InspectorPanelKind::ProjectOverviewPanel { surface } => {
            panels::project_overview::spawn_project_overview_panel(panel, *surface);
        }
        InspectorPanelKind::SelectionSummaryPanel { count, primary } => {
            panels::selection::spawn_selection_summary_panel(panel, *count, primary.as_ref());
        }
        InspectorPanelKind::TimelineEventPanel { event } => {
            panels::timeline_event::spawn_timeline_event_panel(panel, *event, preview_snapshot);
        }
    }
}

pub(crate) fn process_inspector_transitions(
    mut queue: ResMut<InspectorTransitionQueue>,
    mut host: ResMut<InspectorPanelHost>,
    project: Res<MusaicProject>,
    session: Res<EditorSession>,
    preview_snapshot: Res<RuntimePreviewSnapshot>,
    visible: Res<VisibleBoardState>,
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

    let queries = DocumentQueries::new(&project.document);

    refresh_inspector_chrome(&mut commands, &host, &pending.layout, &queries);

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
                    &images,
                    &queries,
                    &pending.layout,
                    &session,
                    &preview_snapshot,
                    &visible,
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
    layout: &InspectorLayout,
    queries: &DocumentQueries<'_>,
) {
    if let Some(header) = host.header {
        commands.entity(header).despawn_children();
        commands.entity(header).with_children(|panel| {
            spawn_inspector_header(panel, layout, queries);
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
        panel.spawn((
            button(
                ButtonProps::default(),
                ViewSettingsCycleButton(scope),
                Spawn((UiText::new(format!("{label}: {mode}")), ThemedText)),
            ),
            observe(on_view_settings_cycle_activated),
        ));
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
