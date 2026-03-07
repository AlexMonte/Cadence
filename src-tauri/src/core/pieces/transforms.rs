use std::collections::BTreeMap;

use serde_json::{Number, Value};

use crate::core::strudel_schema::{pattern_port, pattern_schema, rhythm_schema};
use tile_graph::code_expr::CodeExpr;
use tile_graph::piece::{ParamDef, ParamSchema, Piece, PieceDef, PieceInputs};
use tile_graph::types::{PieceCategory, TileSide};

pub struct FastPiece {
    def: PieceDef,
}

impl FastPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.fast".into(),
                label: "fast".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "factor".into(),
                        label: "x".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 2.0,
                            min: Some(0.125),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .fast(factor). Connect a pattern to 'pattern' and set/route 'factor' to increase playback rate."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for FastPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let factor = inputs
            .get("factor")
            .cloned()
            .or_else(|| inline_params.get("factor").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(2))));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "fast".into(),
            args: vec![factor],
        }
    }
}

pub struct SlowPiece {
    def: PieceDef,
}

impl SlowPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.slow".into(),
                label: "slow".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "factor".into(),
                        label: "/".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 2.0,
                            min: Some(0.125),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .slow(factor). Connect a pattern to 'pattern' and set/route 'factor' to reduce playback rate."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for SlowPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let factor = inputs
            .get("factor")
            .cloned()
            .or_else(|| inline_params.get("factor").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(2))));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "slow".into(),
            args: vec![factor],
        }
    }
}

pub struct GainPiece {
    def: PieceDef,
}

impl GainPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.gain".into(),
                label: "gain".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "gain".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 0.8,
                            min: Some(0.0),
                            max: Some(8.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .gain(amount). Use to control amplitude per voice before mixing/output."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for GainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let amount = inputs
            .get("amount")
            .cloned()
            .or_else(|| inline_params.get("amount").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| {
                CodeExpr::Literal(Value::Number(
                    Number::from_f64(0.8).unwrap_or_else(|| Number::from(1)),
                ))
            });
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "gain".into(),
            args: vec![amount],
        }
    }
}

pub struct RevPiece {
    def: PieceDef,
}

impl RevPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.rev".into(),
                label: "rev".into(),
                category: PieceCategory::Transform,
                params: vec![ParamDef {
                    id: "pattern".into(),
                    label: "pattern".into(),
                    side: TileSide::West,
                    schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                }],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .rev(). Connect a pattern to reverse event order in time."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for RevPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "rev".into(),
            args: Vec::new(),
        }
    }
}

pub struct PanPiece {
    def: PieceDef,
}

impl PanPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.pan".into(),
                label: "pan".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "pan".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 0.5,
                            min: Some(0.0),
                            max: Some(1.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .pan(amount) for stereo placement.".into(),
                ),
            },
        }
    }
}

impl Piece for PanPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let amount = inputs
            .get("amount")
            .cloned()
            .or_else(|| inline_params.get("amount").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| {
                CodeExpr::Literal(Value::Number(
                    Number::from_f64(0.5).unwrap_or_else(|| Number::from(0)),
                ))
            });
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "pan".into(),
            args: vec![amount],
        }
    }
}

pub struct RoomPiece {
    def: PieceDef,
}

impl RoomPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.room".into(),
                label: "room".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "room".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 0.2,
                            min: Some(0.0),
                            max: Some(1.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .room(amount) to set reverb room mix.".into(),
                ),
            },
        }
    }
}

impl Piece for RoomPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let amount = inputs
            .get("amount")
            .cloned()
            .or_else(|| inline_params.get("amount").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| {
                CodeExpr::Literal(Value::Number(
                    Number::from_f64(0.2).unwrap_or_else(|| Number::from(0)),
                ))
            });
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "room".into(),
            args: vec![amount],
        }
    }
}

pub struct SizePiece {
    def: PieceDef,
}

impl SizePiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.size".into(),
                label: "size".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "amount".into(),
                        label: "size".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 0.5,
                            min: Some(0.0),
                            max: Some(1.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .size(amount) for reverb space sizing.".into(),
                ),
            },
        }
    }
}

impl Piece for SizePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let amount = inputs
            .get("amount")
            .cloned()
            .or_else(|| inline_params.get("amount").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| {
                CodeExpr::Literal(Value::Number(
                    Number::from_f64(0.5).unwrap_or_else(|| Number::from(0)),
                ))
            });
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "size".into(),
            args: vec![amount],
        }
    }
}

pub struct MaskPiece {
    def: PieceDef,
}

impl MaskPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.mask".into(),
                label: "mask".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "by".into(),
                        label: "by".into(),
                        side: TileSide::South,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .mask(by) to gate events using another pattern."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for MaskPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let by = inputs
            .get("by")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing mask pattern */".into()));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "mask".into(),
            args: vec![by],
        }
    }
}

pub struct BankPiece {
    def: PieceDef,
}

impl BankPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.bank".into(),
                label: "bank".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "bank".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Text {
                            default: "AlesisHR16".into(),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("Pattern transform: applies .bank(value).".into()),
            },
        }
    }
}

impl Piece for BankPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("AlesisHR16".into())));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "bank".into(),
            args: vec![value],
        }
    }
}

pub struct ClipPiece {
    def: PieceDef,
}

impl ClipPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.clip".into(),
                label: "clip".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "clip".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 1.0,
                            min: Some(0.0),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("Pattern transform: applies .clip(value).".into()),
            },
        }
    }
}

impl Piece for ClipPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(1))));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "clip".into(),
            args: vec![value],
        }
    }
}

pub struct ReleasePiece {
    def: PieceDef,
}

impl ReleasePiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.release".into(),
                label: "release".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "release".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 0.5,
                            min: Some(0.0),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("Pattern transform: applies .release(value).".into()),
            },
        }
    }
}

impl Piece for ReleasePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| {
                CodeExpr::Literal(Value::Number(
                    Number::from_f64(0.5).unwrap_or_else(|| Number::from(0)),
                ))
            });
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "release".into(),
            args: vec![value],
        }
    }
}

pub struct SustainPiece {
    def: PieceDef,
}

impl SustainPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.sustain".into(),
                label: "sustain".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "sustain".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 0.8,
                            min: Some(0.0),
                            max: Some(32.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("Pattern transform: applies .sustain(value).".into()),
            },
        }
    }
}

impl Piece for SustainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| {
                CodeExpr::Literal(Value::Number(
                    Number::from_f64(0.8).unwrap_or_else(|| Number::from(0)),
                ))
            });
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "sustain".into(),
            args: vec![value],
        }
    }
}

pub struct ScalePiece {
    def: PieceDef,
}

impl ScalePiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.scale".into(),
                label: "scale".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "scale".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Text {
                            default: "c minor".into(),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("Pattern transform: applies .scale(value).".into()),
            },
        }
    }
}

impl Piece for ScalePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("c minor".into())));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "scale".into(),
            args: vec![value],
        }
    }
}

pub struct TransposePiece {
    def: PieceDef,
}

impl TransposePiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.transpose".into(),
                label: "transpose".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "steps".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Number {
                            default: 0.0,
                            min: Some(-48.0),
                            max: Some(48.0),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("Pattern transform: applies .transpose(value).".into()),
            },
        }
    }
}

impl Piece for TransposePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(0))));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "transpose".into(),
            args: vec![value],
        }
    }
}

pub struct StructPiece {
    def: PieceDef,
}

impl StructPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.struct".into(),
                label: "struct".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "value".into(),
                        label: "rhythm".into(),
                        side: TileSide::South,
                        schema: rhythm_schema("[x]".to_string(), true),
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("Pattern transform: applies .struct(value).".into()),
            },
        }
    }
}

impl Piece for StructPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let value = inputs
            .get("value")
            .cloned()
            .or_else(|| inline_params.get("value").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[1].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::String("[x]".into())));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "struct".into(),
            args: vec![value],
        }
    }
}

pub struct ApplyPiece {
    def: PieceDef,
}

impl ApplyPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.apply".into(),
                label: "apply".into(),
                category: PieceCategory::Transform,
                params: vec![
                    ParamDef {
                        id: "pattern".into(),
                        label: "pattern".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "fn_name".into(),
                        label: "fn".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Text {
                            default: "".into(),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: true,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Pattern transform: applies .apply(fn). Pass a trick or function name to transform the pattern through a user-defined function."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for ApplyPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing pattern */".into()));
        let fn_expr = inputs
            .get("fn_name")
            .cloned()
            .or_else(|| {
                inline_params
                    .get("fn_name")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(|s| CodeExpr::Ident(s.to_string()))
            })
            .unwrap_or_else(|| CodeExpr::Raw("/* missing fn */".into()));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "apply".into(),
            args: vec![fn_expr],
        }
    }
}
