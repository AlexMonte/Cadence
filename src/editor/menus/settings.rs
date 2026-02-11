//! The settings menu.
//!
//! Additional settings and accessibility options should go here.

use bevy::{input::common_conditions::input_just_pressed, prelude::*};

use crate::editor::{
    audio::{GlobalVolume, Volume},
    menus::{Menu, MenuAssets},
    screens::Screen,
    ui::{
        UiStyles,
        buttons::{button_small, normal::button},
        header, label,
        styles::ButtonImages,
        ui_root,
    },
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Settings), spawn_settings_menu);
    app.add_systems(
        Update,
        go_back.run_if(in_state(Menu::Settings).and(input_just_pressed(KeyCode::Escape))),
    );

    app.add_systems(
        Update,
        update_global_volume_label.run_if(in_state(Menu::Settings)),
    );
}

fn spawn_settings_menu(mut commands: Commands, assets: Res<MenuAssets>, styles: Res<UiStyles>) {
    commands.spawn((
        ui_root("Settings Menu"),
        GlobalZIndex(2),
        DespawnOnExit(Menu::Settings),
        children![
            header("Settings", &styles),
            settings_grid(assets.as_ref(), &styles),
            button("Back", &styles, assets.as_ref(), go_back_on_click),
        ],
    ));
}

fn settings_grid(assets: &MenuAssets, styles: &UiStyles) -> impl Bundle {
    (
        Name::new("Settings Grid"),
        Node {
            display: Display::Grid,
            row_gap: px(10),
            column_gap: px(30),
            grid_template_columns: RepeatedGridTrack::px(2, 400.0),
            ..default()
        },
        children![
            (
                label("Master Volume", styles),
                Node {
                    justify_self: JustifySelf::End,
                    ..default()
                }
            ),
            global_volume_widget(assets, styles),
        ],
    )
}

fn global_volume_widget(assets: &MenuAssets, styles: &UiStyles) -> impl Bundle {
    let images = ButtonImages::from(assets);
    (
        Name::new("Global Volume Widget"),
        Node {
            justify_self: JustifySelf::Start,
            ..default()
        },
        children![
            button_small("-", styles, images.clone(), lower_global_volume),
            (
                Name::new("Current Volume"),
                Node {
                    padding: UiRect::horizontal(px(10)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                children![(label("", styles), GlobalVolumeLabel)],
            ),
            button_small("+", styles, images, raise_global_volume),
        ],
    )
}

const MIN_VOLUME: f32 = 0.0;
const MAX_VOLUME: f32 = 3.0;

fn lower_global_volume(_: On<Pointer<Click>>, mut global_volume: ResMut<GlobalVolume>) {
    let linear = (global_volume.volume.to_linear() - 0.1).max(MIN_VOLUME);
    global_volume.volume = Volume::Linear(linear);
}

fn raise_global_volume(_: On<Pointer<Click>>, mut global_volume: ResMut<GlobalVolume>) {
    let linear = (global_volume.volume.to_linear() + 0.1).min(MAX_VOLUME);
    global_volume.volume = Volume::Linear(linear);
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct GlobalVolumeLabel;

fn update_global_volume_label(
    global_volume: Res<GlobalVolume>,
    mut label: Single<&mut Text, With<GlobalVolumeLabel>>,
) {
    let percent = 100.0 * global_volume.volume.to_linear();
    label.0 = format!("{percent:3.0}%");
}

fn go_back_on_click(
    _: On<Pointer<Click>>,
    screen: Res<State<Screen>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    next_menu.set(back_menu_for_screen(&screen));
}

fn go_back(screen: Res<State<Screen>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(back_menu_for_screen(&screen));
}

fn back_menu_for_screen(screen: &State<Screen>) -> Menu {
    if screen.get() == &Screen::Title {
        Menu::ProjectHub
    } else {
        Menu::None
    }
}
