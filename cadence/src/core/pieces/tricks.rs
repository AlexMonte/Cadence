use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::strudel_schema::{json_schema, pattern_schema, schema_for_port_type};
use crate::model::CadenceTrickInput;
use tessera::code_expr::CodeExpr;
use tessera::piece::{ParamDef, ParamSchema, Piece, PieceDef, PieceInputs};
use tessera::types::{PieceCategory, PortType, TileSide};

pub const TRICK_INPUT_1_ID: &str = "cadence.trick_input_1";
pub const TRICK_INPUT_2_ID: &str = "cadence.trick_input_2";
pub const TRICK_INPUT_3_ID: &str = "cadence.trick_input_3";
pub const TRICK_OUTPUT_ID: &str = "cadence.trick_output";

const TRICK_PORT_OPTIONS: [&str; 7] = [
    "pattern", "number", "text", "rhythm", "bool", "trigger", "signal",
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
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: format!("input {slot}"),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "port_type".into(),
                        label: "type".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Enum {
                            options: TRICK_PORT_OPTIONS.iter().map(|v| (*v).to_string()).collect(),
                            default: "pattern".into(),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "required".into(),
                        label: "required".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Bool {
                            default: true,
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "is_receiver".into(),
                        label: "receiver".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Bool {
                            default: false,
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "default_value".into(),
                        label: "default".into(),
                        side: TileSide::BOTTOM,
                        schema: json_schema(None, true),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                output_type: Some(PortType::any()),
                output_side: Some(TileSide::RIGHT),
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
                    side: TileSide::LEFT,
                    schema: pattern_schema(),
                    text_semantics: Default::default(),
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
                output_type: Some(PortType::any()),
                output_side: Some(TileSide::RIGHT),
                description: Some("User-defined Cadence trick.".into()),
            },
            binding_name: binding_name.to_lowercase().into(),
            ordered_inputs: ordered_inputs.to_vec(),
        }
    }
}

impl Piece for GeneratedTrickPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr {
        // Zero-input tricks are value declarations — just reference the binding.
        if self.ordered_inputs.is_empty() {
            return CodeExpr::Ident(self.binding_name.clone());
        }

        let resolved = self
            .ordered_inputs
            .iter()
            .map(|input| resolve_generated_input(input, inputs, inline_params))
            .collect::<Vec<_>>();

        let receiver = resolved.iter().find(|entry| entry.input.is_receiver);
        if let Some(receiver) = receiver {
            if receiver.expr.is_none() {
                let has_explicit_partial_args = resolved
                    .iter()
                    .any(|entry| !entry.input.is_receiver && entry.is_explicit);
                if !has_explicit_partial_args {
                    return CodeExpr::Ident(self.binding_name.clone());
                }

                let placeholder = "pattern";
                let args = render_call_args(resolved.iter().map(|entry| {
                    if entry.input.is_receiver {
                        Some(CodeExpr::Ident(placeholder.into()))
                    } else {
                        entry.expr.clone()
                    }
                }));
                return CodeExpr::Raw(format!("{placeholder} => {}({args})", self.binding_name));
            }
        }

        CodeExpr::Call {
            func: self.binding_name.clone(),
            args: rendered_call_args(
                resolved
                    .into_iter()
                    .map(|entry| entry.expr)
                    .collect::<Vec<Option<CodeExpr>>>(),
            ),
        }
    }
}

#[derive(Clone)]
struct ResolvedGeneratedInput<'a> {
    input: &'a CadenceTrickInput,
    expr: Option<CodeExpr>,
    is_explicit: bool,
}

fn resolve_generated_input<'a>(
    input: &'a CadenceTrickInput,
    inputs: &PieceInputs,
    inline_params: &BTreeMap<String, Value>,
) -> ResolvedGeneratedInput<'a> {
    let param_id = format!("arg{}", input.slot);
    let connected = inputs.get(param_id.as_str()).cloned();
    let inline = inline_params.get(param_id.as_str()).and_then(|value| {
        schema_for_port_type(
            &input.port_type,
            None,
            can_inline_for_port(&input.port_type),
        )
        .inline_expr(value)
    });
    ResolvedGeneratedInput {
        input,
        expr: connected.or(inline),
        is_explicit: inputs.get(param_id.as_str()).is_some()
            || inline_params.contains_key(param_id.as_str()),
    }
}

fn rendered_call_args(args: Vec<Option<CodeExpr>>) -> Vec<CodeExpr> {
    let mut args = args;
    while matches!(args.last(), Some(None)) {
        args.pop();
    }
    args.into_iter()
        .map(|value| value.unwrap_or_else(|| CodeExpr::Ident("undefined".into())))
        .collect()
}

fn render_call_args(args: impl IntoIterator<Item = Option<CodeExpr>>) -> String {
    rendered_call_args(args.into_iter().collect())
        .into_iter()
        .map(|arg| arg.render())
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_generated_params(inputs: &[CadenceTrickInput]) -> Vec<ParamDef> {
    let mut ordered = inputs.to_vec();
    ordered.sort_by_key(|input| (if input.is_receiver { 0 } else { 1 }, input.slot));
    let mut extra_sides = vec![TileSide::BOTTOM, TileSide::TOP, TileSide::RIGHT];
    let mut params = Vec::with_capacity(ordered.len());

    for (index, input) in ordered.iter().enumerate() {
        let side = if input.is_receiver {
            TileSide::LEFT
        } else if !ordered.iter().any(|value| value.is_receiver) && index == 0 {
            TileSide::LEFT
        } else {
            extra_sides.remove(0.min(extra_sides.len().saturating_sub(1)))
        };
        params.push(ParamDef {
            id: format!("arg{}", input.slot),
            label: input.label.clone(),
            side,
            schema: schema_for_port_type(
                &input.port_type,
                None,
                can_inline_for_port(&input.port_type),
            ),
            text_semantics: Default::default(),
            variadic_group: None,
            required: input.required && !input.is_receiver,
        });
    }

    params
}

fn can_inline_for_port(port_type: &PortType) -> bool {
    !matches!(port_type.as_str(), "pattern" | "trigger" | "signal")
}

pub(crate) fn default_expr_for_input(input: &CadenceTrickInput) -> Option<CodeExpr> {
    schema_for_port_type(
        &input.port_type,
        input.default_value.clone(),
        can_inline_for_port(&input.port_type),
    )
    .default_expr()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Number, Value};

    use super::GeneratedTrickPiece;
    use crate::core::pieces::transforms::ApplyPiece;
    use crate::model::CadenceTrickInput;
    use tessera::code_expr::CodeExpr;
    use tessera::piece::{Piece, PieceInputs};
    use tessera::types::{GridPos, PortType};

    fn trick_input(
        slot: u8,
        port_type: &str,
        required: bool,
        is_receiver: bool,
    ) -> CadenceTrickInput {
        CadenceTrickInput {
            slot,
            pos: GridPos {
                col: i32::from(slot),
                row: 0,
            },
            label: format!("arg{slot}"),
            port_type: PortType::from(port_type),
            required,
            is_receiver,
            default_value: None,
        }
    }

    fn sound_expr() -> CodeExpr {
        CodeExpr::Call {
            func: "s".into(),
            args: vec![CodeExpr::Literal(Value::String("bd".into()))],
        }
    }

    #[test]
    fn generated_transform_trick_outputs_any_and_keeps_receiver_optional() {
        let piece = GeneratedTrickPiece::new(
            "ritmo",
            "ritmo",
            "ritmo",
            &[trick_input(1, "pattern", true, true)],
        );

        assert_eq!(piece.def().output_type, Some(PortType::any()));
        let receiver = piece
            .def()
            .params
            .iter()
            .find(|param| param.id == "arg1")
            .expect("receiver param");
        assert!(!receiver.required);
    }

    #[test]
    fn generated_transform_trick_without_receiver_compiles_to_identifier() {
        let piece = GeneratedTrickPiece::new(
            "ritmo",
            "ritmo",
            "ritmo",
            &[trick_input(1, "pattern", true, true)],
        );

        let expr = piece.compile(&PieceInputs::default(), &BTreeMap::new());
        assert_eq!(expr.render(), "ritmo");
    }

    #[test]
    fn generated_transform_trick_with_receiver_compiles_to_invocation() {
        let piece = GeneratedTrickPiece::new(
            "ritmo",
            "ritmo",
            "ritmo",
            &[trick_input(1, "pattern", true, true)],
        );
        let mut inputs = PieceInputs::default();
        inputs.scalar.insert("arg1".into(), sound_expr());

        let expr = piece.compile(&inputs, &BTreeMap::new());
        assert_eq!(expr.render(), "ritmo(s('bd'))");
    }

    #[test]
    fn generated_transform_trick_can_feed_apply_as_reference() {
        let trick_piece = GeneratedTrickPiece::new(
            "ritmo",
            "ritmo",
            "ritmo",
            &[trick_input(1, "pattern", true, true)],
        );
        let trick_ref = trick_piece.compile(&PieceInputs::default(), &BTreeMap::new());
        let apply = ApplyPiece::new();
        let mut inputs = PieceInputs::default();
        inputs.scalar.insert("pattern".into(), sound_expr());
        inputs.scalar.insert("fn_name".into(), trick_ref);

        let expr = apply.compile(&inputs, &BTreeMap::new());
        assert_eq!(expr.render(), "s('bd').apply(ritmo)");
    }

    #[test]
    fn zero_input_trick_compiles_to_identifier() {
        let piece = GeneratedTrickPiece::new("scala", "scala", "scala", &[]);
        let expr = piece.compile(&PieceInputs::default(), &BTreeMap::new());
        assert_eq!(expr.render(), "scala");
    }

    #[test]
    fn zero_input_trick_has_no_params() {
        let piece = GeneratedTrickPiece::new("scala", "scala", "scala", &[]);
        assert!(piece.def().params.is_empty());
        assert_eq!(piece.def().output_type, Some(PortType::any()));
    }

    #[test]
    fn generated_transform_trick_without_receiver_can_partially_apply_other_args() {
        let piece = GeneratedTrickPiece::new(
            "shimmer",
            "shimmer",
            "shimmer",
            &[
                trick_input(1, "pattern", true, true),
                trick_input(2, "number", false, false),
            ],
        );
        let mut inputs = PieceInputs::default();
        inputs.scalar.insert(
            "arg2".into(),
            CodeExpr::Literal(Value::Number(Number::from_f64(0.25).expect("number"))),
        );

        let expr = piece.compile(&inputs, &BTreeMap::new());
        assert_eq!(expr.render(), "(pattern) => shimmer(pattern, 0.25)");
    }
}
