use crate::domain::board::TileClass;
use crate::infrastructure::dto::PieceDef;

pub fn classify_piece(piece: &PieceDef) -> TileClass {
    classify_piece_id(piece.id.as_str())
}

pub fn classify_piece_id(piece_id: &str) -> TileClass {
    match piece_id {
        "cadence.output" => TileClass::Terminal,
        "cadence.container.basic"
        | "cadence.container.subdivide"
        | "cadence.container.alternate"
        | "cadence.container.parallel" => TileClass::Container,
        "cadence.atom.note"
        | "cadence.atom.scalar"
        | "cadence.atom.rest"
        | "cadence.atom.operator.elongation"
        | "cadence.atom.operator.pitch_shift"
        | "cadence.atom.operator.slow"
        | "cadence.atom.operator.fast" => TileClass::Atom,
        _ => TileClass::Transform,
    }
}
