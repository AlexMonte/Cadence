//! Editor audio channels, shared volume model, and UI/audio path constants.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

pub fn plugin(app: &mut App) {
    app
        // Add Kira audio plugins.
        .add_plugins(AudioPlugin)
        // Register component types.
        .register_type::<SoundFxChannel>()
        .add_audio_channel::<SoundFxChannel>()
        .register_type::<UiSoundChannel>()
        .add_audio_channel::<UiSoundChannel>()
        .register_type::<GlobalVolume>()
        .insert_resource(GlobalVolume::new(Volume::Decibels(1.0)))
        .add_systems(
            Update,
            volume_changed.run_if(resource_changed::<GlobalVolume>),
        );
}

/// Audio volume constants.
pub mod volume {
    pub const DEFAULT: f32 = 0.6;
    pub const QUIET: f32 = 0.3;
    pub const LOUD: f32 = 0.8;
    pub const MUSIC: f32 = 0.4;
    pub const SFX: f32 = 0.7;
    pub const UI: f32 = 0.5;
}

/// Basic UI sound effects.
pub mod ui {
    pub const CLICK: &str = "audio/sfx/ui_audio/click3.ogg";
    pub const HOVER: &str = "audio/sfx/ui_audio/rollover3.ogg";
    pub const CONFIRMATION: &str = "audio/sfx/interface/confirmation_001.ogg";
    pub const ERROR: &str = "audio/sfx/interface/error_001.ogg";
}

/// Audio Channels

#[derive(Resource, Default, Reflect, Debug, Clone)]
#[reflect(Resource)]
pub struct SoundFxChannel;

#[derive(Resource, Default, Reflect, Debug, Clone)]
#[reflect(Resource)]
pub struct UiSoundChannel;

fn volume_changed(
    res: Res<GlobalVolume>,
    main_channel: Res<Audio>,
    sfx_channel: Res<AudioChannel<SoundFxChannel>>,
    ui_channel: Res<AudioChannel<UiSoundChannel>>,
) {
    let volume_value = res.into_inner().volume.to_decibels();

    main_channel.set_volume(volume::MUSIC * volume_value);
    sfx_channel.set_volume(volume::SFX * volume_value);
    ui_channel.set_volume(volume::UI * volume_value);
}

// Volume control.
#[derive(Resource, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Resource, Debug, Default, Clone)]
pub struct GlobalVolume {
    /// The global volume of all audio.
    pub volume: Volume,
}

impl From<Volume> for GlobalVolume {
    fn from(volume: Volume) -> Self {
        Self { volume }
    }
}

impl GlobalVolume {
    /// Create a new [`GlobalVolume`] with the given volume.
    pub fn new(volume: Volume) -> Self {
        Self { volume }
    }
}
#[derive(Clone, Copy, Debug, Reflect)]
#[reflect(Clone, Debug, PartialEq)]
pub enum Volume {
    Linear(f32),
    Decibels(f32),
}

impl Default for Volume {
    fn default() -> Self {
        Self::Linear(1.0)
    }
}

impl PartialEq for Volume {
    fn eq(&self, other: &Self) -> bool {
        use Volume::{Decibels, Linear};

        match (self, other) {
            (Linear(a), Linear(b)) => a.abs() == b.abs(),
            (Decibels(a), Decibels(b)) => a == b,
            (a, b) => a.to_decibels() == b.to_decibels(),
        }
    }
}

impl PartialOrd for Volume {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        use Volume::{Decibels, Linear};

        Some(match (self, other) {
            (Linear(a), Linear(b)) => a.abs().total_cmp(&b.abs()),
            (Decibels(a), Decibels(b)) => a.total_cmp(b),
            (a, b) => a.to_decibels().total_cmp(&b.to_decibels()),
        })
    }
}

#[inline]
fn decibels_to_linear(decibels: f32) -> f32 {
    ops::powf(10.0f32, decibels / 20.0)
}

#[inline]
fn linear_to_decibels(linear: f32) -> f32 {
    20.0 * ops::log10(linear.abs())
}
impl Volume {
    /// Returns the volume in linear scale as a float.
    pub fn to_linear(&self) -> f32 {
        match self {
            Self::Linear(v) => v.abs(),
            Self::Decibels(v) => decibels_to_linear(*v),
        }
    }

    /// Returns the volume in decibels as a float.
    ///
    /// If the volume is silent / off / muted, i.e., its underlying linear scale
    /// is `0.0`, this method returns negative infinity.
    pub fn to_decibels(&self) -> f32 {
        match self {
            Self::Linear(v) => linear_to_decibels(*v),
            Self::Decibels(v) => *v,
        }
    }

    /// The silent volume. Also known as "off" or "muted".
    pub const SILENT: Self = Volume::Linear(0.0);
}
