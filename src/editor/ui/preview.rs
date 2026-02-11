//! Optional in-app preview for validating UI assets and style presets.

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::editor::ui::{
    assets::{
        UiBackgroundAssets, UiButtonArrowAssets, UiButtonBigAssets, UiButtonCheckAssets,
        UiButtonCheckboxAssets, UiButtonLongAssets, UiButtonNormalAssets, UiButtonSkewedAssets,
        UiButtonTabAssets, UiButtonTabsAssets, UiComponentAssets, UiFxAssets, UiIconAssets,
        UiIconId, UiKeyIconId, UiKeybindingIconAssets, UiPanelAssets, UiRangeAssets,
    },
    buttons::{
        arrow::button_arrow,
        big::button_big,
        button_small,
        check::check_button as check_text_button,
        checkbox::checkbox_button,
        long::button_long,
        normal::button as button_normal,
        skewed::button_skewed,
        tab::tab_button,
        tabs::{tabs_button, tabs_button_main},
    },
    header, label,
    styles::UiStyles,
    widgets::{
        background::{ui_background_sized, ui_title},
        components::{
            button_slice_bottom_left, button_slice_bottom_right, button_slice_top_left,
            button_slice_top_right, chevron_left, chevron_right, switch_base, switch_head,
            symetric_button, symetric_button_sliced,
        },
        fx::fx_light_blue,
        icons::{icon, icon_button},
        panels::{
            panel_menu_sized, panel_off_sized, panel_on_sized, panel_options_sized, panel_sized,
            panel_v2_off_sized, panel_v2_on_sized, panel_v2_sized,
        },
        range::{range_fill, range_grabber, range_slider, range_track},
        toggles::{
            check_button as check_toggle_button, check_tile, checkbox, checkbox_round,
            checkbox_small,
        },
    },
};
#[derive(Component)]
struct UiPreviewRoot;

const PREVIEW_SECTION_GAP: f32 = 8.0;

fn preview_section_node() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(PREVIEW_SECTION_GAP),
        ..default()
    }
}

fn preview_flow_row_node(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        column_gap: px(gap),
        row_gap: px(gap),
        align_items: AlignItems::Center,
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

#[derive(SystemParam)]
struct UiPreviewResources<'w> {
    styles: Res<'w, UiStyles>,
    button_normal: Res<'w, UiButtonNormalAssets>,
    button_long: Res<'w, UiButtonLongAssets>,
    button_big: Res<'w, UiButtonBigAssets>,
    button_skewed: Res<'w, UiButtonSkewedAssets>,
    button_arrow: Res<'w, UiButtonArrowAssets>,
    button_tab: Res<'w, UiButtonTabAssets>,
    button_tabs: Res<'w, UiButtonTabsAssets>,
    button_check: Res<'w, UiButtonCheckAssets>,
    button_checkbox: Res<'w, UiButtonCheckboxAssets>,
    panels: Res<'w, UiPanelAssets>,
    range: Res<'w, UiRangeAssets>,
    components: Res<'w, UiComponentAssets>,
    icons: Res<'w, UiIconAssets>,
    key_icons: Res<'w, UiKeybindingIconAssets>,
    background: Res<'w, UiBackgroundAssets>,
    fx: Res<'w, UiFxAssets>,
}

#[derive(Resource, Clone, Reflect)]
#[reflect(Resource)]
pub struct UiPreviewConfig {
    pub enabled: bool,
    pub hotkey: KeyCode,
    pub z_index: i32,
}

impl Default for UiPreviewConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hotkey: KeyCode::F9,
            z_index: 900,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<UiPreviewConfig>();
    #[cfg(feature = "ui_reflect")]
    app.register_type::<UiPreviewConfig>();
    app.add_systems(Update, (ensure_preview_spawned, toggle_preview));
}

fn ensure_preview_spawned(
    mut commands: Commands,
    config: Res<UiPreviewConfig>,
    preview_root: Query<Entity, With<UiPreviewRoot>>,
    assets: If<UiPreviewResources>,
) {
    if !config.enabled || !preview_root.is_empty() {
        return;
    }

    spawn_preview(&mut commands, &config, &assets);
}

fn toggle_preview(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<UiPreviewConfig>,
    preview_root: Query<Entity, With<UiPreviewRoot>>,
    assets: If<UiPreviewResources>,
) {
    if !input.just_pressed(config.hotkey) {
        return;
    }

    config.enabled = !config.enabled;

    if config.enabled {
        spawn_preview(&mut commands, &config, &assets);
    } else {
        for entity in &preview_root {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_preview(commands: &mut Commands, config: &UiPreviewConfig, assets: &UiPreviewResources) {
    let styles = assets.styles.as_ref();
    let button_normal_assets = assets.button_normal.as_ref();
    let button_long_assets = assets.button_long.as_ref();
    let button_big_assets = assets.button_big.as_ref();
    let button_skewed_assets = assets.button_skewed.as_ref();
    let button_arrow_assets = assets.button_arrow.as_ref();
    let button_tab_assets = assets.button_tab.as_ref();
    let button_tabs_assets = assets.button_tabs.as_ref();
    let button_check_assets = assets.button_check.as_ref();
    let button_checkbox_assets = assets.button_checkbox.as_ref();
    let panels = assets.panels.as_ref();
    let range = assets.range.as_ref();
    let components = assets.components.as_ref();
    let icons = assets.icons.as_ref();
    let key_icons = assets.key_icons.as_ref();
    let background = assets.background.as_ref();
    let fx = assets.fx.as_ref();

    let panel_size = Vec2::new(180.0, 110.0);
    let panel_menu_size = Vec2::new(200.0, 130.0);
    let panel_options_size = Vec2::new(240.0, 120.0);
    let background_size = Vec2::new(320.0, 180.0);

    let mut alt_styles = styles.clone();
    alt_styles.buttons.normal = alt_styles
        .buttons
        .normal
        .clone()
        .with_size(Vec2::new(220.0, 60.0));
    alt_styles.buttons.normal.text.font_size = 28.0;

    let mut root = commands.spawn((
        Name::new("UI Preview"),
        UiPreviewRoot,
        GlobalZIndex(config.z_index),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::FlexStart,
            padding: UiRect::all(px(16.0)),
            row_gap: px(16.0),
            column_gap: px(16.0),
            ..default()
        },
        Pickable::IGNORE,
    ));

    root.with_children(|parent| {
        parent.spawn((
            Name::new("Preview Legend"),
            Node {
                position_type: PositionType::Absolute,
                top: px(8.0),
                right: px(8.0),
                padding: UiRect::all(px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            children![(
                Name::new("Preview Legend Text"),
                Text(format!("{:?}: Toggle UI preview", config.hotkey)),
                styles.text.label.text_font(),
                TextColor(styles.text.label.color),
            )],
        ));

        parent.spawn(header("UI Preview", styles));

        parent
            .spawn((Name::new("Preview Buttons"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Buttons", styles));
                section
                    .spawn((Name::new("Button Row"), preview_flow_row_node(8.0)))
                    .with_children(|row| {
                        row.spawn(button_normal(
                            "Normal",
                            styles,
                            button_normal_assets,
                            noop_click,
                        ));
                        row.spawn(button_long("Long", styles, button_long_assets, noop_click));
                        row.spawn(button_big("Big", styles, button_big_assets, noop_click));
                        row.spawn(button_skewed(
                            "Skewed",
                            styles,
                            button_skewed_assets,
                            noop_click,
                        ));
                        row.spawn(button_arrow(">", styles, button_arrow_assets, noop_click));
                        row.spawn(tab_button("A", styles, button_tab_assets, noop_click));
                        row.spawn(tabs_button(
                            "Tab",
                            styles,
                            button_tabs_assets.base_images(),
                            noop_click,
                        ));
                        row.spawn(tabs_button_main(
                            "Main",
                            styles,
                            button_tabs_assets.main_images(),
                            noop_click,
                        ));
                        row.spawn(check_text_button(
                            "Check",
                            styles,
                            button_check_assets,
                            noop_click,
                        ));
                        row.spawn(checkbox_button(
                            "Box",
                            styles,
                            button_checkbox_assets,
                            noop_click,
                        ));
                        row.spawn(button_small("+", styles, button_normal_assets, noop_click));
                    });
            });

        parent
            .spawn((Name::new("Preview Overrides"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Style Overrides", styles));
                section
                    .spawn((Name::new("Override Row"), preview_flow_row_node(8.0)))
                    .with_children(|row| {
                        row.spawn(button_normal(
                            "Custom",
                            &alt_styles,
                            button_normal_assets,
                            noop_click,
                        ));
                        row.spawn(range_slider(styles, range, 0.6, true));
                    });
            });

        parent
            .spawn((Name::new("Preview Panels"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Panels", styles));
                section
                    .spawn((Name::new("Panels Row"), preview_flow_row_node(8.0)))
                    .with_children(|row| {
                        row.spawn(panel_sized(styles, panels, panel_size));
                        row.spawn(panel_off_sized(styles, panels, panel_size));
                        row.spawn(panel_on_sized(styles, panels, panel_size));
                        row.spawn(panel_v2_sized(styles, panels, panel_size));
                        row.spawn(panel_v2_off_sized(styles, panels, panel_size));
                        row.spawn(panel_v2_on_sized(styles, panels, panel_size));
                        row.spawn(panel_menu_sized(styles, panels, panel_menu_size));
                        row.spawn(panel_options_sized(styles, panels, panel_options_size));
                    });
            });

        parent
            .spawn((Name::new("Preview Range"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Range + Slider", styles));
                section
                    .spawn((Name::new("Range Row"), preview_flow_row_node(8.0)))
                    .with_children(|row| {
                        row.spawn(range_track(styles, range));
                        row.spawn(range_fill(styles, range));
                        row.spawn(range_grabber(styles, range, false));
                        row.spawn(range_grabber(styles, range, true));
                        row.spawn(range_slider(styles, range, 0.35, false));
                    });
            });

        parent
            .spawn((Name::new("Preview Toggles"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Toggles", styles));
                section
                    .spawn((Name::new("Toggle Row"), preview_flow_row_node(8.0)))
                    .with_children(|row| {
                        row.spawn(checkbox(styles, button_checkbox_assets, false));
                        row.spawn(checkbox(styles, button_checkbox_assets, true));
                        row.spawn(checkbox_small(styles, button_checkbox_assets, false));
                        row.spawn(checkbox_small(styles, button_checkbox_assets, true));
                        row.spawn(checkbox_round(styles, button_checkbox_assets, false));
                        row.spawn(checkbox_round(styles, button_checkbox_assets, true));
                        row.spawn(check_toggle_button(styles, button_check_assets, false));
                        row.spawn(check_toggle_button(styles, button_check_assets, true));
                        row.spawn(check_tile(styles, button_check_assets, true));
                    });
            });

        parent
            .spawn((Name::new("Preview Icons"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Icons", styles));
                section
                    .spawn((Name::new("Icon Row"), preview_flow_row_node(6.0)))
                    .with_children(|row| {
                        row.spawn(icon(styles, icons.get(UiIconId::ArrowDown)));
                        row.spawn(icon(styles, icons.get(UiIconId::ArrowRight)));
                        row.spawn(icon(styles, icons.get(UiIconId::Close)));
                        row.spawn(icon(styles, icons.get(UiIconId::Settings)));
                        row.spawn(icon(styles, icons.get(UiIconId::Refresh)));
                        row.spawn(icon(styles, icons.get(UiIconId::Menu)));
                        row.spawn(icon_button(styles, icons.get(UiIconId::More)));
                        row.spawn(icon_button(styles, icons.get(UiIconId::Snap)));
                    });

                section
                    .spawn((Name::new("Key Icon Row"), preview_flow_row_node(6.0)))
                    .with_children(|row| {
                        row.spawn(icon(styles, key_icons.get(UiKeyIconId::Attack)));
                        row.spawn(icon(styles, key_icons.get(UiKeyIconId::Mouse1)));
                        row.spawn(icon(styles, key_icons.get(UiKeyIconId::MoveUp)));
                        row.spawn(icon(styles, key_icons.get(UiKeyIconId::Dash)));
                    });
            });

        parent
            .spawn((Name::new("Preview Components"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Components", styles));
                section
                    .spawn((Name::new("Component Row"), preview_flow_row_node(6.0)))
                    .with_children(|row| {
                        row.spawn(symetric_button(styles, components));
                        row.spawn(symetric_button_sliced(styles, components));
                        row.spawn(button_slice_top_left(styles, components));
                        row.spawn(button_slice_top_right(styles, components));
                        row.spawn(button_slice_bottom_left(styles, components));
                        row.spawn(button_slice_bottom_right(styles, components));
                        row.spawn(chevron_left(styles, components));
                        row.spawn(chevron_right(styles, components));
                        row.spawn(switch_base(styles, components));
                        row.spawn(switch_head(styles, components));
                    });
            });

        parent
            .spawn((Name::new("Preview Background"), preview_section_node()))
            .with_children(|section| {
                section.spawn(label("Background + FX", styles));
                section
                    .spawn((Name::new("Background Row"), preview_flow_row_node(8.0)))
                    .with_children(|row| {
                        row.spawn(ui_title(styles, background));
                        row.spawn(ui_background_sized(styles, background, background_size));
                        row.spawn(fx_light_blue(styles, fx));
                    });
            });
    });
}

fn noop_click(_: On<Pointer<Click>>) {}
