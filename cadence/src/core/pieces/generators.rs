use std::collections::BTreeMap;

use serde_json::Value;

use super::{expr_string_value, resolve_param_expr, strudel_piece_def};
use crate::core::strudel_schema::{pattern_port, pattern_schema};
use tessera::ast::Expr;
use tessera::piece::{ParamDef, ParamSchema, ParamTextSemantics, Piece, PieceDef, PieceInputs};
use tessera::types::{PieceCategory, TileSide};

fn looks_like_mini_notation_sample(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && (trimmed
            .chars()
            .any(|ch| matches!(ch, '[' | ']' | '<' | '>' | '!' | '~' | '@' | ',' | '*'))
            || trimmed.split_whitespace().count() > 1)
}

fn pattern_if_string(expr: Expr) -> Expr {
    expr_string_value(&expr).map(Expr::pattern).unwrap_or(expr)
}

fn sound_value_expr(expr: Expr) -> Expr {
    match expr_string_value(&expr) {
        Some(value) if looks_like_mini_notation_sample(value) => Expr::pattern(value),
        _ => expr,
    }
}

pub struct SoundPiece {
    def: PieceDef,
}

impl SoundPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.sound",
                "s",
                PieceCategory::Generator,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "value".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: "".into(),
                            can_inline: true,
                        },
                        text_semantics: ParamTextSemantics::Mini,
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Sample source or transform: emits s(value) or applies .s(value) when a pattern receiver is connected.",
            ),
        }
    }
}

impl Piece for SoundPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        let receiver = inputs.get("pattern").cloned();
        let sample = sound_value_expr(
            resolve_param_expr(&self.def, "value", inputs, inline_params)
                .unwrap_or_else(|| Expr::str_lit("")),
        );

        if let Some(receiver) = receiver {
            Expr::method_call(receiver, "s", vec![sample])
        } else {
            Expr::call_named("s", vec![sample])
        }
    }
}

pub struct NotePiece {
    def: PieceDef,
}

impl NotePiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.note",
                "note",
                PieceCategory::Generator,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "value".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: "c3".into(),
                            can_inline: true,
                        },
                        text_semantics: ParamTextSemantics::Mini,
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Note source or transform: emits note(value) or applies .note(value) when a pattern receiver is connected.",
            ),
        }
    }
}

impl Piece for NotePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        let receiver = inputs.get("pattern").cloned();
        let notes = pattern_if_string(
            resolve_param_expr(&self.def, "value", inputs, inline_params)
                .unwrap_or_else(|| Expr::pattern("c3")),
        );

        if let Some(receiver) = receiver {
            Expr::method_call(receiver, "note", vec![notes])
        } else {
            Expr::call_named("note", vec![notes])
        }
    }
}

pub struct NPiece {
    def: PieceDef,
}

impl NPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.n",
                "n",
                PieceCategory::Generator,
                vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::BOTTOM,
                    schema: ParamSchema::Text {
                        default: "0 2 4 7".into(),
                        can_inline: true,
                    },
                    text_semantics: ParamTextSemantics::Mini,
                    variadic_group: None,
                    required: false,
                }],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pitch-index generator: creates a pattern with n(value). Use for scale-degree style sequencing.",
            ),
        }
    }
}

impl Piece for NPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        let value = pattern_if_string(
            resolve_param_expr(&self.def, "value", inputs, inline_params)
                .unwrap_or_else(|| Expr::pattern("0 2 4 7")),
        );
        Expr::call_named("n", vec![value])
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

impl_default_from_new!(SoundPiece, NotePiece, NPiece);

#[cfg(test)]
mod tests {
    use super::{NPiece, NotePiece, SoundPiece};
    use tessera::piece::{ParamTextSemantics, Piece};

    #[test]
    fn generator_value_params_are_marked_as_mini_semantics() {
        for piece in [
            SoundPiece::new().def(),
            NotePiece::new().def(),
            NPiece::new().def(),
        ] {
            let value = piece
                .params
                .iter()
                .find(|param| param.id == "value")
                .expect("value param");
            assert_eq!(value.text_semantics, ParamTextSemantics::Mini);
        }
    }
}
