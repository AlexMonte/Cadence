use tile_graph::piece_registry::PieceRegistry;

use crate::core::pieces::{
    ApplyPiece, BankPiece, CatPiece, ClipPiece, FastPiece, GainPiece, MaskPiece, NPiece,
    NotePiece, NumberPiece, OutputPiece, PanPiece, ReleasePiece, RevPiece, RoomPiece, ScalePiece,
    SizePiece, SlowPiece, SoundPiece, StackPiece, StructPiece, SustainPiece, TextPiece,
    TransposePiece, TrickInputPiece, TrickOutputPiece,
};
use crate::core::tricks::{CompiledTrick, compile_tricks, runtime_trick_pieces};
use crate::model::CadenceProjectDocument;

fn register_builtins(registry: &mut PieceRegistry) {
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
    registry.register(TextPiece::new());
    registry.register(OutputPiece::new());
}

pub(crate) fn default_strudel_registry() -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_builtins(&mut registry);
    registry
}

pub(crate) fn trick_editor_registry() -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_builtins(&mut registry);
    registry.register(TrickInputPiece::new(1));
    registry.register(TrickInputPiece::new(2));
    registry.register(TrickInputPiece::new(3));
    registry.register(TrickOutputPiece::new());
    registry
}

pub(crate) fn runtime_registry(project: &CadenceProjectDocument) -> PieceRegistry {
    let trick_registry = trick_editor_registry();
    let (compiled, _) = compile_tricks(project, &trick_registry);
    runtime_registry_from_compiled(&compiled)
}

pub(crate) fn runtime_registry_from_compiled(compiled: &[CompiledTrick]) -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_builtins(&mut registry);
    for piece in runtime_trick_pieces(compiled) {
        registry.register(piece);
    }
    registry
}
