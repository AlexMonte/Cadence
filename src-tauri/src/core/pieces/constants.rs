use std::collections::BTreeMap;

use serde_json::{Number, Value};

use tile_graph::code_expr::CodeExpr;
use tile_graph::piece::{ParamDef, ParamSchema, Piece, PieceDef, PieceInputs};
use tile_graph::types::{PieceCategory, PortType, TileSide};

pub struct NumberPiece {
    def: PieceDef,
}

impl NumberPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.number".into(),
                label: "number".into(),
                category: PieceCategory::Constant,
                params: vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::South,
                    schema: ParamSchema::Number {
                        default: 1.0,
                        min: None,
                        max: None,
                        can_inline: true,
                    },
                    variadic_group: None,
                    required: false,
                }],
                output_type: Some(PortType::number()),
                output_side: Some(TileSide::North),
                description: Some(
                    "Constant number source. Use to drive numeric params (for example fast factor, gain amount, clock period)."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for NumberPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(1))))
    }
}

pub struct TextPiece {
    def: PieceDef,
}

impl TextPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.text".into(),
                label: "text".into(),
                category: PieceCategory::Constant,
                params: vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::South,
                    schema: ParamSchema::Text {
                        default: "bd".into(),
                        can_inline: true,
                    },
                    variadic_group: None,
                    required: false,
                }],
                output_type: Some(PortType::text()),
                output_side: Some(TileSide::North),
                description: Some(
                    "Constant text source. Use to feed text/string params when you want reusable values instead of inline literals."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for TextPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("bd".into())))
    }
}
