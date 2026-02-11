//! Loading screen and lightweight loading animation.

use crate::shared::resources::ResourceHandles;
use crate::{
    AppSystemsSet,
    editor::{
        screens::Screen,
        ui::{UiStyles, label, ui_root},
    },
};
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    log::info!("Initializing loading screen plugin");

    app.add_systems(OnEnter(Screen::Loading), spawn_loading_screen);
    app.add_systems(
        Update,
        update_loading_animation
            .run_if(in_state(Screen::Loading))
            .in_set(AppSystemsSet::Update),
    );
    app.add_systems(
        Update,
        exit_loading_screen.run_if(
            in_state(Screen::Loading)
                .and(|resource_handles: Res<ResourceHandles>| resource_handles.is_all_done()),
        ),
    );
}

const ANIMATION_FRAMES: [[u8; 8]; 8] = [
    [1, 2, 3, 0, 0, 0, 0, 0],
    [2, 3, 4, 0, 0, 0, 0, 0],
    [3, 4, 5, 0, 0, 0, 0, 0],
    [4, 5, 6, 0, 0, 0, 0, 0],
    [5, 6, 7, 0, 0, 0, 0, 0],
    [6, 7, 8, 0, 0, 0, 0, 0],
    [7, 8, 1, 0, 0, 0, 0, 0],
    [8, 1, 2, 0, 0, 0, 0, 0],
];
const INITIAL_BRAILLE_DOTS: [u8; 8] = [1, 2, 3, 4, 0, 0, 0, 0];

#[derive(Resource)]
struct LoadingAnimations {
    frames: [[u8; 8]; 8],
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct LoadingAnimation {
    current: [u8; 8],
    index: usize,
}

fn spawn_loading_screen(mut commands: Commands, styles: Res<UiStyles>) {
    let braille = render_braille(&INITIAL_BRAILLE_DOTS);

    commands.spawn((
        ui_root("Loading Screen"),
        children![
            label(braille.to_string(), &styles),
            LoadingAnimation {
                current: INITIAL_BRAILLE_DOTS,
                index: 0,
            },
        ],
    ));
    commands.insert_resource(LoadingAnimations {
        frames: ANIMATION_FRAMES,
    });
}

fn exit_loading_screen(mut next_screen: ResMut<NextState<Screen>>) {
    // log::info!("Exiting loading screen");
    next_screen.set(Screen::Editor);
}

fn update_loading_animation(
    mut animation: Query<(&mut Text, &mut LoadingAnimation)>,
    anims: Res<LoadingAnimations>,
) {
    if let Ok((mut text, mut anim)) = animation.single_mut() {
        anim.current = anims.frames[anim.index];
        anim.index = (anim.index + 1) % anims.frames.len();
        text.0 = render_braille(&anim.current).into();
    }
}

fn render_braille(dots: &[u8]) -> char {
    let mut mask = 0u32;
    for &dot in dots {
        if (1..=8).contains(&dot) {
            mask |= 1 << (dot - 1);
        }
    }
    std::char::from_u32(0x2800 + mask).unwrap()
}
