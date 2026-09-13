use tessera::prelude::TransformKind;

use super::document::graph::TilePrototypeId;

/// Maps drawer trick prototype ids to Tessera transform kinds.
pub fn transform_kind_from_prototype(prototype: TilePrototypeId) -> Option<TransformKind> {
    match prototype.0 {
        0 => Some(TransformKind::Slow),
        1 => Some(TransformKind::Fast),
        2 => Some(TransformKind::Rev),
        3 => Some(TransformKind::Gain),
        4 => Some(TransformKind::Attack),
        5 => Some(TransformKind::Transpose),
        6 => Some(TransformKind::Degrade),
        7 => Some(TransformKind::Gate),
        8 => Some(TransformKind::Legato),
        9 => Some(TransformKind::Sustain),
        10 => Some(TransformKind::LowPassCutoff),
        11 => Some(TransformKind::LowPassResonance),
        12 => Some(TransformKind::SampleVariant),
        13 => Some(TransformKind::Decay),
        14 => Some(TransformKind::Release),
        15 => Some(TransformKind::Pan),
        16 => Some(TransformKind::HighPassCutoff),
        17 => Some(TransformKind::HighPassResonance),
        18 => Some(TransformKind::Velocity),
        19 => Some(TransformKind::ClipLength),
        20 => Some(TransformKind::PostGain),
        21 => Some(TransformKind::PitchBend),
        22 => Some(TransformKind::Expression),
        23 => Some(TransformKind::PlaybackRate),
        24 => Some(TransformKind::PlaybackStart),
        25 => Some(TransformKind::PlaybackEnd),
        26 => Some(TransformKind::Reverse),
        27 => Some(TransformKind::Fit),
        28 => Some(TransformKind::Loop),
        29 => Some(TransformKind::Delay),
        30 => Some(TransformKind::Reverb),
        31 => Some(TransformKind::Compressor),
        32 => Some(TransformKind::Wire),
        1000.. => Some(TransformKind::Trick),
        _ => None,
    }
}
