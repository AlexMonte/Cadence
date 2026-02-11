//! The load-project menu.

use bevy::{input::common_conditions::input_just_pressed, prelude::*};

use crate::editor::{
    menus::{Menu, MenuAssets},
    screens::Screen,
    ui::{UiStyles, buttons::normal::button, header, ui_root},
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::LoadProject), spawn_load_menu);
    app.add_systems(
        Update,
        go_back.run_if(in_state(Menu::LoadProject).and(input_just_pressed(KeyCode::Escape))),
    );
}

fn spawn_load_menu(mut commands: Commands, assets: Res<MenuAssets>, styles: Res<UiStyles>) {
    commands.spawn((
        ui_root("Load Project Menu"),
        GlobalZIndex(2),
        DespawnOnExit(Menu::LoadProject),
        children![
            header("Load Project", &styles),
            button("Back", &styles, assets.as_ref(), go_back_on_click),
        ],
    ));
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
