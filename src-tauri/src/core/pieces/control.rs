use std::collections::BTreeMap;

use serde_json::{Number, Value};

use crate::core::strudel_schema::{
    pattern_port, pattern_schema, signal_port, trigger_port, trigger_schema,
};
use tile_graph::code_expr::CodeExpr;
use tile_graph::piece::{ParamDef, ParamSchema, Piece, PieceDef, PieceInputs};
use tile_graph::types::{PieceCategory, TileSide};

pub struct ClockPiece {
    def: PieceDef,
}

impl ClockPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.clock".into(),
                label: "clock".into(),
                category: PieceCategory::Control,
                params: vec![ParamDef {
                    id: "every".into(),
                    label: "every".into(),
                    side: TileSide::South,
                    schema: ParamSchema::Number {
                        default: 1.0,
                        min: Some(0.125),
                        max: Some(64.0),
                        can_inline: true,
                    },
                    variadic_group: None,
                    required: false,
                }],
                output_type: Some(trigger_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Control source: emits trigger pulses every N cycles. Connect to trigger inputs (for example gate/if) to drive rhythmic decisions."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for ClockPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let every = inputs
            .get("every")
            .cloned()
            .or_else(|| inline_params.get("every").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(1))));
        CodeExpr::Method {
            receiver: Box::new(CodeExpr::Call {
                func: "mini".into(),
                args: vec![CodeExpr::Literal(Value::String("x ~ ~ ~".into()))],
            }),
            method: "slow".into(),
            args: vec![every],
        }
    }
}

pub struct CounterPiece {
    def: PieceDef,
}

impl CounterPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.counter".into(),
                label: "counter".into(),
                category: PieceCategory::Control,
                params: vec![ParamDef {
                    id: "max".into(),
                    label: "max".into(),
                    side: TileSide::South,
                    schema: ParamSchema::Number {
                        default: 16.0,
                        min: Some(1.0),
                        max: Some(1024.0),
                        can_inline: true,
                    },
                    variadic_group: None,
                    required: false,
                }],
                output_type: Some(signal_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Control source: emits a cycle counter signal in [0, max). Use to modulate/select behavior over time."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for CounterPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let max = inputs
            .get("max")
            .cloned()
            .or_else(|| inline_params.get("max").cloned().map(CodeExpr::Literal))
            .or_else(|| self.def.params[0].schema.default_expr())
            .unwrap_or_else(|| CodeExpr::Literal(Value::Number(Number::from(16))));
        CodeExpr::Call {
            func: "run".into(),
            args: vec![max],
        }
    }
}

pub struct GatePiece {
    def: PieceDef,
}

impl GatePiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.gate".into(),
                label: "gate".into(),
                category: PieceCategory::Control,
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
                        id: "trigger".into(),
                        label: "trigger".into(),
                        side: TileSide::South,
                        schema: trigger_schema(),
                        variadic_group: None,
                        required: true,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Control gate: passes 'pattern' only when 'trigger' is active (mask). Use for rhythmic muting/chopping."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for GatePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let pattern = inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing gate pattern */".into()));
        let trigger = inputs
            .get("trigger")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing gate trigger */".into()));
        CodeExpr::Method {
            receiver: Box::new(pattern),
            method: "mask".into(),
            args: vec![trigger],
        }
    }
}

pub struct IfPiece {
    def: PieceDef,
}

impl IfPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: "strudel.if".into(),
                label: "if".into(),
                category: PieceCategory::Control,
                params: vec![
                    ParamDef {
                        id: "when_true".into(),
                        label: "true".into(),
                        side: TileSide::West,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "when_false".into(),
                        label: "false".into(),
                        side: TileSide::North,
                        schema: pattern_schema(),
                        variadic_group: None,
                        required: true,
                    },
                    ParamDef {
                        id: "trigger".into(),
                        label: "trigger".into(),
                        side: TileSide::South,
                        schema: trigger_schema(),
                        variadic_group: None,
                        required: true,
                    },
                ],
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Control router: chooses between 'true' and 'false' patterns using trigger state. Use for conditional pattern switching."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for IfPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let when_true = inputs
            .get("when_true")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing when_true */".into()));
        let when_false = inputs
            .get("when_false")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing when_false */".into()));
        let trigger = inputs
            .get("trigger")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing if trigger */".into()));

        let false_mask = CodeExpr::Call {
            func: "inv".into(),
            args: vec![trigger.clone()],
        };
        CodeExpr::Call {
            func: "stack".into(),
            args: vec![
                CodeExpr::Method {
                    receiver: Box::new(when_true),
                    method: "mask".into(),
                    args: vec![trigger],
                },
                CodeExpr::Method {
                    receiver: Box::new(when_false),
                    method: "mask".into(),
                    args: vec![false_mask],
                },
            ],
        }
    }
}
