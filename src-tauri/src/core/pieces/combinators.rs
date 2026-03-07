use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::strudel_schema::{pattern_port, pattern_schema};
use tile_graph::code_expr::CodeExpr;
use tile_graph::piece::{ParamDef, Piece, PieceDef, PieceInputs};
use tile_graph::types::{PieceCategory, TileSide};

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
                        id: "in_w".into(),
                        label: "in_w".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: true,
                    },
                    ParamDef {
                        id: "in_n".into(),
                        label: "in_n".into(),
                        side: TileSide::North,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_s".into(),
                        label: "in_s".into(),
                        side: TileSide::South,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_e".into(),
                        label: "in_e".into(),
                        side: TileSide::East,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern combinator: stacks all connected pattern inputs in parallel via stack(...). Connect any of in_w/in_n/in_s/in_e to layer voices."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for StackPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let mut args = inputs.get_variadic("patterns").cloned().unwrap_or_default();
        if args.is_empty() {
            args.push(CodeExpr::Raw("/* missing stacked pattern */".into()));
        }
        CodeExpr::Call {
            func: "stack".into(),
            args,
        }
    }
}

pub struct CatPiece {
    def: PieceDef,
}

impl CatPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.cat".into(),
                label: "cat".into(),
                category: PieceCategory::Generator,
                params: vec![
                    ParamDef {
                        id: "in_w".into(),
                        label: "in_w".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: true,
                    },
                    ParamDef {
                        id: "in_n".into(),
                        label: "in_n".into(),
                        side: TileSide::North,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_s".into(),
                        label: "in_s".into(),
                        side: TileSide::South,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_e".into(),
                        label: "in_e".into(),
                        side: TileSide::East,
                        schema: pattern_schema(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern combinator: concatenates connected pattern inputs in sequence via cat(...)."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for CatPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let mut args = inputs.get_variadic("patterns").cloned().unwrap_or_default();
        if args.is_empty() {
            args.push(CodeExpr::Raw("/* missing concatenated pattern */".into()));
        }
        CodeExpr::Call {
            func: "cat".into(),
            args,
        }
    }
}
