//! Splash screen timing and fade animation.

use crate::shared::resources::ResourceHandles;
use crate::{
    AppSystemsSet,
    editor::{screens::Screen, ui::ui_root},
};
use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    input::common_conditions::input_just_pressed,
    prelude::*,
};

const SPLASH_BACKGROUND_COLOR: Color = Color::srgb(0.157, 0.157, 0.157);
const SPLASH_DURATION_SECS: f32 = 1.8;
const SPLASH_FADE_DURATION_SECS: f32 = 0.6;

pub(super) fn plugin(app: &mut App) {
    log::info!("Initializing splash screen plugin");
    app.insert_resource(ClearColor(Color::BLACK));

    app.add_systems(OnEnter(Screen::Splash), spawn_splash_screen);

    // Animate splash screen.
    app.add_systems(
        Update,
        (
            tick_fade_in_out.in_set(AppSystemsSet::TickTimers),
            apply_fade_in_out.in_set(AppSystemsSet::Update),
        )
            .run_if(in_state(Screen::Splash)),
    );

    app.add_systems(OnEnter(Screen::Splash), insert_splash_timer);
    app.add_systems(OnExit(Screen::Splash), remove_splash_timer);
    app.add_systems(
        Update,
        (
            tick_splash_timer.in_set(AppSystemsSet::TickTimers),
            check_splash_timer.in_set(AppSystemsSet::Update),
        )
            .run_if(in_state(Screen::Splash)),
    );

    // Exit the splash screen early if the player hits escape.
    app.add_systems(
        Update,
        enter_main_menu_screen.run_if(
            input_just_pressed(KeyCode::Escape)
                .and(in_state(Screen::Splash))
                .and(|resource_handles: Res<ResourceHandles>| resource_handles.is_all_done()),
        ),
    );
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
struct SplashTimer(Timer);

impl Default for SplashTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(SPLASH_DURATION_SECS, TimerMode::Once))
    }
}

fn insert_splash_timer(mut commands: Commands) {
    log::info!("Inserting splash timer");
    commands.init_resource::<SplashTimer>();
}

fn remove_splash_timer(mut commands: Commands) {
    log::info!("Removing splash timer");
    commands.remove_resource::<SplashTimer>();
}

fn tick_splash_timer(time: Res<Time>, mut timer: ResMut<SplashTimer>) {
    timer.0.tick(time.delta());
}

fn check_splash_timer(timer: Res<SplashTimer>, mut next_screen: ResMut<NextState<Screen>>) {
    if timer.0.just_finished() {
        transition_to_editor(&mut next_screen);
    }
}

fn enter_main_menu_screen(mut next_screen: ResMut<NextState<Screen>>) {
    transition_to_editor(&mut next_screen);
}

fn transition_to_editor(next_screen: &mut ResMut<NextState<Screen>>) {
    log::info!("Loading done, entering editor screen");
    next_screen.set(Screen::Editor);
}

fn spawn_splash_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    log::info!("Spawning splash screen");

    commands.spawn((
        ui_root("Splash Screen"),
        BackgroundColor(SPLASH_BACKGROUND_COLOR),
        DespawnOnExit(Screen::Splash),
        ImageNode::new(asset_server.load("images/ui/background.png")),
        ImageNodeFadeInOut {
            total_duration: SPLASH_DURATION_SECS,
            fade_duration: SPLASH_FADE_DURATION_SECS,
            time: 0.0,
        },
        children![(
            Name::new("Splash image"),
            Node {
                margin: UiRect::all(Val::Auto),
                width: Val::Percent(100.0),
                ..default()
            },
            ImageNode::new(asset_server.load_with_settings(
                // This should be an embedded asset for instant loading, but that is
                // currently [broken on Windows Wasm builds](https://github.com/bevyengine/bevy/issues/14246).
                "images/ui/title.png",
                |settings: &mut ImageLoaderSettings| {
                    // Make an exception for the splash image in case
                    // `ImagePlugin::default_nearest()` is used for pixel art.
                    settings.sampler = ImageSampler::nearest();
                },
            )),
            ImageNodeFadeInOut {
                total_duration: SPLASH_DURATION_SECS,
                fade_duration: SPLASH_FADE_DURATION_SECS,
                time: 0.0,
            },
        )],
    ));
}

// component to fade in and out an ImageNode
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ImageNodeFadeInOut {
    total_duration: f32,
    fade_duration: f32,
    time: f32,
}

impl ImageNodeFadeInOut {
    fn alpha(&self) -> f32 {
        let time: f32 = (self.time / self.total_duration).clamp(0.0, 1.0);
        let fade: f32 = self.fade_duration / self.total_duration;

        ((1.0 - (2.0 * time - 1.0).abs()) / fade).min(1.0)
    }
}

fn tick_fade_in_out(time: Res<Time>, mut query: Query<&mut ImageNodeFadeInOut>) {
    for mut animation in query.iter_mut() {
        animation.time += time.delta_secs();
    }
}

fn apply_fade_in_out(
    mut animation_query: Query<(&mut ImageNode, &ImageNodeFadeInOut)>,
    mut image_query: Query<(&ImageNodeFadeInOut, &mut BackgroundColor), Without<ImageNode>>,
) {
    for (mut image, anim) in &mut animation_query {
        image.color.set_alpha(anim.alpha());
    }
    for (anim, mut bg_image) in image_query.iter_mut() {
        bg_image.0.set_alpha(anim.alpha());
    }
}
