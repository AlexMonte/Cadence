use std::collections::BTreeMap;

use serde_json::{Number, Value};

use crate::core::code_expr::CodeExpr;
use crate::core::piece::{ParamDef, ParamSchema, Piece, PieceDef};
use crate::core::types::{PieceCategory, PortType, TileSide};

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
                    required: false,
                }],
                output_type: Some(PortType::Number),
                output_side: Some(TileSide::North),
                description: Some("Numeric constant.".into()),
            },
        }
    }
}

impl Piece for NumberPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
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
                    required: false,
                }],
                output_type: Some(PortType::Text),
                output_side: Some(TileSide::North),
                description: Some("Text constant.".into()),
            },
        }
    }
}

impl Piece for TextPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
        inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("bd".into())))
    }
}
