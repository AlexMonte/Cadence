//! Top-level editor menubar and status bar interactions.

use std::collections::HashMap;

use bevy::camera_controller::pan_camera::PanCamera;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::{Layout, ModelNode, NodeId, NodeKind, ScopeId};
use crate::editor::AppState;
use crate::editor::camera::CanvasCamera;
use crate::editor::ecs::HierarchyAccess;
use crate::editor::gestures::pointer::{ContextMenuIntent, ContextMenuTarget, LayoutChanged};
use crate::editor::input::Modifiers;
use crate::editor::menus::{Menu, TextDraftState};
use crate::editor::screens::editor::InEditor;
use crate::editor::semantics::{SelectionIntent, SelectionState};
use crate::editor::state::EditorReady;
use crate::editor::ui::background::nine_slice_background;
use crate::editor::ui::buttons::button_base;
use crate::editor::ui::styles::ButtonImages;
use crate::editor::ui::{
    BORDER_4, BORDER_12, FontAssets, UiButtonLongAssets, UiPanelAssets, UiStyles, label_with_style,
    ui_root,
};
use crate::runtime::protocol::{RuntimeIntent, RuntimeState};

#[derive(Component)]
/// Root overlay entity that owns menubar and status bar UI.
pub struct EditorUiRoot;

#[derive(Component)]
/// Top menubar container shown while the editor is active.
pub struct EditorMenubar;

#[derive(Component)]
/// Bottom status bar container for editor/runtime diagnostics.
pub struct EditorStatusBar;

#[derive(Component)]
/// Marker for the breadcrumb label row in the status bar.
pub struct BreadcrumbsLabel;

#[derive(Component, Clone, Copy)]
struct BreadcrumbSegment {
    index: usize,
    action: Option<BreadcrumbAction>,
}

#[derive(Component, Clone, Copy)]
struct BreadcrumbSeparator {
    index: usize,
}

#[derive(Component)]
/// Label that shows currently selected node and selection count.
pub struct StatusLabel;

#[derive(Component)]
/// Label that shows runtime playback and validation status.
pub struct RuntimeStatusLabel;

#[derive(Component)]
/// Label that shows cursor coordinates and camera zoom.
pub struct CursorZoomLabel;

#[derive(Component)]
struct RuntimeTempoValueLabel;

#[derive(Component)]
struct NodeTypeDropdown;

#[derive(Component)]
struct NodeTypeDropdownToggle;

#[derive(Component)]
struct OpenTextDraftButton;

#[derive(Component, Clone, Copy)]
struct NodeTypeMenuItem {
    preset_index: usize,
}

#[derive(Component, Clone, Copy)]
struct RuntimeControlButton {
    action: RuntimeControlAction,
}

#[derive(Clone, Copy)]
enum RuntimeControlAction {
    Play,
    Stop,
    TempoDown,
    TempoUp,
}

#[derive(Clone, Copy)]
enum BreadcrumbAction {
    Scope(ScopeId),
    Node(NodeId),
}

const TEMPO_MIN_CPM: f32 = 30.0;
const TEMPO_MAX_CPM: f32 = 300.0;
const TEMPO_STEP_CPM: f32 = 5.0;
const STATUS_BAR_HEIGHT: f32 = 18.0;
const STATUS_TEXT_SIZE: f32 = 10.0;
const MAX_BREADCRUMB_SEGMENTS: usize = 6;

#[derive(Clone, Debug)]
/// Node template metadata used by the Add Node dropdown.
pub struct NodePreset {
    pub id: String,
    pub label: String,
    pub kind: NodeKind,
}

#[derive(Resource, Clone, Debug)]
/// Runtime catalog of node presets exposed in editor UI.
pub struct NodePresetCatalog {
    presets: Vec<NodePreset>,
}

impl Default for NodePresetCatalog {
    fn default() -> Self {
        Self {
            presets: vec![NodePreset {
                id: "pattern".to_string(),
                label: "Pattern".to_string(),
                kind: NodeKind::Pattern {
                    pattern_type: "default".to_string(),
                },
            }],
        }
    }
}

impl NodePresetCatalog {
    /// Builds a catalog from explicit preset entries.
    pub fn with_presets(presets: Vec<NodePreset>) -> Self {
        Self { presets }
    }

    /// Returns presets in display order for the Add Node dropdown.
    pub fn presets(&self) -> &[NodePreset] {
        &self.presets
    }

    fn get(&self, index: usize) -> Option<&NodePreset> {
        self.presets.get(index)
    }
}

#[derive(Resource, Default)]
struct NodeTypeDropdownState {
    open: bool,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<NodeTypeDropdownState>();
    app.init_resource::<NodePresetCatalog>();
    app.add_observer(close_dropdown_on_global_click);
    app.add_observer(on_breadcrumb_segment_click);
    app.add_systems(OnEnter(EditorReady), spawn_editor_menubar);
    app.add_systems(
        Update,
        (
            populate_node_type_dropdown,
            update_menubar_content,
            open_dropdown_from_context_click,
            sync_dropdown_visibility,
            update_menubar_visibility,
        )
            .chain()
            .run_if(in_state(EditorReady)),
    );
}

fn open_dropdown_from_context_click(
    mut context_menu_intents: MessageReader<ContextMenuIntent>,
    mut dropdown_state: ResMut<NodeTypeDropdownState>,
) {
    for intent in context_menu_intents.read() {
        if matches!(
            intent.target,
            ContextMenuTarget::Canvas | ContextMenuTarget::Node(_) | ContextMenuTarget::Widget(_)
        ) {
            dropdown_state.open = true;
        }
    }
}

fn spawn_editor_menubar(
    mut commands: Commands,
    styles: Res<UiStyles>,
    fonts: Res<FontAssets>,
    panel_assets: Res<UiPanelAssets>,
    button_assets: Res<UiButtonLongAssets>,
) {
    let mut add_style = styles
        .buttons
        .long
        .clone()
        .with_size(Vec2::new(152.0, 34.0));
    add_style.text.font_size = 20.0;
    let add_images = ButtonImages::from(button_assets.as_ref());
    let terminal_text_style = crate::editor::ui::styles::TextStyle {
        font: fonts.default_font_mono.clone(),
        font_size: STATUS_TEXT_SIZE,
        color: styles.text.label.color,
    };

    commands.spawn((
        ui_root("Editor UI Root"),
        EditorUiRoot,
        DespawnOnExit(InEditor),
        GlobalZIndex(40),
        children![
            (
                Name::new("Editor Menubar"),
                EditorMenubar,
                Node {
                    width: percent(100.0),
                    height: px(54.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexStart,
                    padding: UiRect::axes(px(10.0), px(8.0)),
                    overflow: Overflow::visible(),
                    ..default()
                },
                nine_slice_background(panel_assets.panel_v2.clone(), BORDER_12),
                children![(
                    Name::new("Menubar Left"),
                    GlobalTransform::default(),
                    Node {
                        position_type: PositionType::Relative,
                        align_items: AlignItems::Center,
                        column_gap: px(8.0),
                        overflow: Overflow::visible(),
                        ..default()
                    },
                    children![
                        button_base(
                            "Add Node",
                            toggle_node_type_dropdown,
                            (add_style.node(), NodeTypeDropdownToggle),
                            Some(add_images),
                            add_style
                        ),
                        button_base(
                            "Text",
                            open_text_draft_menu,
                            (styles.buttons.small.node(), OpenTextDraftButton),
                            Some(ButtonImages::from(button_assets.as_ref())),
                            styles.buttons.small.clone()
                        ),
                        (
                            Name::new("Node Type Dropdown"),
                            NodeTypeDropdown,
                            Node {
                                position_type: PositionType::Absolute,
                                top: px(38.0),
                                left: px(0.0),
                                width: px(180.0),
                                display: Display::None,
                                flex_direction: FlexDirection::Column,
                                row_gap: px(4.0),
                                padding: UiRect::all(px(6.0)),
                                ..default()
                            },
                            nine_slice_background(panel_assets.panel_v2.clone(), BORDER_4),
                        ),
                        (
                            Name::new("Runtime Controls"),
                            GlobalTransform::default(),
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: px(16.0),
                                margin: UiRect::left(px(8.0)),
                                ..default()
                            },
                            children![
                                runtime_control_button(
                                    "Play",
                                    RuntimeControlAction::Play,
                                    styles.as_ref(),
                                    button_assets.as_ref(),
                                    Vec2::new(32.0, 16.0)
                                ),
                                runtime_control_button(
                                    "Stop",
                                    RuntimeControlAction::Stop,
                                    styles.as_ref(),
                                    button_assets.as_ref(),
                                    Vec2::new(32.0, 16.0)
                                ),
                                runtime_control_button(
                                    "-",
                                    RuntimeControlAction::TempoDown,
                                    styles.as_ref(),
                                    button_assets.as_ref(),
                                    Vec2::new(32.0, 16.0)
                                ),
                                (
                                    RuntimeTempoValueLabel,
                                    label_with_style("120", &terminal_text_style),
                                ),
                                runtime_control_button(
                                    "+",
                                    RuntimeControlAction::TempoUp,
                                    styles.as_ref(),
                                    button_assets.as_ref(),
                                    Vec2::new(16.0, 16.0)
                                ),
                            ],
                        ),
                    ],
                )],
            ),
            (
                Name::new("Editor Status Bar"),
                EditorStatusBar,
                Node {
                    width: percent(100.0),
                    height: px(STATUS_BAR_HEIGHT),
                    min_height: px(STATUS_BAR_HEIGHT),
                    max_height: px(STATUS_BAR_HEIGHT),
                    margin: UiRect::top(Val::Auto),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    flex_wrap: FlexWrap::NoWrap,
                    column_gap: px(8.0),
                    padding: UiRect::axes(px(8.0), px(2.0)),
                    ..default()
                },
                nine_slice_background(panel_assets.panel_v2.clone(), BORDER_12),
                children![
                    (
                        Name::new("Status Scope"),
                        Node {
                            align_items: AlignItems::Center,
                            min_width: px(280.0),
                            flex_grow: 1.2,
                            ..default()
                        },
                        children![(
                            Name::new("Breadcrumbs"),
                            BreadcrumbsLabel,
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: px(0.0),
                                ..default()
                            },
                            children![
                                status_breadcrumb_segment(0, &terminal_text_style),
                                status_breadcrumb_separator(0, &terminal_text_style),
                                status_breadcrumb_segment(1, &terminal_text_style),
                                status_breadcrumb_separator(1, &terminal_text_style),
                                status_breadcrumb_segment(2, &terminal_text_style),
                                status_breadcrumb_separator(2, &terminal_text_style),
                                status_breadcrumb_segment(3, &terminal_text_style),
                                status_breadcrumb_separator(3, &terminal_text_style),
                                status_breadcrumb_segment(4, &terminal_text_style),
                                status_breadcrumb_separator(4, &terminal_text_style),
                                status_breadcrumb_segment(5, &terminal_text_style),
                            ],
                        )],
                    ),
                    (
                        Name::new("Status Node"),
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::FlexStart,
                            flex_grow: 0.7,
                            min_width: px(200.0),
                            ..default()
                        },
                        children![(StatusLabel, label_with_style("", &terminal_text_style),)],
                    ),
                    (
                        Name::new("Status Playback"),
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::FlexStart,
                            flex_grow: 1.1,
                            min_width: px(260.0),
                            ..default()
                        },
                        children![(
                            RuntimeStatusLabel,
                            label_with_style("", &terminal_text_style),
                        )],
                    ),
                    (
                        Name::new("Status CursorZoom"),
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::FlexEnd,
                            flex_grow: 0.6,
                            min_width: px(180.0),
                            ..default()
                        },
                        children![(CursorZoomLabel, label_with_style("", &terminal_text_style),)],
                    ),
                ],
            ),
        ],
    ));
}

fn populate_node_type_dropdown(
    mut commands: Commands,
    dropdowns: Query<Entity, Added<NodeTypeDropdown>>,
    styles: Res<UiStyles>,
    button_assets: Res<UiButtonLongAssets>,
    presets: Res<NodePresetCatalog>,
) {
    for dropdown_entity in &dropdowns {
        commands.entity(dropdown_entity).with_children(|parent| {
            for (index, preset) in presets.presets().iter().enumerate() {
                parent.spawn(node_type_menu_button(
                    index,
                    preset.label.as_str(),
                    styles.as_ref(),
                    button_assets.as_ref(),
                ));
            }
        });
    }
}

fn open_text_draft_menu(
    click: On<Pointer<Click>>,
    buttons: Query<(), With<OpenTextDraftButton>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    if buttons.get(click.entity).is_ok()
        || buttons.get(click.original_event_target()).is_ok()
        || buttons.get(click.event_target()).is_ok()
    {
        next_menu.set(Menu::TextDraft);
    }
}

fn node_type_menu_button(
    preset_index: usize,
    label: &str,
    styles: &UiStyles,
    button_assets: &UiButtonLongAssets,
) -> impl Bundle {
    let mut style = styles
        .buttons
        .long
        .clone()
        .with_size(Vec2::new(168.0, 28.0));
    style.text.font_size = 18.0;
    let images = ButtonImages::from(button_assets);
    button_base(
        label.to_string(),
        add_node_from_type_menu,
        (style.node(), NodeTypeMenuItem { preset_index }),
        Some(images),
        style,
    )
}

fn runtime_control_button(
    label: &str,
    action: RuntimeControlAction,
    styles: &UiStyles,
    button_assets: &UiButtonLongAssets,
    size: Vec2,
) -> impl Bundle {
    let mut style = styles.buttons.long.clone().with_size(size);
    style.text.font_size = 16.0;
    let images = ButtonImages::from(button_assets);
    button_base(
        label.to_string(),
        on_runtime_control_button_click,
        (style.node(), RuntimeControlButton { action }),
        Some(images),
        style,
    )
}

fn status_breadcrumb_segment(
    index: usize,
    style: &crate::editor::ui::styles::TextStyle,
) -> impl Bundle {
    (
        BreadcrumbSegment {
            index,
            action: None,
        },
        Node {
            align_items: AlignItems::Center,
            padding: UiRect::axes(px(1.0), px(0.0)),
            ..default()
        },
        Pickable::default(),
        label_with_style("", style),
    )
}

fn status_breadcrumb_separator(
    index: usize,
    style: &crate::editor::ui::styles::TextStyle,
) -> impl Bundle {
    (
        BreadcrumbSeparator { index },
        Pickable::IGNORE,
        label_with_style("", style),
    )
}

fn toggle_node_type_dropdown(
    click: On<Pointer<Click>>,
    toggle_entity: Option<Single<Entity, With<NodeTypeDropdownToggle>>>,
    hierarchy: HierarchyAccess,
    mut dropdown_state: ResMut<NodeTypeDropdownState>,
) {
    let Some(toggle_entity) = toggle_entity else {
        return;
    };
    let toggle_entity = *toggle_entity;
    if !pointer_click_touches_root(&click, toggle_entity, &hierarchy) {
        return;
    }
    dropdown_state.open = !dropdown_state.open;
}

fn on_runtime_control_button_click(
    click: On<Pointer<Click>>,
    buttons: Query<&RuntimeControlButton>,
    mut runtime_state: ResMut<RuntimeState>,
    mut intents: MessageWriter<RuntimeIntent>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };

    match button.action {
        RuntimeControlAction::Play => {
            intents.write(RuntimeIntent::play());
        }
        RuntimeControlAction::Stop => {
            intents.write(RuntimeIntent::stop());
        }
        RuntimeControlAction::TempoDown => {
            let next =
                (runtime_state.tempo_cpm - TEMPO_STEP_CPM).clamp(TEMPO_MIN_CPM, TEMPO_MAX_CPM);
            runtime_state.tempo_cpm = next;
            intents.write(RuntimeIntent::set_tempo(next));
        }
        RuntimeControlAction::TempoUp => {
            let next =
                (runtime_state.tempo_cpm + TEMPO_STEP_CPM).clamp(TEMPO_MIN_CPM, TEMPO_MAX_CPM);
            runtime_state.tempo_cpm = next;
            intents.write(RuntimeIntent::set_tempo(next));
        }
    }
}

fn add_node_from_type_menu(
    click: On<Pointer<Click>>,
    items: Query<&NodeTypeMenuItem>,
    presets: Res<NodePresetCatalog>,
    mut app_state: ResMut<AppState>,
    mut dropdown_state: ResMut<NodeTypeDropdownState>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&GlobalTransform, &Camera, &Transform), With<CanvasCamera>>,
    mut layout_changed: MessageWriter<LayoutChanged>,
) {
    let Ok(item) = items
        .get(click.entity)
        .or_else(|_| items.get(click.original_event_target()))
        .or_else(|_| items.get(click.event_target()))
    else {
        return;
    };

    let Some(preset) = presets.get(item.preset_index) else {
        return;
    };

    dropdown_state.open = false;

    let (camera_global, camera, camera_transform) = camera.into_inner();
    let spawn_world = window
        .cursor_position()
        .and_then(|cursor_viewport_pos| {
            camera
                .viewport_to_world_2d(camera_global, cursor_viewport_pos)
                .ok()
        })
        .unwrap_or(camera_transform.translation.truncate());

    let scope_id = app_state.current_scope;
    let node_id = NodeId::new();

    app_state.project.add_node(
        scope_id,
        ModelNode {
            id: node_id,
            kind: preset.kind.clone(),
            parent_scope: scope_id,
            params: HashMap::new(),
            input_ports: vec![],
            output_ports: vec![],
        },
    );

    let layout = app_state
        .project
        .layout
        .layouts
        .entry(scope_id)
        .or_insert_with(|| Layout {
            scope_id,
            node_positions: HashMap::new(),
            camera_pos: (0.0, 0.0),
            zoom: 1.0,
        });
    layout
        .node_positions
        .insert(node_id, (spawn_world.x, spawn_world.y));

    layout_changed.write(LayoutChanged {
        scope: Some(scope_id),
    });
}

fn update_menubar_content(
    app_state: Res<AppState>,
    selection: Res<SelectionState>,
    runtime_state: Res<RuntimeState>,
    text_draft_state: Res<TextDraftState>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform, &Transform, Option<&PanCamera>), With<CanvasCamera>>,
    mut text_queries: ParamSet<(
        Query<
            &mut Text,
            (
                With<StatusLabel>,
                Without<BreadcrumbSegment>,
                Without<BreadcrumbSeparator>,
            ),
        >,
        Query<
            (&mut Text, &mut TextColor),
            (
                With<RuntimeStatusLabel>,
                Without<BreadcrumbSegment>,
                Without<BreadcrumbSeparator>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<RuntimeTempoValueLabel>,
                Without<BreadcrumbSegment>,
                Without<BreadcrumbSeparator>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<CursorZoomLabel>,
                Without<BreadcrumbSegment>,
                Without<BreadcrumbSeparator>,
            ),
        >,
        Query<
            (
                &mut BreadcrumbSegment,
                &mut Text,
                &mut TextColor,
                &mut Pickable,
            ),
            (Without<RuntimeStatusLabel>, Without<StatusLabel>),
        >,
        Query<
            (&BreadcrumbSeparator, &mut Text, &mut TextColor),
            (Without<RuntimeStatusLabel>, Without<StatusLabel>),
        >,
    )>,
) {
    let breadcrumb_sections = build_terminal_breadcrumbs(&app_state, &selection);

    let window = window.into_inner();
    let (camera, camera_global, transform, controller) = camera.into_inner();
    let zoom = controller
        .map(|controller| controller.zoom_factor)
        .unwrap_or(transform.scale.x.max(0.0001));
    let cursor_pos = window
        .cursor_position()
        .and_then(|cursor_viewport_pos| {
            camera
                .viewport_to_world_2d(camera_global, cursor_viewport_pos)
                .ok()
        })
        .map(|world_pos| format!("Cursor: {:.0},{:.0}", world_pos.x, world_pos.y))
        .unwrap_or_else(|| "Cursor: --,--".to_string());
    let primary_node_pos = format_primary_node_position(&app_state, &selection);
    let node_status = format!(
        "{primary_node_pos} | Selected: {}",
        selection.selected.len()
    );
    let cursor_zoom_status = format!("{cursor_pos} | Zoom: {:.1}x", zoom);

    let tempo = format!("{:.0}", runtime_state.tempo_cpm);
    let draft_status = format_text_draft_status(&text_draft_state);

    let playback_status = if let Some(error) = runtime_state.last_error.as_ref() {
        let trimmed = truncate_runtime_error(error);
        let rev = runtime_state.last_error_rev.unwrap_or(0);
        format!("Playback: Error (rev {rev}) | {trimmed} | {draft_status}")
    } else {
        let playback = if runtime_state.playing {
            "Playing"
        } else {
            "Stopped"
        };
        let runtime_rev = runtime_state.last_ok_rev.unwrap_or(0);
        format!(
            "Playback: {playback} | Tempo {:.0} cpm | Rev {runtime_rev}/{} | {draft_status}",
            runtime_state.last_eval_rev, runtime_state.tempo_cpm,
        )
    };

    {
        let mut breadcrumb_segments = text_queries.p4();
        for (mut segment, mut span, mut color, mut pickable) in &mut breadcrumb_segments {
            if let Some(section) = breadcrumb_sections.get(segment.index) {
                span.0 = section.text.clone();
                color.0 = section.color;
                segment.action = Some(section.action);
                *pickable = Pickable::default();
            } else {
                span.0.clear();
                color.0 = Color::WHITE;
                segment.action = None;
                *pickable = Pickable::IGNORE;
            }
        }
    }
    {
        let mut breadcrumb_separators = text_queries.p5();
        for (separator, mut span, mut color) in &mut breadcrumb_separators {
            if separator.index + 1 < breadcrumb_sections.len() {
                span.0 = " > ".to_string();
                color.0 = separator_color();
            } else {
                span.0.clear();
            }
        }
    }
    {
        let mut status_query = text_queries.p0();
        let Ok(mut status_label) = status_query.single_mut() else {
            return;
        };
        status_label.0 = node_status;
    }
    {
        let mut runtime_query = text_queries.p1();
        let Ok((mut runtime_label, mut runtime_color)) = runtime_query.single_mut() else {
            return;
        };
        runtime_label.0 = playback_status;
        runtime_color.0 = runtime_status_color(&runtime_state, &text_draft_state);
    }
    {
        let mut tempo_query = text_queries.p2();
        let Ok(mut tempo_label) = tempo_query.single_mut() else {
            return;
        };
        tempo_label.0 = tempo;
    }
    {
        let mut cursor_zoom_query = text_queries.p3();
        let Ok(mut cursor_zoom_label) = cursor_zoom_query.single_mut() else {
            return;
        };
        cursor_zoom_label.0 = cursor_zoom_status;
    }
}

fn truncate_runtime_error(message: &str) -> String {
    const MAX_LEN: usize = 64;
    if message.chars().count() <= MAX_LEN {
        return message.to_string();
    }
    let mut result: String = message.chars().take(MAX_LEN).collect();
    result.push_str("...");
    result
}

fn format_text_draft_status(state: &TextDraftState) -> String {
    let dirty = if state.dirty { "dirty" } else { "clean" };
    if state.pending_validation_request.is_some() {
        return format!("Draft: {dirty}/validating");
    }
    if state.validation_debounce_armed {
        return format!("Draft: {dirty}/typing");
    }
    if state.last_validation_ok {
        return format!("Draft: {dirty}/ok");
    }
    if let Some(error) = state.last_validation_error.as_deref() {
        let truncated = truncate_runtime_error(error);
        return format!("Draft: {dirty}/err {truncated}");
    }
    format!("Draft: {dirty}/pending")
}

fn runtime_status_color(runtime_state: &RuntimeState, text_draft_state: &TextDraftState) -> Color {
    if runtime_state.last_error.is_some() || text_draft_state.last_validation_error.is_some() {
        return Color::srgb(0.98, 0.42, 0.42);
    }
    if text_draft_state.pending_validation_request.is_some() {
        return Color::srgb(0.95, 0.84, 0.36);
    }
    if text_draft_state.validation_debounce_armed {
        return Color::srgb(0.45, 0.82, 0.95);
    }
    if text_draft_state.last_validation_ok {
        return Color::srgb(0.58, 0.95, 0.66);
    }
    Color::srgb(0.72, 0.76, 0.8)
}

fn sync_dropdown_visibility(
    dropdown_state: Res<NodeTypeDropdownState>,
    dropdown: Option<Single<&mut Node, With<NodeTypeDropdown>>>,
) {
    let Some(mut dropdown) = dropdown else {
        return;
    };
    dropdown.display = if dropdown_state.open {
        Display::Flex
    } else {
        Display::None
    };
}

fn update_menubar_visibility(
    menu: Res<State<Menu>>,
    mut dropdown_state: ResMut<NodeTypeDropdownState>,
    root: Option<Single<&mut Node, With<EditorUiRoot>>>,
) {
    let Some(mut root) = root else {
        return;
    };
    let visible = *menu.get() == Menu::None;
    if !visible {
        dropdown_state.open = false;
    }
    root.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
}

fn close_dropdown_on_global_click(
    click: On<Pointer<Click>>,
    mut dropdown_state: ResMut<NodeTypeDropdownState>,
    dropdown_entity: Option<Single<Entity, With<NodeTypeDropdown>>>,
    toggle_entity: Option<Single<Entity, With<NodeTypeDropdownToggle>>>,
    hierarchy: HierarchyAccess,
) {
    if !dropdown_state.open {
        return;
    }
    let Some(dropdown_entity) = dropdown_entity else {
        return;
    };
    let Some(toggle_entity) = toggle_entity else {
        return;
    };
    let dropdown_entity = *dropdown_entity;
    let toggle_entity = *toggle_entity;

    if pointer_click_touches_root(&click, dropdown_entity, &hierarchy)
        || pointer_click_touches_root(&click, toggle_entity, &hierarchy)
    {
        return;
    }

    dropdown_state.open = false;
}

fn pointer_click_touches_root(
    click: &On<Pointer<Click>>,
    root: Entity,
    hierarchy: &HierarchyAccess,
) -> bool {
    hierarchy.is_descendant_or_self(click.event_target(), root)
        || hierarchy.is_descendant_or_self(click.original_event_target(), root)
        || hierarchy.is_descendant_or_self(click.entity, root)
}

fn on_breadcrumb_segment_click(
    click: On<Pointer<Click>>,
    segments: Query<&BreadcrumbSegment>,
    mut app_state: ResMut<AppState>,
    mut selection: ResMut<SelectionState>,
    mut selection_intents: MessageWriter<SelectionIntent>,
) {
    let Ok(segment) = segments
        .get(click.entity)
        .or_else(|_| segments.get(click.original_event_target()))
        .or_else(|_| segments.get(click.event_target()))
    else {
        return;
    };

    let Some(action) = segment.action else {
        return;
    };

    match action {
        BreadcrumbAction::Scope(scope_id) => {
            if !app_state.project.model.scopes.contains_key(&scope_id) {
                return;
            }
            if app_state.current_scope != scope_id {
                app_state.current_scope = scope_id;
                selection.clear();
            }
        }
        BreadcrumbAction::Node(node_id) => {
            let Some(parent_scope) = app_state
                .project
                .model
                .nodes
                .get(&node_id)
                .map(|node| node.parent_scope)
            else {
                return;
            };

            if app_state.current_scope != parent_scope {
                app_state.current_scope = parent_scope;
                selection.clear();
            }

            selection_intents.write(SelectionIntent::ClickNode {
                node_id,
                modifiers: Modifiers::default(),
            });
        }
    }
}

#[derive(Clone)]
struct BreadcrumbSection {
    text: String,
    color: Color,
    action: BreadcrumbAction,
}

fn build_terminal_breadcrumbs(
    app_state: &AppState,
    selection: &SelectionState,
) -> Vec<BreadcrumbSection> {
    let selected = build_selected_node_breadcrumbs(app_state, selection);
    if !selected.is_empty() {
        return selected;
    }
    build_scope_breadcrumbs(app_state)
}

fn build_scope_breadcrumbs(app_state: &AppState) -> Vec<BreadcrumbSection> {
    let mut lineage = vec![];
    let mut current = Some(app_state.current_scope);
    let max_depth = app_state.project.model.scopes.len().saturating_add(1);

    for _ in 0..max_depth {
        let Some(scope_id) = current else {
            break;
        };
        lineage.push(scope_id);
        current = app_state
            .project
            .model
            .scopes
            .get(&scope_id)
            .and_then(|scope| scope.parent_scope);
    }

    lineage.reverse();

    let mut parts = vec![BreadcrumbSection {
        text: app_state.project.name.clone(),
        color: Color::srgb(0.85, 0.9, 0.95),
        action: BreadcrumbAction::Scope(app_state.project.model.root_scope),
    }];
    parts.push(BreadcrumbSection {
        text: "ROOT".to_string(),
        color: Color::srgb(0.65, 0.72, 0.78),
        action: BreadcrumbAction::Scope(app_state.project.model.root_scope),
    });
    for scope_id in lineage.into_iter().skip(1) {
        parts.push(BreadcrumbSection {
            text: format!("SCOPE {}", short_scope_id(scope_id)),
            color: Color::srgb(0.65, 0.72, 0.78),
            action: BreadcrumbAction::Scope(scope_id),
        });
    }
    parts.truncate(MAX_BREADCRUMB_SEGMENTS);
    parts
}

fn build_selected_node_breadcrumbs(
    app_state: &AppState,
    selection: &SelectionState,
) -> Vec<BreadcrumbSection> {
    let mut sections = Vec::new();
    for node_id in selection.selected.iter().take(MAX_BREADCRUMB_SEGMENTS) {
        let Some(node) = app_state.project.model.nodes.get(node_id) else {
            continue;
        };
        sections.push(BreadcrumbSection {
            text: format!(
                "{}:{}",
                node_kind_short_label(&node.kind),
                short_node_id(*node_id)
            ),
            color: node_kind_color(&node.kind),
            action: BreadcrumbAction::Node(*node_id),
        });
    }
    sections
}

fn node_kind_short_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Pattern { .. } => "PAT",
        NodeKind::Transform { .. } => "FX",
        NodeKind::Output => "OUT",
        NodeKind::Binding { .. } => "BIND",
        NodeKind::Custom { .. } => "NODE",
    }
}

fn node_kind_color(kind: &NodeKind) -> Color {
    match kind {
        NodeKind::Pattern { .. } => Color::srgb(0.31, 0.82, 0.95),
        NodeKind::Transform { .. } => Color::srgb(0.98, 0.73, 0.35),
        NodeKind::Output => Color::srgb(0.58, 0.95, 0.66),
        NodeKind::Binding { .. } => Color::srgb(0.86, 0.70, 0.98),
        NodeKind::Custom { .. } => Color::srgb(0.86, 0.86, 0.86),
    }
}

fn separator_color() -> Color {
    Color::srgb(0.58, 0.65, 0.72)
}

fn format_primary_node_position(app_state: &AppState, selection: &SelectionState) -> String {
    let Some(primary_id) = selection.primary else {
        return "Node: --,--".to_string();
    };
    let Some(layout) = app_state
        .project
        .layout
        .layouts
        .get(&app_state.current_scope)
    else {
        return "Node: --,--".to_string();
    };
    let Some((x, y)) = layout.node_positions.get(&primary_id).copied() else {
        return "Node: --,--".to_string();
    };

    format!("Node {}: {:.0},{:.0}", short_node_id(primary_id), x, y)
}

fn short_scope_id(scope_id: ScopeId) -> String {
    scope_id.to_string().chars().take(8).collect()
}

fn short_node_id(node_id: NodeId) -> String {
    node_id.to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_button_assets() -> UiButtonLongAssets {
        UiButtonLongAssets {
            off: Handle::default(),
            on: Handle::default(),
            hover: Handle::default(),
            focus: Handle::default(),
            disabled: Handle::default(),
        }
    }

    fn dummy_panel_assets() -> UiPanelAssets {
        UiPanelAssets {
            panel: Handle::default(),
            panel_off: Handle::default(),
            panel_on: Handle::default(),
            panel_v2: Handle::default(),
            panel_v2_off: Handle::default(),
            panel_v2_on: Handle::default(),
            panel_menu: Handle::default(),
            options: Handle::default(),
        }
    }

    fn dummy_fonts() -> FontAssets {
        FontAssets {
            default_font: Handle::default(),
            default_font_bold: Handle::default(),
            default_font_mono: Handle::default(),
            pixel_font: Handle::default(),
            pixel_font_bold: Handle::default(),
        }
    }

    #[test]
    fn spawn_editor_menubar_spawns_expected_entities_without_duplicate_node() {
        let mut app = App::new();
        app.insert_resource(UiStyles::default());
        app.insert_resource(dummy_fonts());
        app.insert_resource(dummy_panel_assets());
        app.insert_resource(dummy_button_assets());
        app.add_systems(Update, spawn_editor_menubar);

        app.update();

        let world = app.world_mut();

        let mut root_query = world.query::<&EditorUiRoot>();
        assert_eq!(root_query.iter(world).count(), 1);

        let mut menubar_query = world.query::<&EditorMenubar>();
        assert_eq!(menubar_query.iter(world).count(), 1);

        let mut statusbar_query = world.query::<&EditorStatusBar>();
        assert_eq!(statusbar_query.iter(world).count(), 1);

        let mut breadcrumbs_query = world.query::<&BreadcrumbsLabel>();
        assert_eq!(breadcrumbs_query.iter(world).count(), 1);

        let mut status_query = world.query::<&StatusLabel>();
        assert_eq!(status_query.iter(world).count(), 1);

        let mut runtime_query = world.query::<&RuntimeStatusLabel>();
        assert_eq!(runtime_query.iter(world).count(), 1);

        let mut cursor_zoom_query = world.query::<&CursorZoomLabel>();
        assert_eq!(cursor_zoom_query.iter(world).count(), 1);
    }
}
