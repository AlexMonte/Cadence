use std::collections::BTreeMap;

use serde_json::{Number, Value};

use crate::core::code_expr::CodeExpr;
use crate::core::piece::{ParamDef, ParamSchema, Piece, PieceDef};
use crate::core::types::{PieceCategory, PortType, TileSide};

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
                        schema: ParamSchema::Pattern { can_inline: false },
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
                        required: false,
                    },
                ],
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Speed up a pattern by a factor.".into()),
            },
        }
    }
}

impl Piece for FastPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
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
                        schema: ParamSchema::Pattern { can_inline: false },
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
                        required: false,
                    },
                ],
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Slow down a pattern by a factor.".into()),
            },
        }
    }
}

impl Piece for SlowPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
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
                        schema: ParamSchema::Pattern { can_inline: false },
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
                        required: false,
                    },
                ],
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Set output gain.".into()),
            },
        }
    }
}

impl Piece for GainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
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
                    schema: ParamSchema::Pattern { can_inline: false },
                    required: true,
                }],
                output_type: Some(PortType::Pattern),
                output_side: Some(TileSide::East),
                description: Some("Reverse pattern order.".into()),
            },
        }
    }
}

impl Piece for RevPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        _inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr {
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
