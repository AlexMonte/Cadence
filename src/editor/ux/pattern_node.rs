//! Pattern node UI overlay for grid editing, sample lanes, and inline controls.

use bevy::{camera::visibility::RenderLayers, picking::hover::Hovered, prelude::*};

use crate::EditorSystemSet;
use crate::core::sequencer::{DEFAULT_SAMPLE, GRID_DIM_MAX, GRID_DIM_MIN};
use crate::core::{ModelNode, NodeId, NodeKind, SequencerPatternData};
use crate::editor::AppState;
use crate::editor::camera::CANVAS_RENDER_LAYER;
use crate::editor::canvas::CanvasNode;
use crate::editor::ecs::HierarchyAccess;
use crate::editor::gestures::pointer::LayoutChanged;
use crate::editor::state::EditorReady;
use crate::editor::ui::interactions::InteractiveChild;
use crate::editor::ui::{
    FontAssets, UiButtonCheckboxAssets, UiButtonLongAssets, UiButtonTabAssets, UiComponentAssets,
    UiIconAssets, UiIconId, UiPanelAssets,
};

const GRID_CELL_SIZE: f32 = 16.0;
const GRID_CELL_SPACING: f32 = 18.0;
const GRID_Y_OFFSET: f32 = -28.0;
const ROW_SAMPLE_X_OFFSET: f32 = 56.0;
const PANEL_Z: f32 = 0.02;
const CONTROL_Z: f32 = 0.08;
const LABEL_Z: f32 = 0.09;
const INTERFACE_ROOT_Z: f32 = 0.2;
const OPTION_ROW_HEIGHT: f32 = 20.0;
const OPTION_PANEL_WIDTH: f32 = 144.0;
const ROW_SAMPLE_BUTTON_SIZE: Vec2 = Vec2::new(56.0, 16.0);
const STEP_LEVELS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const HELP_BUTTON_SIZE: f32 = 16.0;
const HELP_BUTTON_PADDING: f32 = 10.0;
const HELP_PANEL_Z: f32 = 0.18;
const HELP_PANEL_WIDTH: f32 = 248.0;
const HELP_PANEL_HEIGHT: f32 = 136.0;
const SAMPLE_OPTIONS: [&str; 8] = ["bd", "sn", "hh", "cp", "perc", "kick", "tom", "ride"];

pub(super) fn plugin(app: &mut App) {
    app.add_observer(close_pattern_dropdowns_on_global_click);
    app.add_systems(
        Update,
        (
            spawn_pattern_node_interface,
            rebuild_pattern_grid_when_dirty,
            sync_pattern_node_interface,
        )
            .chain()
            .run_if(in_state(EditorReady))
            .in_set(EditorSystemSet::RenderSelection),
    );
}

#[derive(Component, Clone, Copy)]
struct PatternNodeUiRoot;

#[derive(Component, Clone, Copy)]
struct PatternNodeUi {
    rows_label: Entity,
    cols_label: Entity,
    lane_root: Entity,
    dropdown_root: Entity,
    grid_root: Entity,
    help_root: Entity,
}

#[derive(Component, Clone)]
struct PatternNodeUiState {
    row_samples: Vec<String>,
    rows: u8,
    cols: u8,
    beats: Vec<bool>,
    velocities: Vec<f32>,
    accents: Vec<bool>,
    probabilities: Vec<f32>,
    dropdown_open: bool,
    dropdown_row: Option<u8>,
    help_open: bool,
    grid_dirty: bool,
}

#[derive(Component, Clone, Copy)]
struct PatternSampleToggle {
    owner: Entity,
    row: u8,
}

#[derive(Component, Clone, Copy)]
struct PatternSampleOption {
    owner: Entity,
    sample: &'static str,
}

#[derive(Component, Clone, Copy)]
struct PatternGridResizeButton {
    owner: Entity,
    axis: GridAxis,
    delta: i8,
}

#[derive(Component, Clone, Copy)]
struct PatternRowSampleLabel {
    owner: Entity,
    row: u8,
}

#[derive(Component, Clone, Copy)]
struct PatternBeatCell {
    owner: Entity,
    row: u8,
    col: u8,
}

#[derive(Component, Clone, Copy)]
struct PatternHelpToggle {
    owner: Entity,
}

#[derive(Component, Clone, Copy)]
struct PatternHelpClose {
    owner: Entity,
}

#[derive(Clone, Copy)]
enum GridAxis {
    Rows,
    Cols,
}

#[derive(Clone, Copy)]
enum CellEditAction {
    ToggleBeat,
    ToggleAccent,
    CycleVelocity,
    CycleProbability,
}

fn spawn_pattern_node_interface(
    mut commands: Commands,
    nodes: Query<(Entity, &CanvasNode), Added<CanvasNode>>,
    app_state: Res<AppState>,
    fonts: Res<FontAssets>,
    panel_assets: Res<UiPanelAssets>,
    long_buttons: Res<UiButtonLongAssets>,
    tab_buttons: Res<UiButtonTabAssets>,
    checkbox_assets: Res<UiButtonCheckboxAssets>,
    component_assets: Res<UiComponentAssets>,
    icon_assets: Res<UiIconAssets>,
) {
    for (node_entity, canvas_node) in &nodes {
        let Some(model_node) = app_state.project.model.nodes.get(&canvas_node.node_id) else {
            continue;
        };
        if !matches!(model_node.kind, NodeKind::Pattern { .. }) {
            continue;
        }

        let state = pattern_state_from_model(model_node);
        let half_w = canvas_node.width * 0.5;
        let half_h = canvas_node.height * 0.5;
        let panel_size = Vec2::new(
            (canvas_node.width - 14.0).max(220.0),
            (canvas_node.height - 14.0).max(140.0),
        );
        let rows_y = half_h - 40.0;
        let cols_y = half_h - 68.0;

        let ui_root = commands
            .spawn((
                Name::new("Pattern Node UI"),
                PatternNodeUiRoot,
                Transform::from_xyz(0.0, 0.0, INTERFACE_ROOT_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(node_entity).add_child(ui_root);

        let panel = {
            let mut sprite = Sprite::from_image(panel_assets.panel_v2.clone());
            sprite.custom_size = Some(panel_size);
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.92);
            commands
                .spawn((
                    Name::new("Pattern Panel"),
                    sprite,
                    Transform::from_xyz(0.0, 0.0, PANEL_Z),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::IGNORE,
                ))
                .id()
        };

        let title = commands
            .spawn((
                Name::new("Pattern Title"),
                Text2d::new("Pattern"),
                TextFont {
                    font: fonts.default_font_bold.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                Transform::from_xyz(0.0, half_h - 18.0, LABEL_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();

        let dropdown_root = commands
            .spawn((
                Name::new("Pattern Sample Dropdown"),
                Transform::from_xyz(0.0, 0.0, CONTROL_Z),
                Visibility::Hidden,
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();

        let dropdown_panel_height = SAMPLE_OPTIONS.len() as f32 * OPTION_ROW_HEIGHT + 10.0;
        let dropdown_panel = {
            let mut sprite = Sprite::from_image(panel_assets.panel.clone());
            sprite.custom_size = Some(Vec2::new(OPTION_PANEL_WIDTH, dropdown_panel_height));
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.98);
            commands
                .spawn((
                    Name::new("Pattern Sample Dropdown Panel"),
                    sprite,
                    Transform::from_xyz(0.0, -dropdown_panel_height * 0.5 + 12.0, PANEL_Z),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::IGNORE,
                ))
                .id()
        };

        let rows_label = commands
            .spawn((
                Name::new("Pattern Rows Label"),
                Text2d::new(format!("R:{}", state.rows)),
                TextFont {
                    font: fonts.default_font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Transform::from_xyz(half_w - 84.0, rows_y, LABEL_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();

        let cols_label = commands
            .spawn((
                Name::new("Pattern Cols Label"),
                Text2d::new(format!("C:{}", state.cols)),
                TextFont {
                    font: fonts.default_font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Transform::from_xyz(half_w - 84.0, cols_y, LABEL_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();

        let rows_dec = resize_button(
            &mut commands,
            node_entity,
            GridAxis::Rows,
            -1,
            component_assets.chevron_left.clone(),
            Vec3::new(half_w - 116.0, rows_y, CONTROL_Z),
        );
        let rows_inc = resize_button(
            &mut commands,
            node_entity,
            GridAxis::Rows,
            1,
            component_assets.chevron_right.clone(),
            Vec3::new(half_w - 52.0, rows_y, CONTROL_Z),
        );
        let cols_dec = resize_button(
            &mut commands,
            node_entity,
            GridAxis::Cols,
            -1,
            component_assets.chevron_left.clone(),
            Vec3::new(half_w - 116.0, cols_y, CONTROL_Z),
        );
        let cols_inc = resize_button(
            &mut commands,
            node_entity,
            GridAxis::Cols,
            1,
            component_assets.chevron_right.clone(),
            Vec3::new(half_w - 52.0, cols_y, CONTROL_Z),
        );

        let grid_root = commands
            .spawn((
                Name::new("Pattern Grid"),
                Transform::from_xyz(0.0, GRID_Y_OFFSET, CONTROL_Z),
                Visibility::Visible,
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();
        let lane_root = commands
            .spawn((
                Name::new("Pattern Sample Lanes"),
                Transform::from_xyz(0.0, GRID_Y_OFFSET, CONTROL_Z),
                Visibility::Visible,
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();
        let help_toggle = {
            let mut sprite = Sprite::from_color(
                Color::srgba(0.12, 0.18, 0.24, 0.95),
                Vec2::splat(HELP_BUTTON_SIZE),
            );
            sprite.custom_size = Some(Vec2::splat(HELP_BUTTON_SIZE));
            let help_toggle = commands
                .spawn((
                    Name::new("Pattern Help Toggle"),
                    PatternHelpToggle { owner: node_entity },
                    sprite,
                    Transform::from_xyz(
                        -half_w + HELP_BUTTON_PADDING,
                        half_h - HELP_BUTTON_PADDING,
                        CONTROL_Z,
                    ),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::default(),
                    Hovered(false),
                    InteractiveChild,
                ))
                .observe(toggle_pattern_help_popup)
                .id();

            let help_glyph = commands
                .spawn((
                    Name::new("Pattern Help Glyph"),
                    Text2d::new("?"),
                    TextFont {
                        font: fonts.default_font_bold.clone(),
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Transform::from_xyz(0.0, -1.0, 0.01),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::IGNORE,
                ))
                .id();

            commands.entity(help_toggle).add_child(help_glyph);
            help_toggle
        };
        let help_root = commands
            .spawn((
                Name::new("Pattern Help Popup"),
                Transform::from_xyz(0.0, -2.0, HELP_PANEL_Z),
                Visibility::Hidden,
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();
        let help_panel = {
            let mut sprite = Sprite::from_image(panel_assets.panel.clone());
            sprite.custom_size = Some(Vec2::new(HELP_PANEL_WIDTH, HELP_PANEL_HEIGHT));
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.98);
            commands
                .spawn((
                    Name::new("Pattern Help Panel"),
                    sprite,
                    Transform::from_xyz(0.0, 0.0, PANEL_Z),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::IGNORE,
                ))
                .id()
        };
        let help_title = commands
            .spawn((
                Name::new("Pattern Help Title"),
                Text2d::new("Pattern Controls"),
                TextFont {
                    font: fonts.default_font_bold.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                Transform::from_xyz(0.0, HELP_PANEL_HEIGHT * 0.5 - 16.0, LABEL_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();
        let help_text = commands
            .spawn((
                Name::new("Pattern Help Text"),
                Text2d::new(
                    "Left click: toggle step\nShift or right click: accent\nAlt + click: velocity\nCmd/Ctrl or middle click: probability\nSample lane button: choose row sample\nRows/Cols arrows: resize grid",
                ),
                TextFont {
                    font: fonts.default_font.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                Transform::from_xyz(0.0, -8.0, LABEL_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ))
            .id();
        let help_close = {
            let mut sprite = Sprite::from_image(icon_assets.get(UiIconId::Close));
            sprite.custom_size = Some(Vec2::splat(14.0));
            commands
                .spawn((
                    Name::new("Pattern Help Close"),
                    PatternHelpClose { owner: node_entity },
                    sprite,
                    Transform::from_xyz(
                        HELP_PANEL_WIDTH * 0.5 - 14.0,
                        HELP_PANEL_HEIGHT * 0.5 - 14.0,
                        CONTROL_Z,
                    ),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::default(),
                    Hovered(false),
                    InteractiveChild,
                ))
                .observe(close_pattern_help_popup)
                .id()
        };
        commands
            .entity(help_root)
            .add_children(&[help_panel, help_title, help_text, help_close]);

        commands.entity(ui_root).add_children(&[
            panel,
            title,
            dropdown_root,
            help_root,
            help_toggle,
            rows_label,
            cols_label,
            rows_dec,
            rows_inc,
            cols_dec,
            cols_inc,
            lane_root,
            grid_root,
        ]);
        commands.entity(dropdown_root).add_child(dropdown_panel);
        spawn_pattern_row_sample_lanes(
            &mut commands,
            lane_root,
            node_entity,
            &state,
            long_buttons.as_ref(),
            fonts.as_ref(),
        );
        commands.entity(node_entity).insert(PatternNodeUi {
            rows_label,
            cols_label,
            lane_root,
            dropdown_root,
            grid_root,
            help_root,
        });
        commands.entity(node_entity).insert(state.clone());

        for (index, sample) in SAMPLE_OPTIONS.iter().enumerate() {
            let y = -8.0 - index as f32 * OPTION_ROW_HEIGHT;
            let option_entity = {
                let mut sprite = Sprite::from_image(tab_buttons.normal.clone());
                sprite.custom_size = Some(Vec2::new(OPTION_PANEL_WIDTH - 14.0, 18.0));
                commands
                    .spawn((
                        Name::new(format!("Pattern Sample Option::{sample}")),
                        PatternSampleOption {
                            owner: node_entity,
                            sample,
                        },
                        sprite,
                        Transform::from_xyz(0.0, y, CONTROL_Z),
                        RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                        Pickable::default(),
                        Hovered(false),
                        InteractiveChild,
                    ))
                    .observe(select_pattern_sample)
                    .id()
            };
            let option_label = commands
                .spawn((
                    Name::new(format!("Pattern Sample Label::{sample}")),
                    Text2d::new((*sample).to_string()),
                    TextFont {
                        font: fonts.default_font.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Transform::from_xyz(0.0, y, LABEL_Z),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::IGNORE,
                ))
                .id();
            commands
                .entity(dropdown_root)
                .add_children(&[option_entity, option_label]);
        }

        spawn_pattern_grid_cells(
            &mut commands,
            grid_root,
            node_entity,
            &state,
            checkbox_assets.as_ref(),
        );
    }
}

fn resize_button(
    commands: &mut Commands,
    owner: Entity,
    axis: GridAxis,
    delta: i8,
    image: Handle<Image>,
    position: Vec3,
) -> Entity {
    let mut sprite = Sprite::from_image(image);
    sprite.custom_size = Some(Vec2::splat(14.0));
    commands
        .spawn((
            Name::new("Pattern Grid Resize"),
            PatternGridResizeButton { owner, axis, delta },
            sprite,
            Transform::from_translation(position),
            RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
            Pickable::default(),
            Hovered(false),
            InteractiveChild,
        ))
        .observe(resize_pattern_grid)
        .id()
}

fn spawn_pattern_row_sample_lanes(
    commands: &mut Commands,
    lane_root: Entity,
    owner: Entity,
    state: &PatternNodeUiState,
    long_buttons: &UiButtonLongAssets,
    fonts: &FontAssets,
) {
    let sample_x = sample_button_x(state.cols);
    let start_y = grid_start_y(state.rows);
    commands.entity(lane_root).with_children(|parent| {
        for row in 0..state.rows as usize {
            let sample = state
                .row_samples
                .get(row)
                .cloned()
                .unwrap_or_else(|| "bd".to_string());
            let y = start_y - row as f32 * GRID_CELL_SPACING;

            let mut sprite = Sprite::from_image(long_buttons.off.clone());
            sprite.custom_size = Some(ROW_SAMPLE_BUTTON_SIZE);
            parent
                .spawn((
                    Name::new(format!("Pattern Row Sample Button::{row}")),
                    PatternSampleToggle {
                        owner,
                        row: row as u8,
                    },
                    sprite,
                    Transform::from_xyz(sample_x, y, CONTROL_Z),
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                    Pickable::default(),
                    Hovered(false),
                    InteractiveChild,
                ))
                .observe(toggle_pattern_sample_dropdown);

            parent.spawn((
                Name::new(format!("Pattern Row Sample Label::{row}")),
                PatternRowSampleLabel {
                    owner,
                    row: row as u8,
                },
                Text2d::new(sample),
                TextFont {
                    font: fonts.default_font.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                Transform::from_xyz(sample_x, y - 0.5, LABEL_Z),
                RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                Pickable::IGNORE,
            ));
        }
    });
}

fn spawn_pattern_grid_cells(
    commands: &mut Commands,
    grid_root: Entity,
    owner: Entity,
    state: &PatternNodeUiState,
    checkbox_assets: &UiButtonCheckboxAssets,
) {
    let rows = state.rows as usize;
    let cols = state.cols as usize;
    let start_x = grid_start_x(state.cols);
    let start_y = grid_start_y(state.rows);

    commands.entity(grid_root).with_children(|parent| {
        for row in 0..rows {
            for col in 0..cols {
                let idx = grid_index(row as u8, col as u8, state.cols);
                let on = state.beats.get(idx).copied().unwrap_or(false);
                let accent = state.accents.get(idx).copied().unwrap_or(false);
                let velocity = state.velocities.get(idx).copied().unwrap_or(1.0);
                let probability = state.probabilities.get(idx).copied().unwrap_or(1.0);
                let mut sprite = Sprite::from_image(checkbox_assets.small_check_off.clone());
                sprite.custom_size = Some(Vec2::splat(GRID_CELL_SIZE));
                apply_cell_visual(
                    &mut sprite,
                    on,
                    accent,
                    velocity,
                    probability,
                    checkbox_assets,
                );
                parent
                    .spawn((
                        Name::new(format!("Pattern Cell::{row}::{col}")),
                        PatternBeatCell {
                            owner,
                            row: row as u8,
                            col: col as u8,
                        },
                        sprite,
                        Transform::from_xyz(
                            start_x + col as f32 * GRID_CELL_SPACING,
                            start_y - row as f32 * GRID_CELL_SPACING,
                            CONTROL_Z,
                        ),
                        RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                        Pickable::default(),
                        Hovered(false),
                        InteractiveChild,
                    ))
                    .observe(toggle_pattern_beat_cell);
            }
        }
    });
}

fn grid_start_x(cols: u8) -> f32 {
    -((cols.saturating_sub(1)) as f32 * GRID_CELL_SPACING) * 0.5
}

fn grid_start_y(rows: u8) -> f32 {
    ((rows.saturating_sub(1)) as f32 * GRID_CELL_SPACING) * 0.5
}

fn sample_button_x(cols: u8) -> f32 {
    grid_start_x(cols) - ROW_SAMPLE_X_OFFSET
}

fn toggle_pattern_sample_dropdown(
    click: On<Pointer<Click>>,
    toggles: Query<&PatternSampleToggle>,
    mut states: Query<&mut PatternNodeUiState>,
) {
    let Ok(toggle) = toggles
        .get(click.entity)
        .or_else(|_| toggles.get(click.original_event_target()))
        .or_else(|_| toggles.get(click.event_target()))
    else {
        return;
    };
    let Ok(mut state) = states.get_mut(toggle.owner) else {
        return;
    };
    if state.dropdown_open && state.dropdown_row == Some(toggle.row) {
        state.dropdown_open = false;
        state.dropdown_row = None;
    } else {
        state.dropdown_open = true;
        state.dropdown_row = Some(toggle.row);
        state.help_open = false;
    }
}

fn toggle_pattern_help_popup(
    click: On<Pointer<Click>>,
    toggles: Query<&PatternHelpToggle>,
    mut states: Query<&mut PatternNodeUiState>,
) {
    let Ok(toggle) = toggles
        .get(click.entity)
        .or_else(|_| toggles.get(click.original_event_target()))
        .or_else(|_| toggles.get(click.event_target()))
    else {
        return;
    };

    let Ok(mut state) = states.get_mut(toggle.owner) else {
        return;
    };
    state.help_open = !state.help_open;
    if state.help_open {
        state.dropdown_open = false;
        state.dropdown_row = None;
    }
}

fn close_pattern_help_popup(
    click: On<Pointer<Click>>,
    close_buttons: Query<&PatternHelpClose>,
    mut states: Query<&mut PatternNodeUiState>,
) {
    let Ok(close) = close_buttons
        .get(click.entity)
        .or_else(|_| close_buttons.get(click.original_event_target()))
        .or_else(|_| close_buttons.get(click.event_target()))
    else {
        return;
    };

    let Ok(mut state) = states.get_mut(close.owner) else {
        return;
    };
    state.help_open = false;
}

fn select_pattern_sample(
    click: On<Pointer<Click>>,
    options: Query<&PatternSampleOption>,
    mut states: Query<&mut PatternNodeUiState>,
    node_info: Query<&CanvasNode>,
    mut app_state: ResMut<AppState>,
    mut layout_changed: MessageWriter<LayoutChanged>,
) {
    let Ok(option) = options
        .get(click.entity)
        .or_else(|_| options.get(click.original_event_target()))
        .or_else(|_| options.get(click.event_target()))
    else {
        return;
    };

    let snapshot = {
        let Ok(mut state) = states.get_mut(option.owner) else {
            return;
        };
        let Some(row) = state.dropdown_row else {
            return;
        };
        let row_index = row as usize;
        if row_index >= state.row_samples.len() {
            return;
        }
        state.row_samples[row_index] = option.sample.to_string();
        state.dropdown_open = false;
        state.dropdown_row = None;
        state.clone()
    };
    let Ok(canvas_node) = node_info.get(option.owner) else {
        return;
    };
    persist_pattern_state(
        canvas_node.node_id,
        &snapshot,
        &mut app_state,
        &mut layout_changed,
    );
}

fn resize_pattern_grid(
    click: On<Pointer<Click>>,
    controls: Query<&PatternGridResizeButton>,
    mut states: Query<&mut PatternNodeUiState>,
    node_info: Query<&CanvasNode>,
    mut app_state: ResMut<AppState>,
    mut layout_changed: MessageWriter<LayoutChanged>,
) {
    let Ok(control) = controls
        .get(click.entity)
        .or_else(|_| controls.get(click.original_event_target()))
        .or_else(|_| controls.get(click.event_target()))
    else {
        return;
    };

    let snapshot = {
        let Ok(mut state) = states.get_mut(control.owner) else {
            return;
        };
        let (mut new_rows, mut new_cols) = (state.rows, state.cols);
        match control.axis {
            GridAxis::Rows => {
                new_rows = (state.rows as i16 + control.delta as i16)
                    .clamp(GRID_DIM_MIN as i16, GRID_DIM_MAX as i16)
                    as u8;
            }
            GridAxis::Cols => {
                new_cols = (state.cols as i16 + control.delta as i16)
                    .clamp(GRID_DIM_MIN as i16, GRID_DIM_MAX as i16)
                    as u8;
            }
        }
        if new_rows == state.rows && new_cols == state.cols {
            return;
        }
        state.beats = resize_beats(&state.beats, state.rows, state.cols, new_rows, new_cols);
        state.velocities = resize_step_values(
            &state.velocities,
            state.rows,
            state.cols,
            new_rows,
            new_cols,
            1.0,
        );
        state.accents = resize_step_flags(
            &state.accents,
            state.rows,
            state.cols,
            new_rows,
            new_cols,
            false,
        );
        state.probabilities = resize_step_values(
            &state.probabilities,
            state.rows,
            state.cols,
            new_rows,
            new_cols,
            1.0,
        );
        state.row_samples = resize_row_samples(&state.row_samples, new_rows);
        state.rows = new_rows;
        state.cols = new_cols;
        if state.dropdown_row.is_some_and(|row| row >= new_rows) {
            state.dropdown_open = false;
            state.dropdown_row = None;
        }
        state.grid_dirty = true;
        state.clone()
    };
    let Ok(canvas_node) = node_info.get(control.owner) else {
        return;
    };

    persist_pattern_state(
        canvas_node.node_id,
        &snapshot,
        &mut app_state,
        &mut layout_changed,
    );
}

fn toggle_pattern_beat_cell(
    click: On<Pointer<Click>>,
    cells: Query<&PatternBeatCell>,
    mut states: Query<&mut PatternNodeUiState>,
    mut sprites: Query<&mut Sprite>,
    keyboard: Res<ButtonInput<KeyCode>>,
    checkbox_assets: Res<UiButtonCheckboxAssets>,
    node_info: Query<&CanvasNode>,
    mut app_state: ResMut<AppState>,
    mut layout_changed: MessageWriter<LayoutChanged>,
) {
    let (target, cell) = if let Ok(cell) = cells.get(click.entity) {
        (click.entity, cell)
    } else if let Ok(cell) = cells.get(click.original_event_target()) {
        (click.original_event_target(), cell)
    } else if let Ok(cell) = cells.get(click.event_target()) {
        (click.event_target(), cell)
    } else {
        return;
    };

    let action = resolve_cell_edit_action(&click, &keyboard);
    let (snapshot, active, accent, velocity, probability) = {
        let Ok(mut state) = states.get_mut(cell.owner) else {
            return;
        };
        let total = state.rows as usize * state.cols as usize;
        state.velocities = normalize_step_values(&state.velocities, total, 1.0);
        state.accents = normalize_step_flags(&state.accents, total, false);
        state.probabilities = normalize_step_values(&state.probabilities, total, 1.0);
        let idx = grid_index(cell.row, cell.col, state.cols);
        if idx >= state.beats.len() {
            return;
        }

        match action {
            CellEditAction::ToggleBeat => {
                state.beats[idx] = !state.beats[idx];
                if !state.beats[idx] {
                    state.accents[idx] = false;
                }
            }
            CellEditAction::ToggleAccent => {
                state.beats[idx] = true;
                state.accents[idx] = !state.accents[idx];
            }
            CellEditAction::CycleVelocity => {
                state.beats[idx] = true;
                state.velocities[idx] = next_step_level(state.velocities[idx]);
            }
            CellEditAction::CycleProbability => {
                state.beats[idx] = true;
                state.probabilities[idx] = next_step_level(state.probabilities[idx]);
            }
        }

        (
            state.clone(),
            state.beats[idx],
            state.accents[idx],
            state.velocities[idx],
            state.probabilities[idx],
        )
    };

    if let Ok(mut sprite) = sprites.get_mut(target) {
        apply_cell_visual(
            &mut sprite,
            active,
            accent,
            velocity,
            probability,
            checkbox_assets.as_ref(),
        );
    }
    let Ok(canvas_node) = node_info.get(cell.owner) else {
        return;
    };

    persist_pattern_state(
        canvas_node.node_id,
        &snapshot,
        &mut app_state,
        &mut layout_changed,
    );
}

fn close_pattern_dropdowns_on_global_click(
    click: On<Pointer<Click>>,
    mut nodes: Query<(&PatternNodeUi, &mut PatternNodeUiState)>,
    hierarchy: HierarchyAccess,
) {
    for (ui, mut state) in &mut nodes {
        if !state.dropdown_open {
            continue;
        }

        if pointer_click_touches_root(&click, ui.dropdown_root, &hierarchy)
            || pointer_click_touches_root(&click, ui.lane_root, &hierarchy)
        {
            continue;
        }

        state.dropdown_open = false;
        state.dropdown_row = None;
    }
}

fn rebuild_pattern_grid_when_dirty(
    mut commands: Commands,
    mut nodes: Query<(Entity, &PatternNodeUi, &mut PatternNodeUiState)>,
    checkbox_assets: Res<UiButtonCheckboxAssets>,
    long_buttons: Res<UiButtonLongAssets>,
    fonts: Res<FontAssets>,
) {
    for (owner, ui, mut state) in &mut nodes {
        if !state.grid_dirty {
            continue;
        }
        commands.entity(ui.grid_root).despawn_children();
        commands.entity(ui.lane_root).despawn_children();
        spawn_pattern_row_sample_lanes(
            &mut commands,
            ui.lane_root,
            owner,
            &state,
            long_buttons.as_ref(),
            fonts.as_ref(),
        );
        spawn_pattern_grid_cells(
            &mut commands,
            ui.grid_root,
            owner,
            &state,
            checkbox_assets.as_ref(),
        );
        state.grid_dirty = false;
    }
}

fn sync_pattern_node_interface(
    nodes: Query<(Entity, &PatternNodeUi, &PatternNodeUiState)>,
    mut row_value_labels: Query<(&PatternRowSampleLabel, &mut Text2d)>,
    mut row_buttons: Query<(&PatternSampleToggle, &mut Sprite)>,
    mut texts: Query<&mut Text2d, Without<PatternRowSampleLabel>>,
    mut visibility: Query<&mut Visibility>,
    mut transforms: Query<&mut Transform>,
    long_buttons: Res<UiButtonLongAssets>,
) {
    for (owner, ui, state) in &nodes {
        if let Ok(mut rows_label) = texts.get_mut(ui.rows_label) {
            rows_label.0 = format!("R:{}", state.rows);
        }
        if let Ok(mut cols_label) = texts.get_mut(ui.cols_label) {
            cols_label.0 = format!("C:{}", state.cols);
        }

        for (label, mut text) in &mut row_value_labels {
            if label.owner != owner {
                continue;
            }
            let sample = state
                .row_samples
                .get(label.row as usize)
                .map(String::as_str)
                .unwrap_or("bd");
            text.0 = sample.to_string();
        }

        for (toggle, mut sprite) in &mut row_buttons {
            if toggle.owner != owner {
                continue;
            }
            let open_row = state.dropdown_open && state.dropdown_row == Some(toggle.row);
            sprite.image = if open_row {
                long_buttons.on.clone()
            } else {
                long_buttons.off.clone()
            };
        }

        if let Ok(mut dropdown_visibility) = visibility.get_mut(ui.dropdown_root) {
            *dropdown_visibility = if state.dropdown_open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if let Ok(mut help_visibility) = visibility.get_mut(ui.help_root) {
            *help_visibility = if state.help_open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if let Some(row) = state.dropdown_row
            && let Ok(mut dropdown_transform) = transforms.get_mut(ui.dropdown_root)
        {
            dropdown_transform.translation.x = sample_button_x(state.cols) + 50.0;
            dropdown_transform.translation.y =
                GRID_Y_OFFSET + grid_start_y(state.rows) - row as f32 * GRID_CELL_SPACING - 10.0;
        }
    }
}

fn persist_pattern_state(
    node_id: NodeId,
    state: &PatternNodeUiState,
    app_state: &mut AppState,
    layout_changed: &mut MessageWriter<LayoutChanged>,
) {
    let Some(node) = app_state.project.model.nodes.get_mut(&node_id) else {
        return;
    };
    if !matches!(node.kind, NodeKind::Pattern { .. }) {
        return;
    }

    let default_sample = state
        .row_samples
        .first()
        .map(String::as_str)
        .map(str::trim)
        .filter(|sample| !sample.is_empty())
        .unwrap_or(DEFAULT_SAMPLE)
        .to_string();

    SequencerPatternData {
        rows: state.rows,
        cols: state.cols,
        bits: state.beats.clone(),
        row_samples: state.row_samples.clone(),
        default_sample,
        velocities: state.velocities.clone(),
        accents: state.accents.clone(),
        probabilities: state.probabilities.clone(),
    }
    .write_to_node(node);

    layout_changed.write(LayoutChanged {
        scope: Some(app_state.current_scope),
    });
}

fn pattern_state_from_model(node: &ModelNode) -> PatternNodeUiState {
    let data = SequencerPatternData::from_model_node(node).unwrap_or_default();
    let total = data.rows as usize * data.cols as usize;

    PatternNodeUiState {
        row_samples: data.row_samples,
        rows: data.rows,
        cols: data.cols,
        beats: data.bits,
        velocities: normalize_step_values(&data.velocities, total, 1.0),
        accents: normalize_step_flags(&data.accents, total, false),
        probabilities: normalize_step_values(&data.probabilities, total, 1.0),
        dropdown_open: false,
        dropdown_row: None,
        help_open: false,
        grid_dirty: false,
    }
}

fn resize_row_samples(current: &[String], rows: u8) -> Vec<String> {
    let mut resized = current.to_vec();
    if resized.is_empty() {
        resized.push(DEFAULT_SAMPLE.to_string());
    }
    while resized.len() < rows as usize {
        let fallback = resized
            .last()
            .cloned()
            .unwrap_or_else(|| DEFAULT_SAMPLE.to_string());
        resized.push(fallback);
    }
    resized.truncate(rows as usize);
    resized
}

fn grid_index(row: u8, col: u8, cols: u8) -> usize {
    row as usize * cols as usize + col as usize
}

fn resolve_cell_edit_action(
    click: &On<Pointer<Click>>,
    keyboard: &ButtonInput<KeyCode>,
) -> CellEditAction {
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    let command_or_ctrl = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight);

    if click.button == PointerButton::Secondary || shift {
        return CellEditAction::ToggleAccent;
    }
    if click.button == PointerButton::Middle || command_or_ctrl {
        return CellEditAction::CycleProbability;
    }
    if alt {
        return CellEditAction::CycleVelocity;
    }

    CellEditAction::ToggleBeat
}

fn normalize_step_values(values: &[f32], total: usize, default: f32) -> Vec<f32> {
    if values.is_empty() {
        return vec![default; total];
    }
    let mut out = vec![default; total];
    for (idx, value) in values.iter().copied().enumerate().take(total) {
        out[idx] = clamp_unit(value, default);
    }
    out
}

fn normalize_step_flags(values: &[bool], total: usize, default: bool) -> Vec<bool> {
    if values.is_empty() {
        return vec![default; total];
    }
    let mut out = vec![default; total];
    for (idx, value) in values.iter().copied().enumerate().take(total) {
        out[idx] = value;
    }
    out
}

fn next_step_level(value: f32) -> f32 {
    let value = clamp_unit(value, 1.0);
    STEP_LEVELS
        .iter()
        .copied()
        .find(|candidate| *candidate > value + f32::EPSILON)
        .unwrap_or(STEP_LEVELS[0])
}

fn clamp_unit(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn resize_step_values(
    values: &[f32],
    old_rows: u8,
    old_cols: u8,
    new_rows: u8,
    new_cols: u8,
    default: f32,
) -> Vec<f32> {
    let mut resized = vec![default; new_rows as usize * new_cols as usize];
    let rows = old_rows.min(new_rows) as usize;
    let cols = old_cols.min(new_cols) as usize;
    for row in 0..rows {
        for col in 0..cols {
            let old_idx = row * old_cols as usize + col;
            let new_idx = row * new_cols as usize + col;
            resized[new_idx] = values
                .get(old_idx)
                .copied()
                .map(|value| clamp_unit(value, default))
                .unwrap_or(default);
        }
    }
    resized
}

fn resize_step_flags(
    values: &[bool],
    old_rows: u8,
    old_cols: u8,
    new_rows: u8,
    new_cols: u8,
    default: bool,
) -> Vec<bool> {
    let mut resized = vec![default; new_rows as usize * new_cols as usize];
    let rows = old_rows.min(new_rows) as usize;
    let cols = old_cols.min(new_cols) as usize;
    for row in 0..rows {
        for col in 0..cols {
            let old_idx = row * old_cols as usize + col;
            let new_idx = row * new_cols as usize + col;
            resized[new_idx] = values.get(old_idx).copied().unwrap_or(default);
        }
    }
    resized
}

fn apply_cell_visual(
    sprite: &mut Sprite,
    active: bool,
    accent: bool,
    velocity: f32,
    probability: f32,
    checkbox_assets: &UiButtonCheckboxAssets,
) {
    sprite.image = if active {
        checkbox_assets.small_check_on.clone()
    } else {
        checkbox_assets.small_check_off.clone()
    };

    if !active {
        sprite.color = Color::WHITE;
        return;
    }

    let velocity = clamp_unit(velocity, 1.0);
    let probability = clamp_unit(probability, 1.0);
    let intensity = (0.45 + velocity * 0.55) * (0.55 + probability * 0.45);
    sprite.color = if accent {
        Color::srgba(0.2 + intensity * 0.3, 0.75 + intensity * 0.2, 1.0, 1.0)
    } else {
        let channel = 0.6 + intensity * 0.4;
        Color::srgba(channel, channel, channel, 1.0)
    };
}

fn resize_beats(
    beats: &[bool],
    old_rows: u8,
    old_cols: u8,
    new_rows: u8,
    new_cols: u8,
) -> Vec<bool> {
    let mut resized = vec![false; new_rows as usize * new_cols as usize];
    let rows = old_rows.min(new_rows) as usize;
    let cols = old_cols.min(new_cols) as usize;
    for row in 0..rows {
        for col in 0..cols {
            let old_idx = row * old_cols as usize + col;
            let new_idx = row * new_cols as usize + col;
            resized[new_idx] = beats.get(old_idx).copied().unwrap_or(false);
        }
    }
    resized
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
