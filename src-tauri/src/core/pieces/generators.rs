use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::strudel_schema::{pattern_port, pattern_schema};
use tile_graph::code_expr::CodeExpr;
use tile_graph::piece::{ParamDef, ParamSchema, Piece, PieceDef, PieceInputs};
use tile_graph::types::{PieceCategory, TileSide};

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
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "value".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Text {
                            default: "bd".into(),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Sample source or transform: emits s(value) or applies .s(value) when a pattern receiver is connected."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for SoundPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let receiver = inputs.get("pattern").cloned();
        let sample = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("bd".into())));

        if let Some(receiver) = receiver {
            CodeExpr::Method {
                receiver: Box::new(receiver),
                method: "s".into(),
                args: vec![sample],
            }
        } else {
            CodeExpr::Call {
                func: "s".into(),
                args: vec![sample],
            }
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
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "value".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Text {
                            default: "c3".into(),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Note source or transform: emits note(value) or applies .note(value) when a pattern receiver is connected."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for NotePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let receiver = inputs.get("pattern").cloned();
        let notes = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr());
        if let Some(receiver) = receiver {
            CodeExpr::Method {
                receiver: Box::new(receiver),
                method: "note".into(),
                args: notes.into_iter().collect(),
            }
        } else {
            CodeExpr::Call {
                func: "note".into(),
                args: notes
                    .or_else(|| Some(CodeExpr::Literal(Value::String("c3".into()))))
                    .into_iter()
                    .collect(),
            }
        }
    }
}

pub struct NPiece {
    def: PieceDef,
}

impl NPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.n".into(),
                label: "n".into(),
                category: PieceCategory::Generator,
                params: vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::South,
                    schema: ParamSchema::Text {
                        default: "0 2 4 7".into(),
                        can_inline: true,
                    },
                    variadic_group: None,
                    required: false,
                }],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pitch-index generator: creates a pattern with n(value). Use for scale-degree style sequencing."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for NPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("0 2 4 7".into())));
        CodeExpr::Call {
            func: "n".into(),
            args: vec![value],
        }
    }
}
