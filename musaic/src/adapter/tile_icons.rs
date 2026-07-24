//! Tile icon identity and prototype mapping (shared by scene + UI).

use crate::domain::document::{ContainerKind, TileSpawnKind, graph::TilePrototypeId};

pub const TRANSFORM_ICON_FAST: &str = "tiles/transform_fast.png";
pub const TRANSFORM_ICON_SLOW: &str = "tiles/transform_slow.png";
pub const TRANSFORM_ICON_LEGATO: &str = "tiles/transform_legato.png";
pub const TRANSFORM_ICON_GAIN: &str = "tiles/transform_gain.png";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileIconId {
    TransformFast,
    TransformSlow,
    TransformLegato,
    TransformGain,
}

impl TileIconId {
    pub const fn asset_path(self) -> &'static str {
        match self {
            Self::TransformFast => TRANSFORM_ICON_FAST,
            Self::TransformSlow => TRANSFORM_ICON_SLOW,
            Self::TransformLegato => TRANSFORM_ICON_LEGATO,
            Self::TransformGain => TRANSFORM_ICON_GAIN,
        }
    }
}

pub fn icon_for_trick_prototype(prototype: TilePrototypeId) -> TileIconId {
    match prototype.0 {
        0 => TileIconId::TransformFast,
        1 => TileIconId::TransformSlow,
        2 => TileIconId::TransformLegato,
        3 => TileIconId::TransformGain,
        _ => TileIconId::TransformFast,
    }
}

pub fn icon_for_spawn(spawn: &TileSpawnKind) -> Option<TileIconId> {
    match spawn {
        TileSpawnKind::TrickInstance { prototype } => Some(icon_for_trick_prototype(*prototype)),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn icon_for_container(_kind: ContainerKind) -> Option<TileIconId> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trick_prototype_maps_to_drawn_icons() {
        assert_eq!(
            icon_for_trick_prototype(TilePrototypeId(0)),
            TileIconId::TransformFast
        );
        assert_eq!(
            icon_for_trick_prototype(TilePrototypeId(3)),
            TileIconId::TransformGain
        );
    }
}
