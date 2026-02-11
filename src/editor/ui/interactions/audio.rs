//! Optional UI interaction sound effects.

use bevy::prelude::*;
use bevy_kira_audio::{AudioChannel, AudioControl, prelude as kira};

use crate::editor::audio::{UiSoundChannel, ui::*};
use crate::shared::resources::LoadResource;

pub fn plugin(app: &mut App) {
    #[cfg(feature = "ui_reflect")]
    app.register_type::<InteractionAudioAssets>();
    app.load_resource::<InteractionAudioAssets>();
    app.add_observer(play_on_hover_sound_effect);
    app.add_observer(play_on_click_sound_effect);
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct InteractionAudioAssets {
    #[dependency]
    hover: Handle<kira::AudioSource>,
    #[dependency]
    click: Handle<kira::AudioSource>,
}

impl FromWorld for InteractionAudioAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            hover: assets.load(HOVER),
            click: assets.load(CLICK),
        }
    }
}

fn play_on_hover_sound_effect(
    trigger: On<Pointer<Over>>,
    interaction_assets: Option<Res<InteractionAudioAssets>>,
    interaction_query: Query<(), With<Interaction>>,
    ui_audio_channel: Res<AudioChannel<UiSoundChannel>>,
) {
    let Some(interaction_assets) = interaction_assets else {
        return;
    };

    if interaction_target_hit(
        trigger.entity,
        trigger.original_event_target(),
        trigger.event_target(),
        &interaction_query,
    ) {
        ui_audio_channel.play(interaction_assets.hover.clone());
    }
}

fn play_on_click_sound_effect(
    trigger: On<Pointer<Click>>,
    interaction_assets: Option<Res<InteractionAudioAssets>>,
    interaction_query: Query<(), With<Interaction>>,
    ui_channel: Res<AudioChannel<UiSoundChannel>>,
) {
    let Some(interaction_assets) = interaction_assets else {
        return;
    };

    if interaction_target_hit(
        trigger.entity,
        trigger.original_event_target(),
        trigger.event_target(),
        &interaction_query,
    ) {
        ui_channel.play(interaction_assets.click.clone());
    }
}

fn interaction_target_hit(
    entity: Entity,
    original_target: Entity,
    event_target: Entity,
    interaction_query: &Query<(), With<Interaction>>,
) -> bool {
    interaction_query.contains(entity)
        || interaction_query.contains(original_target)
        || interaction_query.contains(event_target)
}
