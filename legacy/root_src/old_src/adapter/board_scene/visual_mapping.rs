use crate::domain::board::TileClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileVisualKind {
    LargeContainer,
    FlowTile,
    AtomChip,
    OutputTile,
    GhostTile,
    InvalidPreview,
}

pub fn visual_kind_for_class(class: TileClass) -> TileVisualKind {
    match class {
        TileClass::Container => TileVisualKind::LargeContainer,
        TileClass::Transform => TileVisualKind::FlowTile,
        TileClass::Atom => TileVisualKind::AtomChip,
        TileClass::Terminal => TileVisualKind::OutputTile,
    }
}
