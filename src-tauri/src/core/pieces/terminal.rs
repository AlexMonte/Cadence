use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::code_expr::CodeExpr;
use crate::core::piece::{ParamDef, ParamSchema, Piece, PieceDef};
use crate::core::types::{PieceCategory, TileSide};

pub struct OutputPiece {
    def: PieceDef,
}

impl OutputPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.output".into(),
                label: "play".into(),
                category: PieceCategory::Output,
                params: vec![ParamDef {
                    id: "pattern".into(),
                    label: "pattern".into(),
                    side: TileSide::West,
                    schema: ParamSchema::Pattern { can_inline: false },
                    required: true,
                }],
                output_type: None,
                output_side: None,
                description: Some("Terminal output node.".into()),
            },
        }
    }
}

impl Piece for OutputPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        _inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
        inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing terminal input */".into()))
    }
}
