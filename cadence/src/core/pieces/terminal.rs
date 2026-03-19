use std::collections::BTreeMap;

use serde_json::Value;

use super::{require_param_expr, strudel_piece_def};
use crate::core::strudel_schema::pattern_schema;
use tessera::ast::Expr;
use tessera::piece::{ParamDef, Piece, PieceDef, PieceInputs};
use tessera::types::{PieceCategory, TileSide};

pub struct OutputPiece {
    def: PieceDef,
}

impl OutputPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.output",
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
                }],
                None,
                None,
                "Terminal sink: commits a pattern chain as one program voice. Use one output per independent voice path.",
            ),
        }
    }
}

impl Piece for OutputPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        require_param_expr(
            &self.def,
            "pattern",
            inputs,
            inline_params,
            "missing terminal input",
        )
    }
}

impl Default for OutputPiece {
    fn default() -> Self {
        Self::new()
    }
}
