use std::collections::BTreeMap;

use serde_json::Value;

use super::{expr_string_value, resolve_param_expr, strudel_piece_def};
use tessera::ast::Expr;
use tessera::piece::{ParamDef, ParamSchema, ParamTextSemantics, Piece, PieceDef, PieceInputs};
use tessera::types::{PieceCategory, PortType, TileSide};

fn contains_method_call_chain(value: &str) -> bool {
    let bytes = value.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] != b'.' {
            continue;
        }
        let Some(next) = bytes.get(index + 1).copied() else {
            continue;
        };
        let next = next as char;
        if !next.is_ascii_alphabetic() && next != '_' {
            continue;
        }
        if value[index + 1..].contains('(') {
            return true;
        }
    }
    false
}

fn looks_like_mini_expression(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    trimmed.starts_with('"')
        || trimmed.starts_with('\'')
        || trimmed.starts_with('`')
        || trimmed.contains("=>")
        || contains_method_call_chain(trimmed)
        || (trimmed
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphabetic() || ch == '_')
            .unwrap_or(false)
            && trimmed.contains('('))
}

fn compile_mini_piece_value(expr: Expr) -> Expr {
    let Some(value) = expr_string_value(&expr) else {
        return expr;
    };

    let trimmed = value.trim();
    if looks_like_mini_expression(trimmed) {
        Expr::error("strudel.mini only accepts mini-notation patterns")
    } else {
        Expr::pattern(trimmed)
    }
}

pub struct NumberPiece {
    def: PieceDef,
}

impl NumberPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.number",
                "number",
                PieceCategory::Constant,
                vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::BOTTOM,
                    schema: ParamSchema::Number {
                        default: 1.0,
                        min: None,
                        max: None,
                        can_inline: true,
                    },
                    text_semantics: Default::default(),
                    variadic_group: None,
                    required: false,
                }],
                Some(PortType::number()),
                Some(TileSide::TOP),
                "Constant number source. Use to drive numeric params (for example fast factor, gain amount, clock period).",
            ),
        }
    }
}

impl Piece for NumberPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        resolve_param_expr(&self.def, "value", inputs, inline_params)
            .unwrap_or_else(|| Expr::int(1))
    }
}

pub struct MiniPiece {
    def: PieceDef,
}

impl MiniPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.mini",
                "mini",
                PieceCategory::Constant,
                vec![ParamDef {
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
                }],
                Some(PortType::text()),
                Some(TileSide::TOP),
                "Constant mininotation source. Use to feed Mininotation params when you want reusable values instead of inline literals.",
            ),
        }
    }
}

impl Piece for MiniPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        compile_mini_piece_value(
            resolve_param_expr(&self.def, "value", inputs, inline_params)
                .unwrap_or_else(|| Expr::str_lit("")),
        )
    }
}

pub struct TextPiece {
    def: PieceDef,
}

impl TextPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.text",
                "text",
                PieceCategory::Constant,
                vec![ParamDef {
                    id: "value".into(),
                    label: "value".into(),
                    side: TileSide::BOTTOM,
                    schema: ParamSchema::Text {
                        default: "".into(),
                        can_inline: true,
                    },
                    text_semantics: Default::default(),
                    variadic_group: None,
                    required: false,
                }],
                Some(PortType::text()),
                Some(TileSide::TOP),
                "Constant text source. Use to feed text/string params when you want reusable values instead of inline literals.",
            ),
        }
    }
}

impl Piece for TextPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        resolve_param_expr(&self.def, "value", inputs, inline_params)
            .unwrap_or_else(|| Expr::str_lit(""))
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

impl_default_from_new!(NumberPiece, MiniPiece, TextPiece);

#[cfg(test)]
mod tests {
    use super::{MiniPiece, compile_mini_piece_value};
    use std::collections::BTreeMap;
    use tessera::ast::Expr;
    use tessera::backend::{Backend, JsBackend};
    use tessera::piece::{ParamTextSemantics, Piece, PieceInputs};
    use tessera::types::TileSide;

    fn render(expr: &Expr) -> String {
        JsBackend.render(expr)
    }

    #[test]
    fn mini_piece_keeps_plain_mininotation_as_a_pattern_string() {
        assert_eq!(
            render(&compile_mini_piece_value(Expr::str_lit("bd sd"))),
            "\"bd sd\""
        );
    }

    #[test]
    fn mini_piece_rejects_expression_shorthand() {
        let source = "<-4 -2 0 -1>\".struct(\"[[x ~]!2 x x@0.5 [x ~]!2 x@0.5 [x ~]!2]\")";
        assert_eq!(
            render(&compile_mini_piece_value(Expr::str_lit(source))),
            "/* strudel.mini only accepts mini-notation patterns */"
        );
    }

    #[test]
    fn mini_piece_compile_rejects_expression_shorthand() {
        let piece = MiniPiece::new();
        let mut inputs = PieceInputs::default();
        inputs.scalar.insert(
            "value".into(),
            Expr::str_lit("<-4 -2 0 -1>\".struct(\"[x ~ x ~]\")"),
        );

        let compiled = piece.compile(&inputs, &BTreeMap::new());
        assert!(compiled.contains_error());
    }

    #[test]
    fn mini_piece_value_param_is_marked_as_mini_semantics() {
        let piece = MiniPiece::new();
        let value = piece
            .def()
            .params
            .iter()
            .find(|param| param.id == "value")
            .expect("value param");
        assert_eq!(value.text_semantics, ParamTextSemantics::Mini);
        assert_eq!(value.side, TileSide::BOTTOM);
    }
}
