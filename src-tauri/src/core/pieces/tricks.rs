use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::strudel_schema::{json_schema, pattern_port, pattern_schema, schema_for_port_type};
use crate::model::CadenceTrickInput;
use tile_graph::code_expr::CodeExpr;
use tile_graph::piece::{ParamDef, ParamSchema, Piece, PieceDef, PieceInputs};
use tile_graph::types::{PieceCategory, PortType, TileSide};

pub const TRICK_INPUT_1_ID: &str = "cadence.trick_input_1";
pub const TRICK_INPUT_2_ID: &str = "cadence.trick_input_2";
pub const TRICK_INPUT_3_ID: &str = "cadence.trick_input_3";
pub const TRICK_OUTPUT_ID: &str = "cadence.trick_output";

const TRICK_PORT_OPTIONS: [&str; 7] = [
    "pattern",
    "number",
    "text",
    "rhythm",
    "bool",
    "trigger",
    "signal",
];

pub struct TrickInputPiece {
    def: PieceDef,
    slot: u8,
}

impl TrickInputPiece {
    pub fn new(slot: u8) -> Self {
        let id = match slot {
            1 => TRICK_INPUT_1_ID,
            2 => TRICK_INPUT_2_ID,
            3 => TRICK_INPUT_3_ID,
            _ => TRICK_INPUT_1_ID,
        };
        Self {
            def: PieceDef {
                id: id.into(),
                label: format!("arg{slot}"),
                category: PieceCategory::Trick,
                params: vec![
                    ParamDef {
                        id: "label".into(),
                        label: "label".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Text {
                            default: format!("input {slot}"),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "port_type".into(),
                        label: "type".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Enum {
                            options: TRICK_PORT_OPTIONS.iter().map(|v| (*v).to_string()).collect(),
                            default: "pattern".into(),
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "required".into(),
                        label: "required".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Bool {
                            default: true,
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "is_receiver".into(),
                        label: "receiver".into(),
                        side: TileSide::South,
                        schema: ParamSchema::Bool {
                            default: false,
                            can_inline: true,
                        },
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "default_value".into(),
                        label: "default".into(),
                        side: TileSide::South,
                        schema: json_schema(None, true),
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(PortType::any()),
                output_side: Some(TileSide::East),
                description: Some(
                    "Trick boundary input. Configure its metadata in the inspector; Cadence uses it as a function argument when compiling the trick."
                        .into(),
                ),
            },
            slot,
        }
    }
}

impl Piece for TrickInputPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, _inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        CodeExpr::Ident(format!("arg{}", self.slot))
    }
}

pub struct TrickOutputPiece {
    def: PieceDef,
}

impl TrickOutputPiece {
    pub fn new() -> Self {
        Self {
            def: PieceDef {
                id: TRICK_OUTPUT_ID.into(),
                label: "return".into(),
                category: PieceCategory::Output,
                params: vec![ParamDef {
                    id: "pattern".into(),
                    label: "pattern".into(),
                    side: TileSide::West,
                    schema: pattern_schema(),
                    variadic_group: None,
                    required: true,
                }],
                output_type: None,
                output_side: None,
                description: Some(
                    "Trick boundary output. Connect the pattern expression this trick should return."
                        .into(),
                ),
            },
        }
    }
}

impl Piece for TrickOutputPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        inputs
            .get("pattern")
            .cloned()
            .unwrap_or_else(|| CodeExpr::Raw("/* missing trick output */".into()))
    }
}

#[derive(Clone)]
pub struct GeneratedTrickPiece {
    def: PieceDef,
    binding_name: String,
    ordered_inputs: Vec<CadenceTrickInput>,
}

impl GeneratedTrickPiece {
    pub fn new(
        trick_id: &str,
        label: &str,
        binding_name: &str,
        ordered_inputs: &[CadenceTrickInput],
    ) -> Self {
        let params = build_generated_params(ordered_inputs);
        Self {
            def: PieceDef {
                id: format!("cadence.trick.{trick_id}"),
                label: label.into(),
                category: PieceCategory::Trick,
                params,
                output_type: Some(pattern_port()),
                output_side: Some(TileSide::East),
                description: Some("User-defined Cadence trick.".into()),
            },
            binding_name: binding_name.into(),
            ordered_inputs: ordered_inputs.to_vec(),
        }
    }
}

impl Piece for GeneratedTrickPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        let mut args = Vec::<Option<CodeExpr>>::with_capacity(self.ordered_inputs.len());

        for input in &self.ordered_inputs {
            let param_id = format!("arg{}", input.slot);
            let value = inputs
                .get(param_id.as_str())
                .cloned()
                .or_else(|| {
                    inline_params.get(param_id.as_str()).and_then(|value| {
                        schema_for_port_type(&input.port_type, input.default_value.clone(), can_inline_for_port(&input.port_type))
                            .inline_expr(value)
                    })
                })
                .or_else(|| default_expr_for_input(input));
            args.push(value);
        }

        while matches!(args.last(), Some(None)) {
            args.pop();
        }

        let mut rendered = Vec::with_capacity(args.len());
        for value in args {
            rendered.push(value.unwrap_or_else(|| CodeExpr::Ident("undefined".into())));
        }

        CodeExpr::Call {
            func: self.binding_name.clone(),
            args: rendered,
        }
    }
}

fn build_generated_params(inputs: &[CadenceTrickInput]) -> Vec<ParamDef> {
    let mut ordered = inputs.to_vec();
    ordered.sort_by_key(|input| (if input.is_receiver { 0 } else { 1 }, input.slot));
    let mut extra_sides = vec![TileSide::South, TileSide::North, TileSide::East];
    let mut params = Vec::with_capacity(ordered.len());

    for (index, input) in ordered.iter().enumerate() {
        let side = if input.is_receiver {
            TileSide::West
        } else if !ordered.iter().any(|value| value.is_receiver) && index == 0 {
            TileSide::West
        } else {
            extra_sides
                .remove(0.min(extra_sides.len().saturating_sub(1)))
        };
        params.push(ParamDef {
            id: format!("arg{}", input.slot),
            label: input.label.clone(),
            side,
            schema: schema_for_port_type(
                &input.port_type,
                input.default_value.clone(),
                can_inline_for_port(&input.port_type),
            ),
            variadic_group: None,
            required: input.required,
        });
    }

    params
}

fn can_inline_for_port(port_type: &PortType) -> bool {
    !matches!(port_type.as_str(), "pattern" | "trigger" | "signal")
}

pub fn default_expr_for_input(input: &CadenceTrickInput) -> Option<CodeExpr> {
    schema_for_port_type(
        &input.port_type,
        input.default_value.clone(),
        can_inline_for_port(&input.port_type),
    )
    .default_expr()
}
