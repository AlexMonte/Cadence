use tessera::piece_registry::PieceRegistry;

use crate::core::pieces::{
    ApplyPiece, BankPiece, CatPiece, ClipPiece, ConnectorPiece, CrossConnectorPiece, FastPiece,
    GainPiece, MaskPiece, MiniPiece, NPiece, NotePiece, NumberPiece, OutputPiece, PanPiece,
    ReleasePiece, RevPiece, RoomPiece, ScalePiece, SizePiece, SlowPiece, SoundPiece, StackPiece,
    StructPiece, SustainPiece, TextPiece, TransposePiece,
};

pub(crate) fn register_strudel_pieces(registry: &mut PieceRegistry) {
    registry.register(SoundPiece::new());
    registry.register(NotePiece::new());
    registry.register(NPiece::new());
    registry.register(StackPiece::new());
    registry.register(CatPiece::new());
    registry.register(FastPiece::new());
    registry.register(SlowPiece::new());
    registry.register(GainPiece::new());
    registry.register(RevPiece::new());
    registry.register(PanPiece::new());
    registry.register(RoomPiece::new());
    registry.register(SizePiece::new());
    registry.register(MaskPiece::new());
    registry.register(BankPiece::new());
    registry.register(ClipPiece::new());
    registry.register(ReleasePiece::new());
    registry.register(SustainPiece::new());
    registry.register(ScalePiece::new());
    registry.register(TransposePiece::new());
    registry.register(StructPiece::new());
    registry.register(ApplyPiece::new());
    registry.register(NumberPiece::new());
    registry.register(MiniPiece::new());
    registry.register(TextPiece::new());
    registry.register(OutputPiece::new());
    registry.register(ConnectorPiece::new());
    registry.register(CrossConnectorPiece::new());
}

#[cfg(test)]
pub(crate) fn default_strudel_registry() -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_strudel_pieces(&mut registry);
    registry
}
