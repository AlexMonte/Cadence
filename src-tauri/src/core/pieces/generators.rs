use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::code_expr::CodeExpr;
use crate::core::piece::{ParamDef, ParamSchema, Piece, PieceDef};
use crate::core::types::{PieceCategory, PortType, TileSide};

pub struct SoundPiece {
    def: PieceDef,
}

impl SoundPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.sound".into(),
                label: "s".into(),
                category: PieceCategory::Generator,
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
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Create a sample pattern via s().".into()),
            },
        }
    }
}

impl Piece for SoundPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
        let sample = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("bd".into())));

        CodeExpr::Call {
            func: "s".into(),
            args: vec![sample],
        }
    }
}

pub struct NotePiece {
    def: PieceDef,
}

impl NotePiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.note".into(),
                label: "note".into(),
                category: PieceCategory::Generator,
                params: vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::South,
                    schema: ParamSchema::Text {
                        default: "c3".into(),
                        can_inline: true,
                    },
                    required: false,
                }],
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Create a note pattern via note().".into()),
            },
        }
    }
}

impl Piece for NotePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
        let notes = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("c3".into())));
        CodeExpr::Call {
            func: "note".into(),
            args: vec![notes],
        }
    }
}

pub struct MiniPiece {
    def: PieceDef,
}

impl MiniPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.mini".into(),
                label: "mini".into(),
                category: PieceCategory::Generator,
                params: vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::South,
                    schema: ParamSchema::Rhythm {
                        default: "bd sd hh".into(),
                        can_inline: true,
                    },
                    required: false,
                }],
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Create pattern from mini-notation.".into()),
            },
        }
    }
}

impl Piece for MiniPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("bd sd hh".into())));
        CodeExpr::Call {
            func: "mini".into(),
            args: vec![value],
        }
    }
}
