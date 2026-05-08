use std::collections::BTreeMap;

use serde_json::Value;
use tessera::{
    analysis::{AnalyzedGraph, AnalyzedNode, ResolvedInput, ResolvedInputSource},
    diagnostics::{Diagnostic, DiagnosticKind},
    pattern::{AnalyzedPatternSource, PatternEventValue, ResolvedPatternSpan},
    subgraph::{
        SUBGRAPH_INPUT_1_ID, SUBGRAPH_INPUT_2_ID, SUBGRAPH_INPUT_3_ID, SUBGRAPH_OUTPUT_ID,
        SubgraphInput, SubgraphSignature,
    },
    types::Rational,
    types::{GridPos, PortType},
};

use crate::{
    adapter::{
        cadence_core::{preview_events_for_scores, program_output_scores},
        tessera::{
            cadence_domain_bridge, cadence_subgraph_signature,
            host_adapter::{analyze_tricks, subgraph_editor_engine},
        },
    },
    application::authoring::terrance,
    domain::{
        common::GridPos as CadenceGridPos,
        preview::PreviewDocument,
        program::{
            CadenceCallArg, CadenceCallBinding, CadenceCallExpr, CadenceControlKey,
            CadenceControlValueExpr, CadenceFunction, CadenceOutput, CadencePatternExpr,
            CadenceProgram, CadenceValueExpr,
        },
        project::{CadenceGraphTarget, CadenceProjectDocument},
        script::{ScriptInputContext, parse_pitch_text},
    },
};

#[derive(Debug, Clone)]
pub(crate) struct LoweredGraph {
    pub outputs: Vec<CadenceGridPos>,
    pub program: CadenceProgram,
    pub output_scores: Vec<(CadenceGridPos, cadence_core::infrastructure::score::Score)>,
    pub output_debug: Vec<String>,
    pub preview: PreviewDocument,
    pub diagnostics: Vec<Diagnostic>,
    pub sample_selectors: Vec<String>,
}

#[derive(Debug, Clone)]
enum LoweredExpr {
    Pattern(CadencePatternExpr),
    Value(CadenceValueExpr),
    Missing,
}

#[derive(Clone)]
struct TrickSpec<'a> {
    id: String,
    name: String,
    signature: SubgraphSignature,
    analyzed: AnalyzedGraph,
    _graph: &'a tessera::Graph,
}

#[derive(Clone, Copy)]
struct LoweringScope<'a> {
    signature_inputs: Option<&'a [SubgraphInput]>,
}

struct LoweringContext<'a> {
    tricks: Vec<TrickSpec<'a>>,
}

impl<'a> LoweringContext<'a> {
    fn new(project: &'a CadenceProjectDocument) -> Self {
        let analyzed_tricks = analyze_tricks(project)
            .into_iter()
            .map(|trick| (trick.id.clone(), trick))
            .collect::<BTreeMap<_, _>>();
        let editor_engine = subgraph_editor_engine();
        let tricks = project
            .tricks()
            .iter()
            .filter_map(|trick| {
                analyzed_tricks.get(&trick.id).map(|analyzed| TrickSpec {
                    id: trick.id.clone(),
                    name: trick.name.clone(),
                    signature: analyzed.signature.clone(),
                    analyzed: editor_engine.analyze(&trick.graph),
                    _graph: &trick.graph,
                })
            })
            .collect();

        Self { tricks }
    }

    fn lower_graph(
        &self,
        project: &CadenceProjectDocument,
        _target: &CadenceGraphTarget,
        analyzed: &AnalyzedGraph,
    ) -> LoweredGraph {
        let _ = project;
        let mut diagnostics = analyzed.diagnostics.clone();
        let functions = self.lower_functions(&mut diagnostics);
        let scope = LoweringScope {
            signature_inputs: None,
        };

        let outputs = analyzed
            .outputs
            .iter()
            .filter_map(
                |root| match self.lower_output_expr(analyzed, *root, scope) {
                    Ok(expr) => Some(CadenceOutput {
                        root: (*root).into(),
                        expr,
                    }),
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        None
                    }
                },
            )
            .collect::<Vec<_>>();

        let program = CadenceProgram { functions, outputs };
        let rendered_outputs = program_output_scores(&program);
        let preview = PreviewDocument {
            delay_edges: analyzed
                .delay_edges
                .iter()
                .cloned()
                .map(Into::into)
                .collect(),
            domain_bridges: analyzed
                .domain_bridges
                .values()
                .cloned()
                .map(cadence_domain_bridge)
                .collect(),
            output_lanes: rendered_outputs
                .iter()
                .map(|(output_lane, _)| (*output_lane).into())
                .collect(),
            preview_events: preview_events_for_scores(rendered_outputs.as_slice()),
            debug_text: None,
        };

        LoweredGraph {
            outputs: program.outputs.iter().map(|output| output.root).collect(),
            output_debug: program.render_debug(),
            output_scores: rendered_outputs,
            sample_selectors: program.sample_selectors(),
            program,
            preview,
            diagnostics,
        }
    }

    fn lower_functions(&self, diagnostics: &mut Vec<Diagnostic>) -> Vec<CadenceFunction> {
        let mut functions = Vec::new();

        for trick in &self.tricks {
            let scope = LoweringScope {
                signature_inputs: Some(trick.signature.inputs.as_slice()),
            };
            match self.lower_output_expr(&trick.analyzed, trick.signature.output_pos, scope) {
                Ok(body) => functions.push(CadenceFunction {
                    trick_id: trick.id.clone(),
                    trick_name: trick.name.clone(),
                    signature: cadence_subgraph_signature(trick.signature.clone()),
                    body,
                }),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }

        functions
    }

    fn lower_output_expr(
        &self,
        analyzed: &AnalyzedGraph,
        pos: GridPos,
        scope: LoweringScope<'_>,
    ) -> Result<CadencePatternExpr, Diagnostic> {
        let node = analyzed.node(&pos).ok_or_else(|| missing_node(pos))?;

        match node.piece_id.as_str() {
            "cadence.output" => self.lower_pattern_input(node, analyzed, "pattern", scope, pos),
            SUBGRAPH_OUTPUT_ID => self.lower_pattern_input(node, analyzed, "input", scope, pos),
            _ => expect_pattern(self.lower_node_expr(analyzed, pos, scope)?, pos),
        }
    }

    fn lower_node_expr(
        &self,
        analyzed: &AnalyzedGraph,
        pos: GridPos,
        scope: LoweringScope<'_>,
    ) -> Result<LoweredExpr, Diagnostic> {
        let node = analyzed.node(&pos).ok_or_else(|| missing_node(pos))?;

        match node.piece_id.as_str() {
            "connector" | "cross_connector" => self.lower_passthrough(node, analyzed, scope),
            "args_connector" => self.lower_args_connector(node, analyzed, scope, pos),
            "cadence.silence" => Ok(LoweredExpr::Pattern(CadencePatternExpr::Silence)),
            "cadence.container.basic"
            | "cadence.container.subdivide"
            | "cadence.container.alternate"
            | "cadence.container.parallel" => self.lower_container_node(node, analyzed, scope, pos),
            "cadence.control_input" => self.lower_control_input_node(node, analyzed, scope, pos),
            "cadence.overlay" | "cadence.stack" | "cadence.layer" => self
                .lower_branch_pattern_collection(node, analyzed, scope, pos, |branches| {
                    CadencePatternExpr::Merge { branches }
                }),
            "cadence.arrange" | "cadence.cat" => {
                self.lower_branch_pattern_collection(node, analyzed, scope, pos, |branches| {
                    CadencePatternExpr::CycleRoute { branches }
                })
            }
            "cadence.polymeter" => {
                self.lower_branch_pattern_collection(node, analyzed, scope, pos, |branches| {
                    CadencePatternExpr::CycleSlots { branches }
                })
            }
            "cadence.sound" => self.lower_source_node(node, analyzed, scope, pos),
            "cadence.note" => self.lower_note_node(
                node,
                analyzed,
                scope,
                pos,
                terrance::PitchResolveMode::Absolute,
            ),
            "cadence.n" => self.lower_note_node(
                node,
                analyzed,
                scope,
                pos,
                terrance::PitchResolveMode::Degree,
            ),
            "cadence.add" => self.lower_add_node(node, analyzed, scope, pos),
            "cadence.scale" => self.lower_scale_node(node, analyzed, scope, pos),
            "cadence.gain" => self.lower_unary_pattern_transform(
                node,
                analyzed,
                scope,
                pos,
                "amount",
                Some(CadenceControlKey::Gain),
                |input, amount| CadencePatternExpr::Gain {
                    input: Box::new(input),
                    amount,
                },
            ),
            "cadence.speed" | "cadence.fast" => self.lower_unary_pattern_transform(
                node,
                analyzed,
                scope,
                pos,
                "factor",
                None,
                |input, factor| CadencePatternExpr::Fast {
                    input: Box::new(input),
                    factor,
                },
            ),
            "cadence.stretch" | "cadence.slow" => self.lower_unary_pattern_transform(
                node,
                analyzed,
                scope,
                pos,
                "factor",
                None,
                |input, factor| CadencePatternExpr::Slow {
                    input: Box::new(input),
                    factor,
                },
            ),
            "cadence.mirror" | "cadence.rev" => {
                self.lower_pattern_only_transform(node, analyzed, scope, pos, |input| {
                    CadencePatternExpr::ReflectCycle {
                        input: Box::new(input),
                    }
                })
            }
            "cadence.jux_by" => self.lower_jux_by_node(node, analyzed, scope, pos),
            "cadence.mask" => self.lower_mask_node(node, analyzed, scope, pos),
            "cadence.sample_bank" => self.lower_control_choice(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::SampleBank,
                "value",
                CadenceValueExpr::text("default"),
            ),
            "cadence.clip" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::ClipLength,
                "value",
                CadenceValueExpr::number(1.0),
            ),
            "cadence.pan" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Pan,
                "value",
                CadenceValueExpr::number(0.0),
            ),
            "cadence.playback_rate" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::PlaybackRate,
                "value",
                CadenceValueExpr::number(1.0),
            ),
            "cadence.playback_start" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::PlaybackStart,
                "value",
                CadenceValueExpr::number(0.0),
            ),
            "cadence.playback_end" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::PlaybackEnd,
                "value",
                CadenceValueExpr::number(1.0),
            ),
            "cadence.reverse" => self.lower_control_transform(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Reverse,
                CadenceControlValueExpr::Bool { value: true },
            ),
            "cadence.velocity" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Velocity,
                "value",
                CadenceValueExpr::number(1.0),
            ),
            "cadence.legato" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Legato,
                "value",
                CadenceValueExpr::number(1.0),
            ),
            "cadence.attack" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Attack,
                "value",
                CadenceValueExpr::number(0.01),
            ),
            "cadence.decay" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Decay,
                "value",
                CadenceValueExpr::number(0.05),
            ),
            "cadence.sustain" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Sustain,
                "value",
                CadenceValueExpr::number(0.8),
            ),
            "cadence.release" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::Release,
                "value",
                CadenceValueExpr::number(0.2),
            ),
            "cadence.lowpass_cutoff" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::LowPassCutoff,
                "value",
                CadenceValueExpr::number(1200.0),
            ),
            "cadence.lowpass_resonance" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::LowPassResonance,
                "value",
                CadenceValueExpr::number(0.0),
            ),
            "cadence.highpass_cutoff" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::HighPassCutoff,
                "value",
                CadenceValueExpr::number(180.0),
            ),
            "cadence.highpass_resonance" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::HighPassResonance,
                "value",
                CadenceValueExpr::number(0.0),
            ),
            "cadence.reverb_send" => self.lower_reverb_node(node, analyzed, scope, pos),
            "cadence.delay" => self.lower_delay_node(node, analyzed, scope, pos),
            "cadence.compressor" => self.lower_compressor_node(node, analyzed, scope, pos),
            "cadence.postgain" => self.lower_control_scalar(
                node,
                analyzed,
                scope,
                pos,
                CadenceControlKey::PostGain,
                "value",
                CadenceValueExpr::number(1.0),
            ),
            SUBGRAPH_INPUT_1_ID | SUBGRAPH_INPUT_2_ID | SUBGRAPH_INPUT_3_ID => {
                self.lower_subgraph_input(node, scope, pos)
            }
            piece_id if extract_trick_id(piece_id).is_some() => {
                let trick_id = extract_trick_id(piece_id).expect("checked trick prefix");
                self.lower_trick_call(trick_id, node, analyzed, scope, pos)
            }
            other => Err(Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: format!(
                        "Cadence Pass 1 does not lower piece `{other}` yet; keep the graph within the foundational slice"
                    ),
                },
                Some(pos),
            )),
        }
    }

    fn lower_source_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let source_pattern = node
            .pattern_source
            .as_ref()
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "`cadence.sound` requires a structured Tessera surface".into(),
                    },
                    Some(pos),
                )
            })
            .and_then(|surface| lower_structured_source_surface(surface, Some(pos.into())))?;

        let pattern = match self.lower_pattern_port_expr(
            node,
            analyzed,
            "pattern",
            scope,
            pos,
            ScriptInputContext::Pattern,
        )? {
            LoweredExpr::Pattern(gate_pattern) => CadencePatternExpr::Mask {
                input: Box::new(source_pattern),
                by: Box::new(gate_pattern),
            },
            LoweredExpr::Missing => source_pattern,
            LoweredExpr::Value(_) => unreachable!("pattern ports only lower to pattern or missing"),
        };

        Ok(LoweredExpr::Pattern(pattern))
    }

    fn lower_note_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
        mode: terrance::PitchResolveMode,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = self.lower_pattern_input(node, analyzed, "pattern", scope, pos)?;
        let pattern = node
            .pattern_source
            .as_ref()
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "`cadence.note` requires a structured Tessera surface".into(),
                    },
                    Some(pos),
                )
            })
            .and_then(|surface| {
                lower_structured_pitch_surface(surface, input, mode, Some(pos.into()))
            })?;

        Ok(LoweredExpr::Pattern(pattern))
    }

    fn lower_container_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let local_pattern = node
            .pattern_source
            .as_ref()
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "container nodes require a structured Tessera surface".into(),
                    },
                    Some(pos),
                )
            })
            .and_then(|surface| lower_analyzed_pattern_source(surface, Some(pos.into())))?;

        let input = match self.lower_pattern_port_expr(
            node,
            analyzed,
            "pattern",
            scope,
            pos,
            ScriptInputContext::Pattern,
        )? {
            LoweredExpr::Pattern(pattern) => pattern,
            LoweredExpr::Missing => CadencePatternExpr::Silence,
            LoweredExpr::Value(_) => {
                return Err(Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "`pattern` expects a Tessera container pattern input".into(),
                    },
                    Some(pos),
                ));
            }
        };

        let pattern = match input {
            CadencePatternExpr::Silence => local_pattern,
            input => CadencePatternExpr::CycleRoute {
                branches: vec![input, local_pattern],
            },
        };

        Ok(LoweredExpr::Pattern(pattern))
    }

    fn lower_control_input_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let widget = node
            .inline_params
            .get("widget")
            .and_then(Value::as_str)
            .unwrap_or("dial");
        let value = match self.lower_input_expr(node, analyzed, "value", scope)? {
            LoweredExpr::Value(CadenceValueExpr::Text { value }) => value,
            LoweredExpr::Value(CadenceValueExpr::Number { value }) => value.to_string(),
            LoweredExpr::Value(CadenceValueExpr::Bool { value }) => value.to_string(),
            LoweredExpr::Missing => String::new(),
            LoweredExpr::Pattern(_) => {
                return Err(Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "control input `value` expects a scalar; \
                                 pattern tiles cannot directly drive a control input's value — \
                                 use a control_input tile with a typed value instead"
                            .into(),
                    },
                    Some(pos),
                ));
            }
            LoweredExpr::Value(_) => String::new(),
        };
        let default = match self.lower_input_expr(node, analyzed, "default", scope)? {
            LoweredExpr::Value(CadenceValueExpr::Text { value }) => value,
            LoweredExpr::Value(CadenceValueExpr::Number { value }) => value.to_string(),
            LoweredExpr::Value(CadenceValueExpr::Bool { value }) => value.to_string(),
            LoweredExpr::Missing => String::new(),
            LoweredExpr::Pattern(_) => {
                return Err(Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "control input `default` expects a scalar; \
                                 pattern tiles cannot directly drive a control input's default"
                            .into(),
                    },
                    Some(pos),
                ));
            }
            LoweredExpr::Value(_) => String::new(),
        };
        let resolved_value = if value.trim().is_empty() {
            default
        } else {
            value
        };

        let lowered = match widget {
            "toggle" => CadenceValueExpr::bool(matches!(
                resolved_value.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "on" | "yes"
            )),
            "select" | "sample_select" | "bank_select" | "text" => {
                CadenceValueExpr::text(if widget == "select" && resolved_value.trim().is_empty() {
                    node.inline_params
                        .get("options")
                        .and_then(Value::as_str)
                        .and_then(|options| {
                            options.lines().map(str::trim).find(|line| !line.is_empty())
                        })
                        .unwrap_or_default()
                        .to_string()
                } else {
                    resolved_value
                })
            }
            _ => CadenceValueExpr::number(
                resolved_value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|parsed| parsed.is_finite())
                    .unwrap_or(0.0),
            ),
        };

        Ok(LoweredExpr::Value(lowered))
    }

    fn lower_add_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = self.lower_pattern_input(node, analyzed, "pattern", scope, pos)?;
        let pattern = node
            .pattern_source
            .as_ref()
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "`cadence.add` requires a structured Tessera surface".into(),
                    },
                    Some(pos),
                )
            })
            .and_then(|surface| {
                lower_structured_pitch_surface(
                    surface,
                    input,
                    terrance::PitchResolveMode::Offset,
                    Some(pos.into()),
                )
            })?;

        Ok(LoweredExpr::Pattern(pattern))
    }

    fn lower_scale_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = self.lower_pattern_input(node, analyzed, "pattern", scope, pos)?;
        let pattern = node
            .pattern_source
            .as_ref()
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "`cadence.scale` requires a structured Tessera surface".into(),
                    },
                    Some(pos),
                )
            })
            .map(|surface| {
                let spec = scale_spec_from_surface(surface, Some(pos.into()))?;
                Ok::<CadencePatternExpr, Diagnostic>(terrance::apply_scale_to_pattern_expr(
                    input, spec,
                ))
            })??;

        Ok(LoweredExpr::Pattern(pattern))
    }

    fn lower_jux_by_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = self.lower_pattern_input(node, analyzed, "pattern", scope, pos)?;
        let amount = match self.lower_input_expr(node, analyzed, "amount", scope)? {
            LoweredExpr::Value(value) => value,
            LoweredExpr::Missing => CadenceValueExpr::number(1.0),
            LoweredExpr::Pattern(_) => {
                return Err(Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: "`amount` expects a scalar input".into(),
                    },
                    Some(pos),
                ));
            }
        };

        let mirrored = CadencePatternExpr::ReflectCycle {
            input: Box::new(input.clone()),
        };
        let left = CadencePatternExpr::Control {
            input: Box::new(input),
            key: CadenceControlKey::Pan,
            value: CadenceControlValueExpr::Scalar {
                value: negate_value(amount.clone()),
            },
        };
        let right = CadencePatternExpr::Control {
            input: Box::new(mirrored),
            key: CadenceControlKey::Pan,
            value: CadenceControlValueExpr::Scalar { value: amount },
        };

        Ok(LoweredExpr::Pattern(CadencePatternExpr::Merge {
            branches: vec![left, right],
        }))
    }

    fn lower_unary_pattern_transform(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
        scalar_param: &str,
        control_key: Option<CadenceControlKey>,
        build: impl FnOnce(CadencePatternExpr, CadenceValueExpr) -> CadencePatternExpr,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = expect_pattern(
            self.lower_input_expr(node, analyzed, "pattern", scope)?,
            pos,
        )?;
        let scalar = match self.lower_input_expr(node, analyzed, scalar_param, scope)? {
            LoweredExpr::Value(CadenceValueExpr::Text { value }) if control_key.is_some() => {
                match lower_scalar_control_text(
                    value.as_str(),
                    control_key.expect("checked above"),
                    pos,
                )? {
                    Some(CadenceControlValueExpr::Scalar { value }) => value,
                    Some(CadenceControlValueExpr::Pattern { expr }) => {
                        return Ok(LoweredExpr::Pattern(CadencePatternExpr::Control {
                            input: Box::new(input),
                            key: control_key.expect("checked above"),
                            value: CadenceControlValueExpr::Pattern { expr },
                        }));
                    }
                    None => {
                        return Err(Diagnostic::error(
                            DiagnosticKind::InvalidOperation {
                                reason: format!("missing required scalar input `{scalar_param}`"),
                            },
                            Some(pos),
                        ));
                    }
                    Some(_) => {
                        return Err(Diagnostic::error(
                            DiagnosticKind::InvalidOperation {
                                reason: format!(
                                    "`{scalar_param}` expects a scalar or value-pattern input"
                                ),
                            },
                            Some(pos),
                        ));
                    }
                }
            }
            LoweredExpr::Value(value) => value,
            LoweredExpr::Missing => {
                return Err(Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: format!("missing required scalar input `{scalar_param}`"),
                    },
                    Some(pos),
                ));
            }
            LoweredExpr::Pattern(value_pattern) => {
                if let Some(key) = control_key {
                    return Ok(LoweredExpr::Pattern(CadencePatternExpr::Control {
                        input: Box::new(input),
                        key,
                        value: CadenceControlValueExpr::Pattern {
                            expr: Box::new(value_pattern),
                        },
                    }));
                }
                return Err(Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: format!(
                            "`{scalar_param}` expects a scalar value, not a pattern stream;                              use a control_input tile for a fixed value"
                        ),
                    },
                    Some(pos),
                ));
            }
        };

        Ok(LoweredExpr::Pattern(build(input, scalar)))
    }

    fn lower_pattern_only_transform(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
        build: impl FnOnce(CadencePatternExpr) -> CadencePatternExpr,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = self.lower_pattern_input(node, analyzed, "pattern", scope, pos)?;
        Ok(LoweredExpr::Pattern(build(input)))
    }

    fn lower_branch_pattern_collection(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
        build: impl FnOnce(Vec<CadencePatternExpr>) -> CadencePatternExpr,
    ) -> Result<LoweredExpr, Diagnostic> {
        let mut branches = Vec::new();
        for param in ["in_w", "in_n", "in_s", "in_e"] {
            match self.lower_pattern_port_expr(
                node,
                analyzed,
                param,
                scope,
                pos,
                ScriptInputContext::Pattern,
            )? {
                LoweredExpr::Pattern(branch) => branches.push(branch),
                LoweredExpr::Missing => {}
                LoweredExpr::Value(_) => {
                    unreachable!("pattern ports only lower to pattern or missing")
                }
            }
        }

        if branches.is_empty() {
            Ok(LoweredExpr::Pattern(CadencePatternExpr::Silence))
        } else {
            Ok(LoweredExpr::Pattern(build(branches)))
        }
    }

    fn lower_mask_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = self.lower_pattern_input(node, analyzed, "pattern", scope, pos)?;
        let pattern = match self.lower_pattern_port_expr(
            node,
            analyzed,
            "by",
            scope,
            pos,
            ScriptInputContext::StructuralPattern,
        )? {
            LoweredExpr::Pattern(by) => CadencePatternExpr::Mask {
                input: Box::new(input),
                by: Box::new(by),
            },
            LoweredExpr::Missing => input,
            LoweredExpr::Value(_) => unreachable!("pattern ports only lower to pattern or missing"),
        };
        Ok(LoweredExpr::Pattern(pattern))
    }

    fn lower_control_transform(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
        key: CadenceControlKey,
        value: CadenceControlValueExpr,
    ) -> Result<LoweredExpr, Diagnostic> {
        let input = self.lower_pattern_input(node, analyzed, "pattern", scope, pos)?;
        Ok(LoweredExpr::Pattern(CadencePatternExpr::Control {
            input: Box::new(input),
            key,
            value,
        }))
    }

    fn lower_control_scalar(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
        key: CadenceControlKey,
        param_id: &str,
        default: CadenceValueExpr,
    ) -> Result<LoweredExpr, Diagnostic> {
        let control_value = match self.lower_input_expr(node, analyzed, param_id, scope)? {
            LoweredExpr::Value(value) => match value {
                CadenceValueExpr::Text { value } => {
                    lower_scalar_control_text(value.as_str(), key, pos)?.unwrap_or(
                        CadenceControlValueExpr::Scalar {
                            value: CadenceValueExpr::text(value),
                        },
                    )
                }
                other => CadenceControlValueExpr::Scalar { value: other },
            },
            LoweredExpr::Missing => CadenceControlValueExpr::Scalar { value: default },
            LoweredExpr::Pattern(value_pattern) => CadenceControlValueExpr::Pattern {
                expr: Box::new(value_pattern),
            },
        };
        self.lower_control_transform(node, analyzed, scope, pos, key, control_value)
    }

    fn lower_control_choice(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
        key: CadenceControlKey,
        param_id: &str,
        default: CadenceValueExpr,
    ) -> Result<LoweredExpr, Diagnostic> {
        let value = self
            .lower_optional_value_input(node, analyzed, param_id, scope, pos)?
            .unwrap_or(default);
        self.lower_control_transform(
            node,
            analyzed,
            scope,
            pos,
            key,
            CadenceControlValueExpr::Choice { value },
        )
    }

    fn lower_reverb_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let bundle = self.lower_optional_bundle_input(node, analyzed, "config", scope, pos)?;
        let raw_amount = bundle_item_or_default(bundle.as_ref(), 0, CadenceValueExpr::number(0.2));
        let raw_decay = bundle_item_or_default(bundle.as_ref(), 1, CadenceValueExpr::number(2.0));
        let raw_damping = bundle_item_or_default(bundle.as_ref(), 2, CadenceValueExpr::number(0.3));
        let (amount, decay, damping) = unpack_reverb_values(raw_amount, raw_decay, raw_damping)
            .map_err(|reason| {
                Diagnostic::error(DiagnosticKind::InvalidOperation { reason }, Some(pos))
            })?;

        self.lower_control_transform(
            node,
            analyzed,
            scope,
            pos,
            CadenceControlKey::ReverbSend,
            CadenceControlValueExpr::Reverb {
                amount,
                decay,
                damping,
            },
        )
    }

    fn lower_delay_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let bundle = self.lower_optional_bundle_input(node, analyzed, "config", scope, pos)?;
        let raw_amount = bundle_item_or_default(bundle.as_ref(), 0, CadenceValueExpr::number(0.35));
        let raw_time = bundle_item_or_default(bundle.as_ref(), 1, CadenceValueExpr::number(0.36));
        let raw_feedback =
            bundle_item_or_default(bundle.as_ref(), 2, CadenceValueExpr::number(0.3));
        let raw_damping = bundle_item_or_default(bundle.as_ref(), 3, CadenceValueExpr::number(0.2));
        let (amount, time, feedback, damping) =
            unpack_delay_values(raw_amount, raw_time, raw_feedback, raw_damping).map_err(
                |reason| Diagnostic::error(DiagnosticKind::InvalidOperation { reason }, Some(pos)),
            )?;

        self.lower_control_transform(
            node,
            analyzed,
            scope,
            pos,
            CadenceControlKey::DelaySend,
            CadenceControlValueExpr::Delay {
                amount,
                time,
                feedback,
                damping,
            },
        )
    }

    fn lower_compressor_node(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let bundle = self.lower_optional_bundle_input(node, analyzed, "config", scope, pos)?;
        let threshold = bundle_item_or_default(bundle.as_ref(), 0, CadenceValueExpr::number(0.35));
        let ratio = bundle_item_or_default(bundle.as_ref(), 1, CadenceValueExpr::number(4.0));
        let attack = bundle_item_or_default(bundle.as_ref(), 2, CadenceValueExpr::number(0.005));
        let release = bundle_item_or_default(bundle.as_ref(), 3, CadenceValueExpr::number(0.04));

        self.lower_control_transform(
            node,
            analyzed,
            scope,
            pos,
            CadenceControlKey::Compressor,
            CadenceControlValueExpr::Compressor {
                threshold,
                ratio,
                attack,
                release,
            },
        )
    }

    fn lower_args_connector(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let mut items = Vec::new();
        for param_id in ["arg1", "arg2", "arg3", "arg4"] {
            match self.lower_input_expr(node, analyzed, param_id, scope)? {
                LoweredExpr::Value(value) => items.push(Some(value)),
                LoweredExpr::Missing => items.push(None),
                LoweredExpr::Pattern(_) => {
                    return Err(Diagnostic::error(
                        DiagnosticKind::InvalidOperation {
                            reason: format!("`{param_id}` expects a scalar control input"),
                        },
                        Some(pos),
                    ));
                }
            }
        }
        while matches!(items.last(), Some(None)) {
            items.pop();
        }
        if items.is_empty() {
            Ok(LoweredExpr::Missing)
        } else {
            Ok(LoweredExpr::Value(CadenceValueExpr::bundle(items)))
        }
    }

    fn lower_pattern_input(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        param_id: &str,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<CadencePatternExpr, Diagnostic> {
        match self.lower_pattern_port_expr(
            node,
            analyzed,
            param_id,
            scope,
            pos,
            ScriptInputContext::Pattern,
        )? {
            LoweredExpr::Pattern(pattern) => Ok(pattern),
            LoweredExpr::Missing => Ok(CadencePatternExpr::Silence),
            LoweredExpr::Value(_) => unreachable!("pattern ports only lower to pattern or missing"),
        }
    }

    fn lower_pattern_port_expr(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        param_id: &str,
        scope: LoweringScope<'_>,
        pos: GridPos,
        context: ScriptInputContext,
    ) -> Result<LoweredExpr, Diagnostic> {
        match self.lower_input_expr(node, analyzed, param_id, scope)? {
            LoweredExpr::Pattern(pattern) => Ok(LoweredExpr::Pattern(pattern)),
            LoweredExpr::Missing => Ok(LoweredExpr::Missing),
            LoweredExpr::Value(_) => Err(Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: format!(
                        "`{param_id}` is a {context:?} port and must be driven by a structured pattern surface; inline text pattern parsing has been removed"
                    ),
                },
                Some(pos),
            )),
        }
    }

    fn lower_optional_value_input(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        param_id: &str,
        scope: LoweringScope<'_>,
        _pos: GridPos,
    ) -> Result<Option<CadenceValueExpr>, Diagnostic> {
        match self.lower_input_expr(node, analyzed, param_id, scope)? {
            LoweredExpr::Value(value) => Ok(Some(value)),
            LoweredExpr::Missing => Ok(None),
            // Keep runtime playback resilient if a pattern stream is routed into
            // a scalar control input; the control falls back to its default.
            LoweredExpr::Pattern(_) => Ok(None),
        }
    }

    fn lower_optional_bundle_input(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        param_id: &str,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<Option<Vec<Option<CadenceValueExpr>>>, Diagnostic> {
        match self.lower_input_expr(node, analyzed, param_id, scope)? {
            LoweredExpr::Value(CadenceValueExpr::Bundle { items }) => Ok(Some(items)),
            LoweredExpr::Missing => Ok(None),
            LoweredExpr::Value(_) | LoweredExpr::Pattern(_) => Err(Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: format!("`{param_id}` expects an ordered control bundle"),
                },
                Some(pos),
            )),
        }
    }

    fn lower_subgraph_input(
        &self,
        node: &AnalyzedNode,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let slot = slot_for_subgraph_input(node.piece_id.as_str()).ok_or_else(|| {
            Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: "unknown subgraph input slot".to_string(),
                },
                Some(pos),
            )
        })?;
        let input = scope
            .signature_inputs
            .and_then(|inputs| inputs.iter().find(|input| input.slot == slot))
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: format!(
                            "missing signature metadata for subgraph input slot {slot}"
                        ),
                    },
                    Some(pos),
                )
            })?;

        if is_pattern_port(&input.port_type) {
            Ok(LoweredExpr::Pattern(CadencePatternExpr::Argument {
                slot,
                label: input.label.clone(),
            }))
        } else {
            Ok(LoweredExpr::Value(CadenceValueExpr::Argument {
                slot,
                label: input.label.clone(),
            }))
        }
    }

    fn lower_trick_call(
        &self,
        trick_id: &str,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
        pos: GridPos,
    ) -> Result<LoweredExpr, Diagnostic> {
        let trick = self
            .tricks
            .iter()
            .find(|trick| trick.id == trick_id)
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: format!("unknown trick `{trick_id}`"),
                    },
                    Some(pos),
                )
            })?;

        let mut arguments = Vec::new();
        for input in &trick.signature.inputs {
            let param_id = format!("arg{}", input.slot);
            let arg = match self.lower_input_expr(node, analyzed, param_id.as_str(), scope)? {
                LoweredExpr::Pattern(pattern) => CadenceCallArg::Pattern { pattern },
                LoweredExpr::Value(value) => CadenceCallArg::Value { value },
                LoweredExpr::Missing => default_call_arg(input, pos)?,
            };
            arguments.push(CadenceCallBinding {
                param: input.label.clone(),
                arg,
            });
        }

        Ok(LoweredExpr::Pattern(CadencePatternExpr::Call {
            call: CadenceCallExpr {
                trick_id: trick.id.clone(),
                trick_name: trick.name.clone(),
                arguments,
            },
        }))
    }

    fn lower_passthrough(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        scope: LoweringScope<'_>,
    ) -> Result<LoweredExpr, Diagnostic> {
        for param in ["target", "h_in", "v_in"] {
            if let Some(resolved) = node.input(param)
                && !resolved.is_missing()
            {
                return self.lower_resolved_input(analyzed, resolved, scope);
            }
        }
        Ok(LoweredExpr::Missing)
    }

    fn lower_input_expr(
        &self,
        node: &AnalyzedNode,
        analyzed: &AnalyzedGraph,
        param_id: &str,
        scope: LoweringScope<'_>,
    ) -> Result<LoweredExpr, Diagnostic> {
        let Some(resolved) = node.input(param_id) else {
            return Ok(LoweredExpr::Missing);
        };
        self.lower_resolved_input(analyzed, resolved, scope)
    }

    fn lower_resolved_input(
        &self,
        analyzed: &AnalyzedGraph,
        resolved: &ResolvedInput,
        scope: LoweringScope<'_>,
    ) -> Result<LoweredExpr, Diagnostic> {
        match &resolved.source {
            ResolvedInputSource::Edge { from, .. } => self.lower_node_expr(analyzed, *from, scope),
            ResolvedInputSource::Inline { value } | ResolvedInputSource::Default { value } => {
                Ok(value_to_expr(value))
            }
            ResolvedInputSource::Missing => Ok(LoweredExpr::Missing),
        }
    }
}

pub(crate) fn lower_target_graph(
    project: &CadenceProjectDocument,
    target: &CadenceGraphTarget,
    analyzed: &AnalyzedGraph,
) -> LoweredGraph {
    LoweringContext::new(project).lower_graph(project, target, analyzed)
}

fn default_call_arg(input: &SubgraphInput, pos: GridPos) -> Result<CadenceCallArg, Diagnostic> {
    if let Some(default) = input.default_value.as_ref() {
        return Ok(match value_to_expr(default) {
            LoweredExpr::Pattern(pattern) => CadenceCallArg::Pattern { pattern },
            LoweredExpr::Value(value) => CadenceCallArg::Value { value },
            LoweredExpr::Missing => {
                return Err(Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: format!("default value for `{}` is unusable", input.label),
                    },
                    Some(pos),
                ));
            }
        });
    }

    if is_pattern_port(&input.port_type) {
        Ok(CadenceCallArg::Pattern {
            pattern: CadencePatternExpr::Silence,
        })
    } else {
        Err(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: format!("missing required trick argument `{}`", input.label),
            },
            Some(pos),
        ))
    }
}

fn value_to_expr(value: &Value) -> LoweredExpr {
    if let Some(number) = value.as_f64() {
        LoweredExpr::Value(CadenceValueExpr::number(number))
    } else if let Some(boolean) = value.as_bool() {
        LoweredExpr::Value(CadenceValueExpr::bool(boolean))
    } else if let Some(text) = value.as_str() {
        LoweredExpr::Value(CadenceValueExpr::text(text))
    } else {
        LoweredExpr::Missing
    }
}

fn lower_scalar_control_text(
    text: &str,
    key: CadenceControlKey,
    pos: GridPos,
) -> Result<Option<CadenceControlValueExpr>, Diagnostic> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let constant = match key {
        CadenceControlKey::Pitch => parse_pitch_text(trimmed).map(CadenceValueExpr::number),
        _ => trimmed
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(CadenceValueExpr::number),
    };
    if let Some(value) = constant {
        return Ok(Some(CadenceControlValueExpr::Scalar { value }));
    }

    terrance::lower_control_text(trimmed, Some(pos.into()))
        .map(|expr| {
            Some(CadenceControlValueExpr::Pattern {
                expr: Box::new(expr),
            })
        })
        .map_err(|error| {
            Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: format!("invalid control value pattern: {error}"),
                },
                Some(pos),
            )
        })
}

fn unpack_reverb_values(
    amount: CadenceValueExpr,
    decay: CadenceValueExpr,
    damping: CadenceValueExpr,
) -> Result<(CadenceValueExpr, CadenceValueExpr, CadenceValueExpr), String> {
    let CadenceValueExpr::Text { value } = &amount else {
        return Ok((amount, decay, damping));
    };
    let parts = value.split(':').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 2 {
        return Ok((amount, decay, damping));
    }
    let amount = parse_packed_number(parts[0], "room amount")?;
    let decay = parse_packed_number(parts[1], "room decay")?;
    Ok((
        CadenceValueExpr::number(amount),
        CadenceValueExpr::number(decay),
        damping,
    ))
}

fn bundle_item_or_default(
    bundle: Option<&Vec<Option<CadenceValueExpr>>>,
    index: usize,
    default: CadenceValueExpr,
) -> CadenceValueExpr {
    bundle
        .and_then(|items| items.get(index))
        .and_then(|value| value.clone())
        .unwrap_or(default)
}

fn unpack_delay_values(
    amount: CadenceValueExpr,
    time: CadenceValueExpr,
    feedback: CadenceValueExpr,
    damping: CadenceValueExpr,
) -> Result<
    (
        CadenceValueExpr,
        CadenceValueExpr,
        CadenceValueExpr,
        CadenceValueExpr,
    ),
    String,
> {
    let CadenceValueExpr::Text { value } = &amount else {
        return Ok((amount, time, feedback, damping));
    };
    let parts = value.split(':').map(str::trim).collect::<Vec<_>>();
    if !(3..=4).contains(&parts.len()) {
        return Ok((amount, time, feedback, damping));
    }
    let amount = parse_packed_number(parts[0], "delay amount")?;
    let time = parse_packed_number(parts[1], "delay time")?;
    let feedback = parse_packed_number(parts[2], "delay feedback")?;
    let damping = if let Some(damping) = parts.get(3) {
        CadenceValueExpr::number(parse_packed_number(damping, "delay damping")?)
    } else {
        damping
    };
    Ok((
        CadenceValueExpr::number(amount),
        CadenceValueExpr::number(time),
        CadenceValueExpr::number(feedback),
        damping,
    ))
}

fn parse_packed_number(raw: &str, label: &str) -> Result<f64, String> {
    let value = raw
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{label} must be finite"))
    }
}

fn negate_value(value: CadenceValueExpr) -> CadenceValueExpr {
    match value {
        CadenceValueExpr::Number { value } => CadenceValueExpr::number(-value),
        other => other,
    }
}

fn expect_pattern(expr: LoweredExpr, pos: GridPos) -> Result<CadencePatternExpr, Diagnostic> {
    match expr {
        LoweredExpr::Pattern(pattern) => Ok(pattern),
        LoweredExpr::Value(_) => Err(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: "expected a pattern expression".to_string(),
            },
            Some(pos),
        )),
        LoweredExpr::Missing => Ok(CadencePatternExpr::Silence),
    }
}

fn missing_node(pos: GridPos) -> Diagnostic {
    Diagnostic::error(
        DiagnosticKind::InvalidOperation {
            reason: format!("missing analyzed node at ({}, {})", pos.col, pos.row),
        },
        Some(pos),
    )
}

fn scale_surface_diagnostic(
    message: impl Into<String>,
    site: Option<CadenceGridPos>,
) -> Diagnostic {
    Diagnostic::error(
        DiagnosticKind::InvalidOperation {
            reason: format!("invalid scale pattern: {}", message.into()),
        },
        site.map(Into::into),
    )
}

fn lower_structured_source_surface(
    analyzed: &AnalyzedPatternSource,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    if let Some(diagnostic) = analyzed.diagnostics.first() {
        return Err(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: diagnostic.message.clone(),
            },
            site.map(Into::into),
        ));
    }

    let branches = analyzed
        .roots
        .iter()
        .map(|root| lower_structured_source_root(root.spans.as_slice(), site))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(match branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::CycleRoute { branches },
    })
}

fn lower_structured_source_root(
    spans: &[ResolvedPatternSpan],
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    let branches = spans
        .iter()
        .map(|span| lower_structured_source_span(span, site))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|expr| !matches!(expr, CadencePatternExpr::Silence))
        .collect::<Vec<_>>();

    Ok(match branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::Merge { branches },
    })
}

fn lower_structured_source_span(
    span: &ResolvedPatternSpan,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    let event_branches = span
        .events
        .iter()
        .map(|event| lower_structured_source_event(event, site))
        .collect::<Result<Vec<_>, _>>()?;

    let mut expr = match event_branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => event_branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::Merge {
            branches: event_branches,
        },
    };

    if !matches!(expr, CadencePatternExpr::Silence) {
        let duration = rational_to_f64(span.duration) / span.tempo_factor;
        if (duration - 1.0).abs() > f64::EPSILON {
            expr = CadencePatternExpr::Fast {
                input: Box::new(expr),
                factor: CadenceValueExpr::number(1.0 / duration),
            };
        }

        let start = rational_to_f64(span.start);
        let offset = start - start.floor();
        if offset.abs() > f64::EPSILON {
            expr = CadencePatternExpr::Shift {
                input: Box::new(expr),
                offset: CadenceValueExpr::number(offset),
            };
        }
    }

    Ok(expr)
}

fn lower_structured_source_event(
    event: &tessera::ResolvedPatternEvent,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    match &event.value {
        PatternEventValue::Note { value } => {
            let (selector, sample_variant) = split_source_variant(value.as_str());
            let pattern = match crate::domain::program::CadenceSynthSource::parse(&selector) {
                Some(source) if sample_variant.is_none() => CadencePatternExpr::Source {
                    source: crate::domain::program::CadenceSourceKind::Synth,
                    value: CadenceValueExpr::synth_source(source),
                    site,
                },
                _ => CadencePatternExpr::Source {
                    source: crate::domain::program::CadenceSourceKind::Sample,
                    value: CadenceValueExpr::text(selector),
                    site,
                },
            };

            Ok(match sample_variant {
                Some(variant) => CadencePatternExpr::Control {
                    input: Box::new(pattern),
                    key: CadenceControlKey::SampleVariant,
                    value: CadenceControlValueExpr::Scalar {
                        value: CadenceValueExpr::number(variant as f64),
                    },
                },
                None => pattern,
            })
        }
        PatternEventValue::Scalar { value } => Ok(CadencePatternExpr::Source {
            source: crate::domain::program::CadenceSourceKind::Pulse,
            value: CadenceValueExpr::number(*value),
            site,
        }),
    }
}

fn lower_structured_pitch_surface(
    analyzed: &AnalyzedPatternSource,
    input: CadencePatternExpr,
    mode: terrance::PitchResolveMode,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    if let Some(diagnostic) = analyzed.diagnostics.first() {
        return Err(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: diagnostic.message.clone(),
            },
            site.map(Into::into),
        ));
    }

    let branches = analyzed
        .roots
        .iter()
        .map(|root| lower_structured_pitch_root(root, input.clone(), mode, site))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(match branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::CycleRoute { branches },
    })
}

fn lower_structured_pitch_root(
    root: &tessera::pattern::AnalyzedPatternRoot,
    input: CadencePatternExpr,
    mode: terrance::PitchResolveMode,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    let branches = root
        .spans
        .iter()
        .map(|span| lower_structured_pitch_span(span, input.clone(), mode, site))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|expr| !matches!(expr, CadencePatternExpr::Silence))
        .collect::<Vec<_>>();

    Ok(match (root.kind, branches.len()) {
        (_, 0) => CadencePatternExpr::Silence,
        (_, 1) => branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        (tessera::pattern::PatternContainerKind::Subdivide, _) => {
            CadencePatternExpr::CycleSlots { branches }
        }
        (tessera::pattern::PatternContainerKind::Alternate, _) => {
            CadencePatternExpr::CycleRoute { branches }
        }
        _ => CadencePatternExpr::Merge { branches },
    })
}

fn lower_structured_pitch_span(
    span: &ResolvedPatternSpan,
    input: CadencePatternExpr,
    mode: terrance::PitchResolveMode,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    let event_branches = span
        .events
        .iter()
        .map(|event| lower_structured_pitch_event(event, input.clone(), mode, site))
        .collect::<Result<Vec<_>, _>>()?;

    let mut expr = match event_branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => event_branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::Merge {
            branches: event_branches,
        },
    };

    if !matches!(expr, CadencePatternExpr::Silence) {
        let duration = rational_to_f64(span.duration) / span.tempo_factor;
        if (duration - 1.0).abs() > f64::EPSILON {
            expr = CadencePatternExpr::Fast {
                input: Box::new(expr),
                factor: CadenceValueExpr::number(1.0 / duration),
            };
        }

        let start = rational_to_f64(span.start);
        let offset = start - start.floor();
        if offset.abs() > f64::EPSILON {
            expr = CadencePatternExpr::Shift {
                input: Box::new(expr),
                offset: CadenceValueExpr::number(offset),
            };
        }
    }

    Ok(expr)
}

fn lower_structured_pitch_event(
    event: &tessera::ResolvedPatternEvent,
    input: CadencePatternExpr,
    mode: terrance::PitchResolveMode,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    match (&event.value, mode) {
        (PatternEventValue::Note { value }, terrance::PitchResolveMode::Absolute) => {
            let label = if let Some(octave) = event.absolute_octave {
                format!("{value}{octave}")
            } else {
                value.clone()
            };
            let mut expr = CadencePatternExpr::Control {
                input: Box::new(input),
                key: CadenceControlKey::Pitch,
                value: CadenceControlValueExpr::Scalar {
                    value: CadenceValueExpr::text(label),
                },
            };
            if event.octave_shift != 0 {
                expr = CadencePatternExpr::Control {
                    input: Box::new(expr),
                    key: CadenceControlKey::Pitch,
                    value: CadenceControlValueExpr::Scalar {
                        value: CadenceValueExpr::number((event.octave_shift * 12) as f64),
                    },
                };
            }
            Ok(expr)
        }
        (PatternEventValue::Note { value }, terrance::PitchResolveMode::Degree) => {
            let degree = value.trim().parse::<i32>().map_err(|_| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: format!("invalid degree pattern: {}", value),
                    },
                    site.map(Into::into),
                )
            })?;
            Ok(CadencePatternExpr::Control {
                input: Box::new(input),
                key: CadenceControlKey::Pitch,
                value: CadenceControlValueExpr::Scalar {
                    value: CadenceValueExpr::text(degree.to_string()),
                },
            })
        }
        (PatternEventValue::Scalar { value }, terrance::PitchResolveMode::Offset) => {
            terrance::lower_add_text(value.to_string().as_str(), input).map_err(|error| {
                Diagnostic::error(
                    DiagnosticKind::InvalidOperation {
                        reason: format!("invalid add pattern: {error}"),
                    },
                    site.map(Into::into),
                )
            })
        }
        (PatternEventValue::Scalar { value }, terrance::PitchResolveMode::Absolute) => {
            Ok(CadencePatternExpr::Control {
                input: Box::new(input),
                key: CadenceControlKey::Pitch,
                value: CadenceControlValueExpr::Scalar {
                    value: CadenceValueExpr::number(*value),
                },
            })
        }
        _ => Err(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: "structured pattern type does not match this musical node".into(),
            },
            site.map(Into::into),
        )),
    }
}

fn scale_spec_from_surface(
    analyzed: &AnalyzedPatternSource,
    site: Option<CadenceGridPos>,
) -> Result<terrance::ScaleSpec, Diagnostic> {
    if let Some(diagnostic) = analyzed.diagnostics.first() {
        return Err(scale_surface_diagnostic(diagnostic.message.clone(), site));
    }

    let mut atoms = Vec::new();
    for root in &analyzed.roots {
        for span in &root.spans {
            for event in &span.events {
                match &event.value {
                    PatternEventValue::Note { value } => {
                        let note = if let Some(octave) = event.absolute_octave {
                            format!("{value}{octave}")
                        } else {
                            value.clone()
                        };
                        atoms.push(note);
                    }
                    PatternEventValue::Scalar { value } => atoms.push(value.to_string()),
                }
            }
        }
    }

    if atoms.len() != 1 {
        return Err(scale_surface_diagnostic(
            "scale surface must contain exactly one atom like `c4:major`",
            site,
        ));
    }

    terrance::ScaleSpec::parse(atoms[0].as_str())
        .map_err(|error| scale_surface_diagnostic(error.to_string(), site))
}

fn split_source_variant(value: &str) -> (String, Option<usize>) {
    let Some((selector, variant)) = value.rsplit_once(':') else {
        return (value.to_string(), None);
    };
    if selector.trim().is_empty()
        || variant.is_empty()
        || !variant.chars().all(|ch| ch.is_ascii_digit())
    {
        return (value.to_string(), None);
    }
    let Ok(ordinal) = variant.parse::<usize>() else {
        return (value.to_string(), None);
    };
    if ordinal == 0 {
        return (value.to_string(), None);
    }
    (selector.to_string(), Some(ordinal - 1))
}

fn slot_for_subgraph_input(piece_id: &str) -> Option<u8> {
    match piece_id {
        SUBGRAPH_INPUT_1_ID => Some(1),
        SUBGRAPH_INPUT_2_ID => Some(2),
        SUBGRAPH_INPUT_3_ID => Some(3),
        _ => None,
    }
}

fn extract_trick_id(piece_id: &str) -> Option<&str> {
    piece_id.strip_prefix("tessera.subgraph.")
}

fn is_pattern_port(port_type: &PortType) -> bool {
    port_type.as_str() == "pattern"
}

fn lower_analyzed_pattern_source(
    analyzed: &AnalyzedPatternSource,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    if let Some(diagnostic) = analyzed.diagnostics.first() {
        return Err(Diagnostic::error(
            DiagnosticKind::InvalidOperation {
                reason: diagnostic.message.clone(),
            },
            site.map(Into::into),
        ));
    }

    let branches = analyzed
        .roots
        .iter()
        .map(|root| lower_pattern_root(root.spans.as_slice(), site))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(match branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::CycleRoute { branches },
    })
}

fn lower_pattern_root(
    spans: &[ResolvedPatternSpan],
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    let branches = spans
        .iter()
        .map(|span| lower_pattern_span(span, site))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|expr| !matches!(expr, CadencePatternExpr::Silence))
        .collect::<Vec<_>>();

    Ok(match branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::Merge { branches },
    })
}

fn lower_pattern_span(
    span: &ResolvedPatternSpan,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    let event_branches = span
        .events
        .iter()
        .map(|event| lower_pattern_event(event, site))
        .collect::<Result<Vec<_>, _>>()?;

    let mut expr = match event_branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => event_branches
            .into_iter()
            .next()
            .unwrap_or(CadencePatternExpr::Silence),
        _ => CadencePatternExpr::Merge {
            branches: event_branches,
        },
    };

    if !matches!(expr, CadencePatternExpr::Silence) {
        let duration = rational_to_f64(span.duration) / span.tempo_factor;
        if (duration - 1.0).abs() > f64::EPSILON {
            expr = CadencePatternExpr::Fast {
                input: Box::new(expr),
                factor: CadenceValueExpr::number(1.0 / duration),
            };
        }

        let start = rational_to_f64(span.start);
        let offset = start - start.floor();
        if offset.abs() > f64::EPSILON {
            expr = CadencePatternExpr::Shift {
                input: Box::new(expr),
                offset: CadenceValueExpr::number(offset),
            };
        }
    }

    Ok(expr)
}

fn lower_pattern_event(
    event: &tessera::ResolvedPatternEvent,
    site: Option<CadenceGridPos>,
) -> Result<CadencePatternExpr, Diagnostic> {
    let mut expr = match &event.value {
        PatternEventValue::Note { value } => {
            let note_value = if let Some(octave) = event.absolute_octave {
                format!("{value}{octave}")
            } else {
                value.clone()
            };
            CadencePatternExpr::Source {
                source: crate::domain::program::CadenceSourceKind::Note,
                value: CadenceValueExpr::text(note_value),
                site,
            }
        }
        PatternEventValue::Scalar { value } => CadencePatternExpr::Source {
            source: crate::domain::program::CadenceSourceKind::Pulse,
            value: CadenceValueExpr::number(*value),
            site,
        },
    };

    if event.octave_shift != 0 {
        expr = CadencePatternExpr::Control {
            input: Box::new(expr),
            key: CadenceControlKey::Pitch,
            value: CadenceControlValueExpr::Scalar {
                value: CadenceValueExpr::number((event.octave_shift * 12) as f64),
            },
        };
    }

    Ok(expr)
}

fn rational_to_f64(value: Rational) -> f64 {
    value.numerator() as f64 / value.denominator() as f64
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;
    use tessera::{
        graph::{Edge, Graph, Node},
        pattern::{
            PatternAtom, PatternContainer, PatternContainerKind, PatternItem, PatternItemKind,
            PatternPos, PatternRoot, PatternSurface,
        },
        subgraph::{SUBGRAPH_INPUT_1_ID, SUBGRAPH_OUTPUT_ID},
        types::{EdgeId, TileSide},
    };

    use super::*;
    use crate::domain::project::CadenceTrickWorkspace;

    fn sample_project_with_sound_value(value: &str) -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String(value.into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.fast".into(),
                inline_params: BTreeMap::from([("factor".into(), Value::from(2.0))]),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("factor".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project() -> CadenceProjectDocument {
        sample_project_with_sound_value("bd")
    }

    fn sample_project_with_mask_surface() -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.mask".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("by".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 1 },
            Node {
                piece_id: "cadence.container.subdivide".into(),
                inline_params: BTreeMap::new(),
                pattern_source: Some(structured_gate_pattern()),
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: Some(TileSide::TOP),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 1 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "by".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "mask-demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "mask-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project_with_sound_gate_surface() -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::BOTTOM)]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 0, row: 1 },
            Node {
                piece_id: "cadence.container.basic".into(),
                inline_params: BTreeMap::new(),
                pattern_source: Some(PatternSurface { roots: Vec::new() }),
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: Some(TileSide::TOP),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 1 },
                to_node: GridPos { col: 0, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "sound-gate-demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "sound-gate-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project_with_note_script(value: &str) -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.note".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String(value.into()))]),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("value".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "note-demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "note-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project_with_sine_phrase() -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("sine".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.note".into(),
                inline_params: BTreeMap::from([(
                    "value".into(),
                    Value::String("[b4 d5 e5 f#5] [e5 f#5 g5 a5] [f#5 g5 a5 b5]".into()),
                )]),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("value".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.gain".into(),
                inline_params: BTreeMap::from([("amount".into(), Value::from(0.8))]),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("amount".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 3, row: 0 },
            Node {
                piece_id: "cadence.reverb_send".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("config".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 4, row: 0 },
            Node {
                piece_id: "cadence.lowpass_cutoff".into(),
                inline_params: BTreeMap::from([("value".into(), Value::from(800.0))]),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("value".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 5, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 2, row: 0 },
                to_node: GridPos { col: 3, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 3, row: 0 },
                to_node: GridPos { col: 4, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 4, row: 0 },
                to_node: GridPos { col: 5, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "sine-phrase".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "sine-phrase".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn structured_gate_pattern() -> PatternSurface {
        PatternSurface {
            roots: vec![PatternRoot {
                position: PatternPos { col: 0, row: 0 },
                container: PatternContainer {
                    kind: PatternContainerKind::Subdivide,
                    items: vec![
                        PatternItem {
                            position: PatternPos { col: 0, row: 0 },
                            kind: PatternItemKind::Atom(PatternAtom::Scalar { value: 1.0 }),
                        },
                        PatternItem {
                            position: PatternPos { col: 1, row: 0 },
                            kind: PatternItemKind::Atom(PatternAtom::Rest),
                        },
                        PatternItem {
                            position: PatternPos { col: 2, row: 0 },
                            kind: PatternItemKind::Atom(PatternAtom::Scalar { value: 1.0 }),
                        },
                        PatternItem {
                            position: PatternPos { col: 3, row: 0 },
                            kind: PatternItemKind::Atom(PatternAtom::Rest),
                        },
                    ],
                },
            }],
        }
    }

    fn sample_project_with_pattern_input() -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.container.subdivide".into(),
                inline_params: BTreeMap::new(),
                pattern_source: Some(structured_gate_pattern()),
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: Some(TileSide::TOP),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edge = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 0, row: 0 },
            to_node: GridPos { col: 1, row: 0 },
            to_param: "pattern".into(),
        };

        CadenceProjectDocument::new(
            "pattern-input-demo".into(),
            Graph {
                nodes,
                edges: BTreeMap::from([(edge.id.clone(), edge)]),
                name: "pattern-input-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project_with_control_input_note_value(value: &str) -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.note".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("value".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 1 },
            Node {
                piece_id: "cadence.control_input".into(),
                inline_params: BTreeMap::from([
                    ("value".into(), Value::String(value.into())),
                    ("widget".into(), Value::String("text".into())),
                ]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::TOP),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 1 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "value".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "control-input-note-demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "control-input-note-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project_with_control_input_gain_pattern(value: &str) -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.gain".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("amount".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 1 },
            Node {
                piece_id: "cadence.control_input".into(),
                inline_params: BTreeMap::from([
                    ("value".into(), Value::String(value.into())),
                    ("widget".into(), Value::String("text".into())),
                ]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::TOP),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 1 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "amount".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "control-input-gain-pattern-demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "control-input-gain-pattern-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project_with_control_input_gain_value(
        value: Option<&str>,
        default: Option<&str>,
    ) -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.gain".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("amount".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        let mut inline_params = BTreeMap::from([("widget".into(), Value::String("text".into()))]);
        if let Some(value) = value {
            inline_params.insert("value".into(), Value::String(value.into()));
        }
        if let Some(default) = default {
            inline_params.insert("default".into(), Value::String(default.into()));
        }
        nodes.insert(
            GridPos { col: 1, row: 1 },
            Node {
                piece_id: "cadence.control_input".into(),
                inline_params,
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::TOP),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 1 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "amount".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
        ];

        CadenceProjectDocument::new(
            "control-input-gain-value-demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "control-input-gain-value-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn sample_project_with_reverb_bundle_inputs() -> CadenceProjectDocument {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "cadence.sound".into(),
                inline_params: BTreeMap::from([("value".into(), Value::String("bd".into()))]),
                pattern_source: None,
                input_sides: BTreeMap::new(),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "cadence.reverb_send".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("pattern".into(), TileSide::LEFT),
                    ("config".into(), TileSide::BOTTOM),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        for (position, value) in [
            (GridPos { col: 0, row: 1 }, "0.6"),
            (GridPos { col: 1, row: 1 }, "3.5"),
            (GridPos { col: 2, row: 1 }, "0.25"),
        ] {
            nodes.insert(
                position,
                Node {
                    piece_id: "cadence.control_input".into(),
                    inline_params: BTreeMap::from([
                        ("value".into(), Value::String(value.into())),
                        ("widget".into(), Value::String("dial".into())),
                    ]),
                    pattern_source: None,
                    input_sides: BTreeMap::new(),
                    output_side: Some(TileSide::TOP),
                    label: None,
                    node_state: None,
                },
            );
        }
        nodes.insert(
            GridPos { col: 1, row: 2 },
            Node {
                piece_id: "args_connector".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([
                    ("arg1".into(), TileSide::LEFT),
                    ("arg2".into(), TileSide::TOP),
                    ("arg3".into(), TileSide::BOTTOM),
                    ("arg4".into(), TileSide::RIGHT),
                ]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );

        let edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 1 },
                to_node: GridPos { col: 1, row: 2 },
                to_param: "arg1".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 1 },
                to_node: GridPos { col: 1, row: 2 },
                to_param: "arg2".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 2, row: 1 },
                to_node: GridPos { col: 1, row: 2 },
                to_param: "arg3".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 2 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "config".into(),
            },
        ];

        CadenceProjectDocument::new(
            "reverb-bundle-demo".into(),
            Graph {
                nodes,
                edges: edges
                    .into_iter()
                    .map(|edge| (edge.id.clone(), edge))
                    .collect(),
                name: "reverb-bundle-demo".into(),
                cols: 8,
                rows: 8,
            },
        )
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn lowering_builds_program_before_debug_or_runtime_output() {
        let project = sample_project();
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(lowered.program.outputs.len(), 1);
        assert_eq!(lowered.output_debug, vec!["speed(2, sample(\"bd\"))"]);
        assert_eq!(lowered.output_scores.len(), 1);
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert_eq!(
            lowered.preview.output_lanes,
            vec![CadenceGridPos { col: 2, row: 0 }]
        );
        assert_eq!(lowered.preview.preview_events.len(), 2);
    }

    #[test]
    fn lowering_sound_tile_script_builds_structured_pattern_ir() {
        let project = sample_project_with_sound_value("bd hh cp");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec!["speed(2, polymeter(sample(\"bd\"), sample(\"hh\"), sample(\"cp\")))"]
        );
        assert_eq!(
            lowered.sample_selectors,
            vec!["bd".to_string(), "cp".to_string(), "hh".to_string()]
        );
        assert_eq!(lowered.preview.preview_events.len(), 6);
    }

    #[test]
    fn lowering_sound_tile_script_types_builtin_synth_sources_in_cadence_ir() {
        let project = sample_project_with_sound_value("sine bd square");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec!["speed(2, polymeter(synth(sine), sample(\"bd\"), synth(square)))"]
        );
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert_eq!(
            lowered
                .preview
                .preview_events
                .iter()
                .map(|event| event.label.clone())
                .collect::<Vec<_>>(),
            vec![
                "sine".to_string(),
                "bd".to_string(),
                "square".to_string(),
                "sine".to_string(),
                "bd".to_string(),
                "square".to_string(),
            ]
        );
    }

    #[test]
    fn lowering_sound_tile_accepts_quoted_query_selectors() {
        let project = sample_project_with_sound_value(r#""/kick/@2/""#);
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);
        let compiled = crate::application::authoring::compile::compile_project(&project);

        assert_eq!(
            lowered.output_debug,
            vec![r#"speed(2, sample("/kick/@2/"))"#]
        );
        assert_eq!(lowered.sample_selectors, vec!["/kick/@2/".to_string()]);
        assert_eq!(compiled.sample_selectors, vec!["/kick/@2/".to_string()]);
        assert!(compiled.can_play);
    }

    #[test]
    fn lowering_sound_tile_reports_quote_hint_for_bare_query_selectors() {
        let project = sample_project_with_sound_value("/kick/@2/");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert!(lowered.program.outputs.is_empty());
        assert!(
            lowered.diagnostics.iter().any(|diagnostic| matches!(
                &diagnostic.kind,
                DiagnosticKind::InvalidOperation { reason }
                    if reason.contains("Query selectors must be quoted")
                        && reason.contains(r#""/kick/@2/""#)
            )),
            "expected quoted-selector hint, got {:?}",
            lowered.diagnostics
        );
    }

    #[test]
    fn lowering_sound_tile_reports_quote_hint_for_mixed_bare_query_selectors() {
        let project = sample_project_with_sound_value("bd /kick/@2/");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert!(lowered.program.outputs.is_empty());
        assert!(
            lowered.diagnostics.iter().any(|diagnostic| matches!(
                &diagnostic.kind,
                DiagnosticKind::InvalidOperation { reason }
                    if reason.contains("Query selectors must be quoted")
                        && reason.contains(r#""/kick/@2/""#)
            )),
            "expected quoted-selector hint, got {:?}",
            lowered.diagnostics
        );
    }

    #[test]
    fn lowering_mask_structured_surface_preserves_structural_rests() {
        let project = sample_project_with_mask_surface();
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec![
                "mask(sample(\"bd\"), overlay(speed(4, pulse(1)), shift(0.5, speed(4, pulse(1)))))"
            ]
        );
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert_eq!(lowered.preview.preview_events.len(), 1);
        assert_eq!(lowered.preview.preview_events[0].label, "bd");
    }

    #[test]
    fn lowering_sound_structured_surface_can_explicitly_mute_source() {
        let project = sample_project_with_sound_gate_surface();
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(lowered.output_debug, vec!["mask(sample(\"bd\"), silence)"]);
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert!(lowered.preview.preview_events.is_empty());
    }

    #[test]
    fn lowering_note_tile_script_sequences_pitch_preview() {
        let project = sample_project_with_note_script("[e3,e7 e4 _ e7]");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec![
                "polymeter(overlay(note(\"e3\", sample(\"bd\")), note(\"e7\", sample(\"bd\"))), note(\"e4\", sample(\"bd\")), silence, note(\"e7\", sample(\"bd\")))"
            ]
        );
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert_eq!(lowered.preview.preview_events.len(), 4);
        assert_eq!(
            lowered
                .preview
                .preview_events
                .iter()
                .map(|event| event.pitch.clone())
                .collect::<Vec<_>>(),
            vec![
                Some("e3".to_string()),
                Some("e7".to_string()),
                Some("e4".to_string()),
                Some("e7".to_string()),
            ]
        );
        assert!(
            lowered
                .preview
                .preview_events
                .iter()
                .all(|event| event.label == "bd")
        );
        assert_close(lowered.preview.preview_events[0].start_normalized, 0.0);
        assert_close(lowered.preview.preview_events[1].start_normalized, 0.0);
        assert_close(lowered.preview.preview_events[2].start_normalized, 0.25);
        assert_close(lowered.preview.preview_events[3].start_normalized, 0.75);
    }

    #[test]
    fn lowering_note_tile_reports_note_context_for_invalid_cues() {
        let project = sample_project_with_note_script("bd");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert!(lowered.program.outputs.is_empty());
        assert!(
            lowered.diagnostics.iter().any(|diagnostic| matches!(
                &diagnostic.kind,
                DiagnosticKind::InvalidOperation { reason }
                    if reason.contains("note pattern") && reason.contains("bd")
            )),
            "expected note-pattern diagnostic, got {:?}",
            lowered.diagnostics
        );
    }

    #[test]
    fn lowering_container_tile_emits_structured_pattern_source() {
        let project = sample_project_with_pattern_input();
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec!["overlay(speed(4, pulse(1)), shift(0.5, speed(4, pulse(1))))"]
        );
    }

    #[test]
    fn lowering_control_input_tile_can_drive_note_value_ports() {
        let project = sample_project_with_control_input_note_value("e3 e4");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec!["polymeter(note(\"e3\", sample(\"bd\")), note(\"e4\", sample(\"bd\")))"]
        );
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert_eq!(lowered.preview.preview_events.len(), 2);
    }

    #[test]
    fn lowering_control_input_tile_can_drive_numeric_value_patterns() {
        let project = sample_project_with_control_input_gain_pattern("[0.5 0.75 <1 0.75>]");
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec![concat!(
                "gain(pattern(polymeter(",
                "pulse(\"0.5\"), ",
                "pulse(\"0.75\"), ",
                "arrange(pulse(\"1\"), pulse(\"0.75\"))",
                ")), sample(\"bd\"))"
            )]
        );
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert_eq!(
            lowered.preview.output_lanes,
            vec![CadenceGridPos { col: 2, row: 0 }]
        );
    }

    #[test]
    fn lowering_control_input_prefers_authored_value_over_default() {
        let project = sample_project_with_control_input_gain_value(Some("0.8"), Some("0.25"));
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(lowered.output_debug, vec!["gain(0.8, sample(\"bd\"))"]);
    }

    #[test]
    fn lowering_control_input_falls_back_to_default_when_value_is_blank() {
        let project = sample_project_with_control_input_gain_value(Some(""), Some("0.25"));
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(lowered.output_debug, vec!["gain(0.25, sample(\"bd\"))"]);
    }

    #[test]
    fn lowering_args_connector_bundles_control_inputs_for_reverb_config() {
        let project = sample_project_with_reverb_bundle_inputs();
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        assert_eq!(
            lowered.output_debug,
            vec!["reverb_send(amount=0.6, decay=3.5, damping=0.25, sample(\"bd\"))"]
        );
        assert_eq!(lowered.sample_selectors, vec!["bd".to_string()]);
        assert_eq!(lowered.preview.preview_events.len(), 1);
    }

    #[test]
    fn lowering_tiles_can_express_a_pitched_sine_phrase_chain() {
        let project = sample_project_with_sine_phrase();
        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);
        let compiled = crate::application::authoring::compile::compile_project(&project);

        assert_eq!(lowered.program.outputs.len(), 1);
        assert!(lowered.sample_selectors.is_empty());
        assert!(compiled.can_play);
        assert!(compiled.sample_selectors.is_empty());
        assert_eq!(lowered.preview.preview_events.len(), 12);
        assert!(
            lowered
                .preview
                .preview_events
                .iter()
                .all(|event| event.label == "sine")
        );
        assert_eq!(
            lowered
                .preview
                .preview_events
                .iter()
                .map(|event| event.pitch.clone())
                .collect::<Vec<_>>(),
            vec![
                Some("b4".to_string()),
                Some("d5".to_string()),
                Some("e5".to_string()),
                Some("f#5".to_string()),
                Some("e5".to_string()),
                Some("f#5".to_string()),
                Some("g5".to_string()),
                Some("a5".to_string()),
                Some("f#5".to_string()),
                Some("g5".to_string()),
                Some("a5".to_string()),
                Some("b5".to_string()),
            ]
        );
    }

    #[test]
    fn lowering_keeps_trick_calls_explicit_in_host_ir() {
        let mut project = sample_project();
        project.tricks_mut().push(CadenceTrickWorkspace {
            id: "groove".into(),
            name: "groove".into(),
            graph: Graph {
                nodes: BTreeMap::from([
                    (
                        GridPos { col: 0, row: 0 },
                        Node {
                            piece_id: SUBGRAPH_INPUT_1_ID.into(),
                            inline_params: BTreeMap::from([
                                ("label".into(), Value::String("pattern".into())),
                                ("port_type".into(), Value::String("pattern".into())),
                            ]),
                            pattern_source: None,
                            input_sides: BTreeMap::new(),
                            output_side: Some(TileSide::RIGHT),
                            label: None,
                            node_state: None,
                        },
                    ),
                    (
                        GridPos { col: 1, row: 0 },
                        Node {
                            piece_id: "cadence.fast".into(),
                            inline_params: BTreeMap::from([("factor".into(), Value::from(2.0))]),
                            pattern_source: None,
                            input_sides: BTreeMap::from([
                                ("pattern".into(), TileSide::LEFT),
                                ("factor".into(), TileSide::BOTTOM),
                            ]),
                            output_side: Some(TileSide::RIGHT),
                            label: None,
                            node_state: None,
                        },
                    ),
                    (
                        GridPos { col: 2, row: 0 },
                        Node {
                            piece_id: SUBGRAPH_OUTPUT_ID.into(),
                            inline_params: BTreeMap::new(),
                            pattern_source: None,
                            input_sides: BTreeMap::from([("input".into(), TileSide::LEFT)]),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                ]),
                edges: vec![
                    Edge {
                        id: EdgeId::new(),
                        from: GridPos { col: 0, row: 0 },
                        to_node: GridPos { col: 1, row: 0 },
                        to_param: "pattern".into(),
                    },
                    Edge {
                        id: EdgeId::new(),
                        from: GridPos { col: 1, row: 0 },
                        to_node: GridPos { col: 2, row: 0 },
                        to_param: "input".into(),
                    },
                ]
                .into_iter()
                .map(|edge| (edge.id.clone(), edge))
                .collect(),
                name: "groove".into(),
                cols: 8,
                rows: 8,
            },
        });
        project.runtime_graph_mut().nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "tessera.subgraph.groove".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("arg1".into(), TileSide::LEFT)]),
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        project.runtime_graph_mut().nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "cadence.output".into(),
                inline_params: BTreeMap::new(),
                pattern_source: None,
                input_sides: BTreeMap::from([("pattern".into(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        project.runtime_graph_mut().edges = vec![
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 0, row: 0 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "arg1".into(),
            },
            Edge {
                id: EdgeId::new(),
                from: GridPos { col: 1, row: 0 },
                to_node: GridPos { col: 2, row: 0 },
                to_param: "pattern".into(),
            },
        ]
        .into_iter()
        .map(|edge| (edge.id.clone(), edge))
        .collect();

        let analyzed = crate::adapter::tessera::host_adapter::runtime_engine(&project)
            .analyze(project.runtime_graph());
        let lowered = lower_target_graph(&project, &CadenceGraphTarget::Runtime, &analyzed);

        match &lowered.program.outputs[0].expr {
            CadencePatternExpr::Call { call } => {
                assert_eq!(call.trick_id, "groove");
                assert_eq!(call.arguments.len(), 1);
            }
            other => panic!("expected explicit call expr, got {other:?}"),
        }
        assert_eq!(lowered.output_scores.len(), 1);
        assert_eq!(lowered.preview.preview_events.len(), 2);
    }
}
