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
        _ => None,
    }
}

pub fn prototype_from_transform_kind(kind: TransformKind) -> TilePrototypeId {
    let id = match kind {
        TransformKind::Slow => 0,
        TransformKind::Fast => 1,
        TransformKind::Rev => 2,
        TransformKind::Gain => 3,
        TransformKind::Attack => 4,
        TransformKind::Transpose => 5,
        TransformKind::Degrade => 6,
    };
    TilePrototypeId(id)
}
