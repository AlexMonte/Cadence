use crate::adapter::tessera::cadence_schema::pattern_schema;
use crate::adapter::tessera::pieces::cadence_piece_def_with_tags;
use tessera::piece::{ParamDef, Piece, PieceDef};
use tessera::types::{PieceCategory, TileSide};

pub struct OutputPiece {
    def: PieceDef,
}

impl OutputPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.output",
                "play",
                PieceCategory::Output,
                vec![ParamDef {
                    id: "pattern".into(),
                    label: "pattern".into(),
                    side: TileSide::LEFT,
                    schema: pattern_schema(),
                    text_semantics: Default::default(),
                    variadic_group: None,
                    required: true,
                    role: Default::default(),
                }],
                None,
                None,
                "Terminal sink: commits a pattern chain as one program voice.",
                Vec::new(),
            ),
        }
    }
}

impl Piece for OutputPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

impl Default for OutputPiece {
    fn default() -> Self {
        Self::new()
    }
}
