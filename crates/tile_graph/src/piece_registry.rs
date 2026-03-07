use std::collections::BTreeMap;
use std::sync::Arc;

use crate::piece::{Piece, PieceDef};

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
}
