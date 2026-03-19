use std::collections::BTreeMap;

use serde_json::Value;

use super::strudel_piece_def;
use crate::core::strudel_schema::{pattern_port, pattern_schema};
use tessera::ast::Expr;
use tessera::piece::{ParamDef, Piece, PieceDef, PieceInputs};
use tessera::types::{PieceCategory, TileSide};

pub struct StackPiece {
    def: PieceDef,
}

impl StackPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.stack",
                "stack",
                PieceCategory::Generator,
                vec![
                    ParamDef {
                        id: "in_w".into(),
                        label: "in_w".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: true,
                    },
                    ParamDef {
                        id: "in_n".into(),
                        label: "in_n".into(),
                        side: TileSide::TOP,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_s".into(),
                        label: "in_s".into(),
                        side: TileSide::BOTTOM,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_e".into(),
                        label: "in_e".into(),
                        side: TileSide::RIGHT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern combinator: stacks all connected pattern inputs in parallel via stack(...). Connect any of in_w/in_n/in_s/in_e to layer voices.",
            ),
        }
    }
}

impl Piece for StackPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> Expr {
        let mut args = inputs.get_variadic("patterns").cloned().unwrap_or_default();
        if args.is_empty() {
            args.push(Expr::error("missing stacked pattern"));
        }
        Expr::call_named("stack", args)
    }
}

pub struct CatPiece {
    def: PieceDef,
}

impl CatPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.cat",
                "cat",
                PieceCategory::Generator,
                vec![
                    ParamDef {
                        id: "in_w".into(),
                        label: "in_w".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: true,
                    },
                    ParamDef {
                        id: "in_n".into(),
                        label: "in_n".into(),
                        side: TileSide::TOP,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_s".into(),
                        label: "in_s".into(),
                        side: TileSide::BOTTOM,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                    ParamDef {
                        id: "in_e".into(),
                        label: "in_e".into(),
                        side: TileSide::RIGHT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: Some("patterns".into()),
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern combinator: concatenates connected pattern inputs in sequence via cat(...).",
            ),
        }
    }
}

impl Piece for CatPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> Expr {
        let mut args = inputs.get_variadic("patterns").cloned().unwrap_or_default();
        if args.is_empty() {
            args.push(Expr::error("missing concatenated pattern"));
        }
        Expr::call_named("cat", args)
    }
}

macro_rules! impl_default_from_new {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Default for $ty {
                fn default() -> Self {
                    Self::new()
                }
            }
        )*
    };
}

impl_default_from_new!(StackPiece, CatPiece);
