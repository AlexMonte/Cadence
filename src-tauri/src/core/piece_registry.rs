use std::collections::BTreeMap;
use std::sync::Arc;

use crate::core::piece::{Piece, PieceDef};
use crate::core::pieces::{
    FastPiece, GainPiece, MiniPiece, NotePiece, NumberPiece, OutputPiece, RevPiece, SlowPiece,
    SoundPiece, StackPiece, TextPiece,
};

pub struct PieceRegistry {
    pieces: BTreeMap<String, Arc<dyn Piece>>,
}

impl PieceRegistry {
    pub fn new() -> Self {
        Self {
            pieces: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, piece: impl Piece + 'static) {
        let id = piece.def().id.clone();
        self.pieces.insert(id, Arc::new(piece));
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Piece>> {
        self.pieces.get(id).cloned()
    }

    pub fn all_defs(&self) -> Vec<PieceDef> {
        self.pieces
            .values()
            .map(|piece| piece.def().clone())
            .collect()
    }

    pub fn default_strudel() -> Self {
        let mut registry = Self::new();
        registry.register(SoundPiece::new());
        registry.register(NotePiece::new());
        registry.register(MiniPiece::new());
        registry.register(FastPiece::new());
        registry.register(SlowPiece::new());
        registry.register(GainPiece::new());
        registry.register(RevPiece::new());
        registry.register(StackPiece::new());
        registry.register(NumberPiece::new());
        registry.register(TextPiece::new());
        registry.register(OutputPiece::new());
        registry
    }
}
