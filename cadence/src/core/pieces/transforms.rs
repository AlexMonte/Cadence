use std::collections::BTreeMap;

use serde_json::Value;

use super::{expr_string_value, require_param_expr, resolve_param_expr, strudel_piece_def};
use crate::core::strudel_schema::{pattern_port, pattern_schema, rhythm_schema};
use tessera::ast::{Expr, ExprKind};
use tessera::parse_ident_path;
use tessera::piece::{
    ParamDef, ParamInlineMode, ParamSchema, ParamTextSemantics, ParamValueKind, Piece, PieceDef,
    PieceInputs,
};
use tessera::types::{PieceCategory, PortType, TileSide};

fn normalize_scale_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains(':') {
        return trimmed.to_string();
    }

    let mut parts = trimmed.split_whitespace().filter(|part| !part.is_empty());
    let Some(tonic) = parts.next() else {
        return String::new();
    };
    let mode = parts.collect::<Vec<_>>().join(" ");
    let tonic = tonic
        .chars()
        .enumerate()
        .map(|(index, ch)| {
            if index == 0 {
                ch.to_ascii_uppercase()
            } else {
                ch
            }
        })
        .collect::<String>();
    if mode.is_empty() {
        tonic
    } else {
        format!("{tonic}:{}", mode.to_ascii_lowercase())
    }
}

fn normalize_scale_expr(expr: Expr) -> Expr {
    expr_string_value(&expr)
        .map(normalize_scale_name)
        .map(Expr::str_lit)
        .unwrap_or(expr)
}

fn pattern_input(
    def: &PieceDef,
    inputs: &PieceInputs,
    inline_params: &BTreeMap<String, Value>,
) -> Expr {
    require_param_expr(def, "pattern", inputs, inline_params, "missing pattern")
}

fn pattern_text_expr(expr: Expr) -> Expr {
    expr_string_value(&expr).map(Expr::pattern).unwrap_or(expr)
}

fn method_transform(
    def: &PieceDef,
    inputs: &PieceInputs,
    inline_params: &BTreeMap<String, Value>,
    method: &str,
    arg_id: &str,
    fallback: Expr,
) -> Expr {
    let pattern = pattern_input(def, inputs, inline_params);
    let value =
        resolve_param_expr(def, arg_id, inputs, inline_params).unwrap_or_else(|| fallback.clone());
    Expr::method_call(pattern, method, vec![value])
}

fn is_ident_path_expr(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Ident { .. } => true,
        ExprKind::Field { object, .. } => is_ident_path_expr(object),
        _ => false,
    }
}

fn coerce_ident_path_expr(expr: Expr) -> Option<Expr> {
    if is_ident_path_expr(&expr) {
        return Some(expr);
    }

    expr_string_value(&expr)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(parse_ident_path)
}

pub struct FastPiece {
    def: PieceDef,
}

impl FastPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.fast",
                "fast",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "factor".into(),
                        label: "x".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 2.0,
                            min: Some(0.125),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .fast(factor). Connect a pattern to 'pattern' and set/route 'factor' to increase playback rate.",
            ),
        }
    }
}

impl Piece for FastPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "fast",
            "factor",
            Expr::float(2.0),
        )
    }
}

pub struct SlowPiece {
    def: PieceDef,
}

impl SlowPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.slow",
                "slow",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "factor".into(),
                        label: "/".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 2.0,
                            min: Some(0.125),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .slow(factor). Connect a pattern to 'pattern' and set/route 'factor' to reduce playback rate.",
            ),
        }
    }
}

impl Piece for SlowPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "slow",
            "factor",
            Expr::float(2.0),
        )
    }
}

pub struct GainPiece {
    def: PieceDef,
}

impl GainPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.gain",
                "gain",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "gain".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 0.8,
                            min: Some(0.0),
                            max: Some(8.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .gain(amount). Use to control amplitude per voice before mixing/output.",
            ),
        }
    }
}

impl Piece for GainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "gain",
            "amount",
            Expr::float(0.8),
        )
    }
}

pub struct RevPiece {
    def: PieceDef,
}

impl RevPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.rev",
                "rev",
                PieceCategory::Transform,
                vec![ParamDef {
                    id: "pattern".into(),
                    label: "pattern".into(),
                    side: TileSide::LEFT,
                    schema: pattern_schema(),
                    text_semantics: Default::default(),
                    variadic_group: None,
                    required: true,
                }],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .rev(). Connect a pattern to reverse event order in time.",
            ),
        }
    }
}

impl Piece for RevPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        Expr::method_call(
            pattern_input(&self.def, inputs, inline_params),
            "rev",
            Vec::new(),
        )
    }
}

pub struct PanPiece {
    def: PieceDef,
}

impl PanPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.pan",
                "pan",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "pan".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 0.5,
                            min: Some(0.0),
                            max: Some(1.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .pan(amount) for stereo placement.",
            ),
        }
    }
}

impl Piece for PanPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "pan",
            "amount",
            Expr::float(0.5),
        )
    }
}

pub struct RoomPiece {
    def: PieceDef,
}

impl RoomPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.room",
                "room",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "room".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 0.2,
                            min: Some(0.0),
                            max: Some(1.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .room(amount) to set reverb room mix.",
            ),
        }
    }
}

impl Piece for RoomPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "room",
            "amount",
            Expr::float(0.2),
        )
    }
}

pub struct SizePiece {
    def: PieceDef,
}

impl SizePiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.size",
                "size",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "size".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 0.5,
                            min: Some(0.0),
                            max: Some(1.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .size(amount) for reverb space sizing.",
            ),
        }
    }
}

impl Piece for SizePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "size",
            "amount",
            Expr::float(0.5),
        )
    }
}

pub struct MaskPiece {
    def: PieceDef,
}

impl MaskPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.mask",
                "mask",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "by".into(),
                        label: "by".into(),
                        side: TileSide::BOTTOM,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .mask(by) to gate events using another pattern.",
            ),
        }
    }
}

impl Piece for MaskPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        let pattern = pattern_input(&self.def, inputs, inline_params);
        let by = require_param_expr(
            &self.def,
            "by",
            inputs,
            inline_params,
            "missing mask pattern",
        );
        Expr::method_call(pattern, "mask", vec![by])
    }
}

pub struct BankPiece {
    def: PieceDef,
}

impl BankPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.bank",
                "bank",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "bank".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: "AlesisHR16".into(),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .bank(value).",
            ),
        }
    }
}

impl Piece for BankPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "bank",
            "value",
            Expr::str_lit("AlesisHR16"),
        )
    }
}

pub struct ClipPiece {
    def: PieceDef,
}

impl ClipPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.clip",
                "clip",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "clip".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 1.0,
                            min: Some(0.0),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .clip(value).",
            ),
        }
    }
}

impl Piece for ClipPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "clip",
            "value",
            Expr::float(1.0),
        )
    }
}

pub struct ReleasePiece {
    def: PieceDef,
}

impl ReleasePiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.release",
                "release",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "release".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 0.5,
                            min: Some(0.0),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .release(value).",
            ),
        }
    }
}

impl Piece for ReleasePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "release",
            "value",
            Expr::float(0.5),
        )
    }
}

pub struct SustainPiece {
    def: PieceDef,
}

impl SustainPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.sustain",
                "sustain",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "sustain".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 0.8,
                            min: Some(0.0),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .sustain(value).",
            ),
        }
    }
}

impl Piece for SustainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "sustain",
            "value",
            Expr::float(0.8),
        )
    }
}

pub struct ScalePiece {
    def: PieceDef,
}

impl ScalePiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.scale",
                "scale",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "scale".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: "C:minor".into(),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .scale(value).",
            ),
        }
    }
}

impl Piece for ScalePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        let pattern = pattern_input(&self.def, inputs, inline_params);
        let value = normalize_scale_expr(
            resolve_param_expr(&self.def, "value", inputs, inline_params)
                .unwrap_or_else(|| Expr::str_lit("C:minor")),
        );
        Expr::method_call(pattern, "scale", vec![value])
    }
}

pub struct TransposePiece {
    def: PieceDef,
}

impl TransposePiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.transpose",
                "transpose",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "steps".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Number {
                            default: 0.0,
                            min: Some(-48.0),
                            max: Some(48.0),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .transpose(value).",
            ),
        }
    }
}

impl Piece for TransposePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        method_transform(
            &self.def,
            inputs,
            inline_params,
            "transpose",
            "value",
            Expr::float(0.0),
        )
    }
}

pub struct StructPiece {
    def: PieceDef,
}

impl StructPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.struct",
                "struct",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "rhythm".into(),
                        side: TileSide::BOTTOM,
                        schema: rhythm_schema("[x]".to_string(), true),
                        text_semantics: ParamTextSemantics::Rhythm,
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .struct(value).",
            ),
        }
    }
}

impl Piece for StructPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        let pattern = pattern_input(&self.def, inputs, inline_params);
        let value = pattern_text_expr(
            resolve_param_expr(&self.def, "value", inputs, inline_params)
                .unwrap_or_else(|| Expr::pattern("[x]")),
        );
        Expr::method_call(pattern, "struct", vec![value])
    }
}

pub struct ApplyPiece {
    def: PieceDef,
}

impl ApplyPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "strudel.apply",
                "apply",
                PieceCategory::Transform,
                vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::LEFT,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "fn_name".into(),
                        label: "fn".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Custom {
                            port_type: PortType::text(),
                            value_kind: ParamValueKind::Text,
                            default: None,
                            can_inline: true,
                            inline_mode: ParamInlineMode::Ident,
                            min: None,
                            max: None,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: true,
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pattern transform: applies .apply(fn). Pass a trick or function name to transform the pattern through a user-defined function.",
            ),
        }
    }
}

impl Piece for ApplyPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> Expr {
        let pattern = pattern_input(&self.def, inputs, inline_params);
        let fn_expr = inputs
            .get("fn_name")
            .cloned()
            .and_then(coerce_ident_path_expr)
            .or_else(|| {
                resolve_param_expr(&self.def, "fn_name", inputs, inline_params)
                    .and_then(coerce_ident_path_expr)
            })
            .unwrap_or_else(|| Expr::error("apply expects a function identifier path"));
        Expr::method_call(pattern, "apply", vec![fn_expr])
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplyPiece, ScalePiece, StructPiece, normalize_scale_name};
    use serde_json::Value;
    use std::collections::BTreeMap;
    use tessera::ast::Expr;
    use tessera::backend::{Backend, JsBackend};
    use tessera::piece::{ParamInlineMode, ParamSchema, ParamTextSemantics, Piece, PieceInputs};

    fn render(expr: &Expr) -> String {
        JsBackend.render(expr)
    }

    #[test]
    fn normalize_scale_name_converts_space_separated_values() {
        assert_eq!(normalize_scale_name("c minor"), "C:minor");
        assert_eq!(normalize_scale_name("bb dorian"), "Bb:dorian");
        assert_eq!(normalize_scale_name("C:major"), "C:major");
    }

    #[test]
    fn scale_piece_normalizes_string_and_pattern_inputs() {
        let piece = ScalePiece::new();

        let mut string_inputs = PieceInputs::default();
        string_inputs.scalar.insert(
            "pattern".into(),
            Expr::call_named("note", vec![Expr::pattern("0 2 4")]),
        );
        string_inputs
            .scalar
            .insert("value".into(), Expr::str_lit("c minor"));
        assert_eq!(
            render(&piece.compile(&string_inputs, &BTreeMap::new())),
            "note(\"0 2 4\").scale('C:minor')"
        );

        let mut pattern_inputs = PieceInputs::default();
        pattern_inputs.scalar.insert(
            "pattern".into(),
            Expr::call_named("note", vec![Expr::pattern("0 2 4")]),
        );
        pattern_inputs
            .scalar
            .insert("value".into(), Expr::pattern("bb dorian"));
        assert_eq!(
            render(&piece.compile(&pattern_inputs, &BTreeMap::new())),
            "note(\"0 2 4\").scale('Bb:dorian')"
        );
    }

    #[test]
    fn struct_piece_value_param_is_marked_as_rhythm_semantics() {
        let piece = StructPiece::new();
        let value = piece
            .def()
            .params
            .iter()
            .find(|param| param.id == "value")
            .expect("value param");
        assert_eq!(value.text_semantics, ParamTextSemantics::Rhythm);
    }

    #[test]
    fn apply_piece_inline_param_uses_ident_mode() {
        let piece = ApplyPiece::new();
        let value = piece
            .def()
            .params
            .iter()
            .find(|param| param.id == "fn_name")
            .expect("fn_name param");
        match &value.schema {
            ParamSchema::Custom { inline_mode, .. } => {
                assert_eq!(*inline_mode, ParamInlineMode::Ident);
            }
            other => panic!("unexpected schema: {other:?}"),
        }
    }

    #[test]
    fn apply_piece_accepts_identifier_paths() {
        let piece = ApplyPiece::new();
        let mut inline_params = BTreeMap::new();
        inline_params.insert("fn_name".into(), Value::String("fx.swing".into()));

        let mut inputs = PieceInputs::default();
        inputs.scalar.insert(
            "pattern".into(),
            Expr::call_named("note", vec![Expr::pattern("c3")]),
        );

        assert_eq!(
            render(&piece.compile(&inputs, &inline_params)),
            "note(\"c3\").apply(fx.swing)"
        );
    }

    #[test]
    fn apply_piece_rejects_arbitrary_expressions() {
        let piece = ApplyPiece::new();
        let mut inline_params = BTreeMap::new();
        inline_params.insert("fn_name".into(), Value::String("fx.swing()".into()));

        let mut inputs = PieceInputs::default();
        inputs.scalar.insert(
            "pattern".into(),
            Expr::call_named("note", vec![Expr::pattern("c3")]),
        );

        let compiled = piece.compile(&inputs, &inline_params);
        assert!(compiled.contains_error());
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

impl_default_from_new!(
    FastPiece,
    SlowPiece,
    GainPiece,
    RevPiece,
    PanPiece,
    RoomPiece,
    SizePiece,
    MaskPiece,
    BankPiece,
    ClipPiece,
    ReleasePiece,
    SustainPiece,
    ScalePiece,
    TransposePiece,
    StructPiece,
    ApplyPiece,
);
