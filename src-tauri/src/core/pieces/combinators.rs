use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::code_expr::CodeExpr;
use crate::core::piece::{ParamDef, ParamSchema, Piece, PieceDef};
use crate::core::types::{PieceCategory, PortType, TileSide};

pub struct StackPiece {
    def: PieceDef,
}

impl StackPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.stack".into(),
                label: "stack".into(),
                category: PieceCategory::Generator,
                params: vec![
                    ParamDef {
                        id: "a".into(),
                        label: "a".into(),
                        side: TileSide::West,
                        schema: ParamSchema::Pattern { can_inline: false },
                        required: true,
                    },
                    ParamDef {
                        id: "b".into(),
                        label: "b".into(),
                        side: TileSide::North,
                        schema: ParamSchema::Pattern { can_inline: false },
                        required: true,
                    },
                    ParamDef {
                        id: "c".into(),
                        label: "c".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Pattern { can_inline: false },
                        required: false,
                    },
                ],
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Layer two or three patterns in parallel.".into()),
            },
        }
    }
}

impl Piece for StackPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        _inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
        let a = inputs
            .get("a")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing a */".into()));
        let b = inputs
            .get("b")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing b */".into()));
        let mut args = vec![a, b];
        if let Some(c) = inputs.get("c").cloned() {
            args.push(c);
        }
        CodeExpr::Call {
            func: "stack".into(),
            args,
        }
    }
}
