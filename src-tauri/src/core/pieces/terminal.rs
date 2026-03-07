use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::strudel_schema::pattern_schema;
use tile_graph::code_expr::CodeExpr;
use tile_graph::piece::{ParamDef, Piece, PieceDef, PieceInputs};
use tile_graph::types::{PieceCategory, TileSide};

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
                    schema: pattern_schema(),
                    variadic_group: None,
                    required: true,
                }],
                output_type: None,
                output_side: None,
                description: Some(
                    "Terminal sink: commits a pattern chain as one program voice. Use one output per independent voice path."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for OutputPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing terminal input */".into()))
    }
}
