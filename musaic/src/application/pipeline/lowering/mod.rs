mod control;

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::state::condition::in_state;
use cadence::prelude::{
    Axis, ConflictPolicy, ControlScore, ControlTile, ControlTrack, Coord, DeduplicateKey,
    DeduplicatePolicy, DeduplicateWinner, DegradePolicy, Intent, Moment, Mosaic, Point3,
    PriorityMergePolicy, Score, SpatialMotion, Time as CycleTime, WeightedScore, reflect, sample,
};
use tessera::prelude::{
    AxisIr, DeduplicateKeyIr, DeduplicatePolicyIr, DeduplicateWinnerIr, EventField, EventValue,
    FieldValue, NodeId, PatternEvent, PatternIr, PatternNodeIr, PatternStream, Point3Ir,
    PriorityConflictIr, PriorityMergePolicyIr, Rational, SpatialMotionIr, WeightedPatternIr,
};

use control::{
    LoweredPattern, combine_lowered, lower_control_stream, lower_event_field,
    lower_pattern_node_as_control, lower_scalar_stream_as_gate, lowered_to_score, shift_lowered,
};

use crate::{
    application::pipeline::runtime::CompiledProject,
    infrastructure::app::MusaicSet,
    infrastructure::diagnostics::{
        AppDiagnostic, DiagnosticPhase, DiagnosticStore, LoweringDiagnostic,
    },
};

/// Tessera IR lowered to host scores keyed by output id.
pub type LoweredScores = BTreeMap<NodeId, Score>;

pub(super) struct LoweringCtx {
    diagnostics: Vec<AppDiagnostic>,
}

impl LoweringCtx {
    pub(super) fn push_unsupported(&mut self, node: impl Into<String>) {
        self.diagnostics.push(AppDiagnostic::Lowering(
            LoweringDiagnostic::UnsupportedPatternNode { node: node.into() },
        ));
    }
}

/// Lowers Tessera pattern IR into cadence scores.
pub fn lower_tessera_ir(ir: &PatternIr) -> (LoweredScores, Vec<AppDiagnostic>) {
    let mut ctx = LoweringCtx {
        diagnostics: Vec::new(),
    };

    let outputs = ir
        .outputs
        .iter()
        .map(|output| {
            (
                output.id.clone(),
                lower_pattern_node(&output.root, &mut ctx),
            )
        })
        .collect();

    (outputs, ctx.diagnostics)
}

fn lower_pattern_node(node: &PatternNodeIr, ctx: &mut LoweringCtx) -> Score {
    lowered_to_score(lower_pattern_node_lowered(node, ctx), ctx)
}

fn lower_pattern_node_lowered(node: &PatternNodeIr, ctx: &mut LoweringCtx) -> LoweredPattern {
    match node {
        PatternNodeIr::EventStream(inner) => lower_event_stream_lowered(&inner.stream, ctx),
        PatternNodeIr::ControlStream(inner) => LoweredPattern {
            events: None,
            controls: lower_control_stream(&inner.stream, ctx),
        },
        PatternNodeIr::ScalarStream(inner) => LoweredPattern {
            events: None,
            controls: lower_scalar_stream_as_gate(&inner.stream, ctx),
        },
        PatternNodeIr::Merge { children } => combine_lowered(
            children
                .iter()
                .map(|child| lower_pattern_node_lowered(child, ctx))
                .collect(),
            ctx,
        ),
        PatternNodeIr::CycleRoute { children } => LoweredPattern {
            events: Some(Score::cycle_route(
                children
                    .iter()
                    .map(|child| lower_pattern_node(child, ctx))
                    .collect(),
            )),
            controls: None,
        },
        PatternNodeIr::CycleSlots { children } => LoweredPattern {
            events: Some(Score::cycle_slots(
                children
                    .iter()
                    .map(|child| lower_pattern_node(child, ctx))
                    .collect(),
            )),
            controls: None,
        },
        PatternNodeIr::TimeScale { inner, factor } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            LoweredPattern {
                events: inner
                    .events
                    .map(|score| Score::time_scale(score, rational_to_time(*factor))),
                controls: inner
                    .controls
                    .map(|controls| ControlScore::time_scale(controls, rational_to_time(*factor))),
            }
        }
        PatternNodeIr::Shift { inner, offset } => shift_lowered(
            lower_pattern_node_lowered(inner, ctx),
            rational_to_time(offset.0),
        ),
        PatternNodeIr::ReflectCycle { inner } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            LoweredPattern {
                events: inner.events.map(reflect),
                controls: inner.controls.map(ControlScore::reflect_cycle),
            }
        }
        PatternNodeIr::SpaceShift { inner, offset } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            let offset = point3_ir_to_point3(*offset);
            LoweredPattern {
                events: inner.events.map(|score| Score::space_shift(score, offset)),
                controls: inner.controls,
            }
        }
        PatternNodeIr::SpaceScale { inner, factor } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            let factor = point3_ir_to_point3(*factor);
            LoweredPattern {
                events: inner.events.map(|score| Score::space_scale(score, factor)),
                controls: inner.controls,
            }
        }
        PatternNodeIr::SpaceReflect { inner, axis } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            let axis = axis_ir_to_axis(*axis);
            LoweredPattern {
                events: inner.events.map(|score| Score::space_reflect(score, axis)),
                controls: inner.controls,
            }
        }
        PatternNodeIr::Degrade {
            inner,
            keep_probability,
            seed,
        } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            LoweredPattern {
                events: inner.events.map(|score| {
                    Score::degrade(
                        score,
                        DegradePolicy::new(rational_to_time(*keep_probability), *seed),
                    )
                }),
                controls: inner.controls,
            }
        }
        PatternNodeIr::Deduplicate { inner, policy } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            LoweredPattern {
                events: inner
                    .events
                    .map(|score| Score::deduplicate(score, lower_deduplicate_policy(*policy))),
                controls: inner.controls,
            }
        }
        PatternNodeIr::PriorityMerge { children, policy } => LoweredPattern {
            events: Some(Score::priority_merge(
                children
                    .iter()
                    .map(|child| lower_pattern_node(child, ctx))
                    .collect(),
                lower_priority_merge_policy(*policy),
            )),
            controls: None,
        },
        PatternNodeIr::WeightedChoice { options, seed } => LoweredPattern {
            events: Some(Score::weighted_choice(
                lower_weighted_options(options, ctx),
                *seed,
            )),
            controls: None,
        },
        PatternNodeIr::MaskClip { source, mask } => {
            let source = lower_pattern_node(source, ctx);
            let mask_controls = lower_pattern_node_as_control(mask, ctx);
            match mask_controls {
                Some(mask) => LoweredPattern {
                    events: Some(Score::mask_clip(source, mask)),
                    controls: None,
                },
                None => {
                    ctx.diagnostics.push(AppDiagnostic::Lowering(
                        LoweringDiagnostic::InvalidControlMapping {
                            key: "mask_clip requires gate control mask".into(),
                        },
                    ));
                    LoweredPattern {
                        events: Some(source),
                        controls: None,
                    }
                }
            }
        }
        PatternNodeIr::Concat { children } => LoweredPattern {
            events: Some(Score::concat(
                children
                    .iter()
                    .map(|child| lower_pattern_node(child, ctx))
                    .collect(),
            )),
            controls: None,
        },
    }
}

fn lower_event_stream_lowered(stream: &PatternStream, ctx: &mut LoweringCtx) -> LoweredPattern {
    let mut field_tiles = Vec::new();
    let moments: Vec<Moment> = stream
        .events
        .iter()
        .filter_map(|event| lower_pattern_event(event, ctx, &mut field_tiles))
        .collect();

    let mut events = if moments.is_empty() && field_tiles.is_empty() {
        None
    } else {
        Some(Score::mosaic(Mosaic::new(moments)))
    };

    if let Some(policy) = degrade_policy_from_stream(stream) {
        if let Some(score) = events {
            events = Some(Score::degrade(score, policy));
        }
    }

    let controls = if field_tiles.is_empty() {
        None
    } else {
        ControlTrack::new(CycleTime::ONE, field_tiles)
            .ok()
            .map(ControlScore::track)
    };

    LoweredPattern { events, controls }
}

fn degrade_policy_from_stream(stream: &PatternStream) -> Option<DegradePolicy> {
    for event in &stream.events {
        for field in &event.fields {
            let EventField::Degrade(value) = field else {
                continue;
            };
            let keep_probability = match value {
                FieldValue::Rational { value } => rational_to_time(*value),
                FieldValue::Bool { value } => {
                    if *value {
                        CycleTime::ONE
                    } else {
                        CycleTime::ZERO
                    }
                }
                _ => continue,
            };
            return Some(DegradePolicy::new(keep_probability, 0));
        }
    }
    None
}

fn lower_pattern_event(
    event: &PatternEvent,
    ctx: &mut LoweringCtx,
    field_tiles: &mut Vec<ControlTile>,
) -> Option<Moment> {
    let intent = lower_event_value(&event.value, ctx)?;
    let start = rational_to_time(event.span.start.0);
    let end = rational_to_time(event.span.end().0);

    for field in &event.fields {
        if matches!(field, EventField::Degrade(_)) {
            continue;
        }
        let Some((key, value)) = lower_event_field(field, ctx) else {
            continue;
        };
        if let Ok(tile) = ControlTile::spanning(start, end, key, value) {
            field_tiles.push(tile);
        }
    }

    let moment = Moment::spanning(start, end, intent)?;
    if event.position.is_origin() {
        Some(moment)
    } else {
        Some(moment.with_position(spatial_motion_ir_to_motion(event.position)))
    }
}

fn rational_to_coord(value: Rational) -> Coord {
    Coord::new(value.numerator, value.denominator)
}

fn point3_ir_to_point3(point: Point3Ir) -> Point3 {
    Point3::new(
        rational_to_coord(point.x),
        rational_to_coord(point.y),
        rational_to_coord(point.z),
    )
}

fn axis_ir_to_axis(axis: AxisIr) -> Axis {
    match axis {
        AxisIr::X => Axis::X,
        AxisIr::Y => Axis::Y,
        AxisIr::Z => Axis::Z,
    }
}

fn spatial_motion_ir_to_motion(motion: SpatialMotionIr) -> SpatialMotion {
    match motion {
        SpatialMotionIr::Static { point } => SpatialMotion::Static(point3_ir_to_point3(point)),
        SpatialMotionIr::Linear { start, end } => SpatialMotion::Linear {
            start: point3_ir_to_point3(start),
            end: point3_ir_to_point3(end),
        },
        SpatialMotionIr::Orbit {
            center,
            radius,
            rate,
            phase,
        } => SpatialMotion::Orbit {
            center: point3_ir_to_point3(center),
            radius: rational_to_coord(radius),
            rate: rational_to_time(rate),
            phase: rational_to_time(phase),
        },
    }
}

fn lower_event_value(value: &EventValue, ctx: &mut LoweringCtx) -> Option<Intent> {
    match value {
        EventValue::Note { value, octave } => {
            let sample_id = match octave {
                Some(octave) => format!("{value}{octave}"),
                None => value.clone(),
            };
            Some(sample(sample_id))
        }
        EventValue::Rest => None,
        EventValue::Scalar { .. } => {
            ctx.push_unsupported("standalone scalar event");
            None
        }
    }
}

fn lower_weighted_options(
    options: &[WeightedPatternIr],
    ctx: &mut LoweringCtx,
) -> Vec<WeightedScore> {
    options
        .iter()
        .map(|option| {
            WeightedScore::new(
                lower_pattern_node(&option.node, ctx),
                rational_to_time(option.weight),
            )
        })
        .collect()
}

pub(super) fn lower_deduplicate_policy(policy: DeduplicatePolicyIr) -> DeduplicatePolicy {
    let key = match policy.key {
        DeduplicateKeyIr::Lifecycle => DeduplicateKey::Lifecycle,
        DeduplicateKeyIr::WholeSpanAndValue => DeduplicateKey::WholeSpanAndIntent,
        DeduplicateKeyIr::StartAndValue => DeduplicateKey::StartAndIntent,
    };
    let winner = match policy.winner {
        DeduplicateWinnerIr::First => DeduplicateWinner::First,
        DeduplicateWinnerIr::Last => DeduplicateWinner::Last,
    };
    DeduplicatePolicy::new(key, winner)
}

fn lower_priority_merge_policy(policy: PriorityMergePolicyIr) -> PriorityMergePolicy {
    let conflict = match policy.conflict {
        PriorityConflictIr::SameWholeStartAndValue => ConflictPolicy::SameWholeStartAndIntent,
        PriorityConflictIr::SameWholeSpanAndValue => ConflictPolicy::SameWholeSpanAndIntent,
        PriorityConflictIr::WholeSpanOverlap => ConflictPolicy::WholeSpanOverlap,
    };
    PriorityMergePolicy::new(conflict)
}

pub(super) fn rational_to_time(value: Rational) -> CycleTime {
    CycleTime::new(value.numerator, value.denominator)
}

/// Registers IR → score lowering. Called by the playback plugin.
pub fn register_lowering(app: &mut App) {
    app.add_systems(
        Update,
        lower_compiled_project
            .in_set(MusaicSet::Lower)
            .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
    );
}

fn lower_compiled_project(
    mut runtime: ResMut<'_, crate::application::pipeline::runtime::RuntimeState>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
    mut active_scores: ResMut<'_, cadence::bevy::ActiveScores>,
) {
    if !runtime.dirty.lowering {
        return;
    }

    let Some(ir) = runtime
        .compiled
        .as_ref()
        .map(|compiled| compiled.tessera_ir.clone())
    else {
        runtime.dirty.lowering = false;
        return;
    };

    let (scores, lowering_diagnostics) = lower_tessera_ir(&ir);
    diagnostics.replace_phase(DiagnosticPhase::Lowering, lowering_diagnostics);

    let string_scores = scores
        .into_iter()
        .map(|(id, score)| (id.0, score))
        .collect();
    active_scores.replace(string_scores);

    match &mut runtime.compiled {
        Some(compiled) => {
            compiled.scores = active_scores.scores.clone();
        }
        None => {
            runtime.compiled = Some(CompiledProject {
                tessera_ir: ir,
                scores: active_scores.scores.clone(),
            });
        }
    }

    runtime.dirty.lowering = false;
    runtime.dirty.runtime = true;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use cadence::prelude::{CadenceCompiler, ControlKey, ControlValue, EvaluatedEventKind, Span};
    use tessera::prelude::{
        ControlEvent, ControlKeyIr, ControlStream, ControlStreamNodeIr, ControlValueIr,
        CycleDuration, CycleSpan, CycleTime, PatternOutput,
    };

    #[test]
    fn event_stream_lowers_to_projectable_non_empty_score() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::event_stream(PatternStream::new(vec![PatternEvent::new(
                CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
                EventValue::Note {
                    value: "bd".into(),
                    octave: None,
                },
            )])),
        )]);
        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty());
        let score = scores.get(&NodeId::new("out")).unwrap();
        let window = Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap();
        let report = CadenceCompiler::new().preview(score, &window).unwrap();
        assert!(!report.projected.moments().is_empty());
    }

    #[test]
    fn merge_with_control_stream_produces_single_start_and_control_updates() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::merge(vec![
                PatternNodeIr::event_stream(PatternStream::new(vec![PatternEvent::new(
                    CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
                    EventValue::Note {
                        value: "pad".into(),
                        octave: None,
                    },
                )])),
                PatternNodeIr::ControlStream(ControlStreamNodeIr {
                    stream: ControlStream::new(vec![
                        ControlEvent::new(
                            CycleSpan::new(
                                CycleTime(Rational::zero()),
                                CycleDuration(Rational::new(1, 2)),
                            ),
                            ControlKeyIr::Gain,
                            ControlValueIr::rational(Rational::new(1, 1)),
                        ),
                        ControlEvent::new(
                            CycleSpan::new(
                                CycleTime(Rational::new(1, 2)),
                                CycleDuration(Rational::new(1, 2)),
                            ),
                            ControlKeyIr::Gain,
                            ControlValueIr::rational(Rational::new(1, 2)),
                        ),
                    ]),
                }),
            ]),
        )]);

        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();
        let window = Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap();
        let report = CadenceCompiler::new().preview(score, &window).unwrap();

        assert_eq!(report.starts().count(), 1);
        assert_eq!(report.control_updates().count(), 1);
        assert!(matches!(
            report.evaluated[1].kind(),
            EvaluatedEventKind::UpdateVoiceControls { .. }
        ));
        assert!(matches!(
            report.evaluated[1].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.5).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn event_field_degrade_wraps_score_with_policy() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::event_stream(PatternStream::new(vec![
                PatternEvent::new(
                    CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
                    EventValue::Note {
                        value: "bd".into(),
                        octave: None,
                    },
                )
                .with_fields(vec![EventField::Degrade(FieldValue::Rational {
                    value: Rational::new(1, 2),
                })]),
            ])),
        )]);

        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();
        let window = Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap();
        let report = CadenceCompiler::new().preview(score, &window).unwrap();
        assert!(report.starts().count() <= 1);
    }

    #[test]
    fn event_field_degrade_with_zero_keep_probability_is_silent() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::event_stream(PatternStream::new(vec![
                PatternEvent::new(
                    CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
                    EventValue::Note {
                        value: "bd".into(),
                        octave: None,
                    },
                )
                .with_fields(vec![EventField::Degrade(FieldValue::Rational {
                    value: Rational::zero(),
                })]),
            ])),
        )]);

        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();
        let window = Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap();
        let report = CadenceCompiler::new().preview(score, &window).unwrap();
        assert!(report.evaluated.is_empty());
    }

    #[test]
    fn mask_clip_limits_visible_span_to_open_gate() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::mask_clip(
                PatternNodeIr::event_stream(PatternStream::new(vec![PatternEvent::new(
                    CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
                    EventValue::Note {
                        value: "pad".into(),
                        octave: None,
                    },
                )])),
                PatternNodeIr::ControlStream(ControlStreamNodeIr {
                    stream: ControlStream::new(vec![ControlEvent::new(
                        CycleSpan::new(
                            CycleTime(Rational::new(1, 4)),
                            CycleDuration(Rational::new(1, 2)),
                        ),
                        ControlKeyIr::Gate,
                        ControlValueIr::bool(true),
                    )]),
                }),
            ),
        )]);

        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();
        let window = Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap();
        let report = CadenceCompiler::new().preview(score, &window).unwrap();

        assert_eq!(report.starts().count(), 1);
        let visible = report.evaluated[0].projected().visible();
        assert_eq!(visible.start(), cadence::prelude::Time::new(1, 4));
        assert_eq!(visible.end(), cadence::prelude::Time::new(3, 4));
    }

    fn note_node(label: &str) -> PatternNodeIr {
        PatternNodeIr::event_stream(PatternStream::new(vec![PatternEvent::new(
            CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
            EventValue::Note {
                value: label.into(),
                octave: None,
            },
        )]))
    }

    fn sample_ids_in_window(
        score: &cadence::prelude::Score,
        start: (i64, i64),
        end: (i64, i64),
    ) -> Vec<String> {
        use cadence::domain::intent::Intent;
        use cadence::prelude::Time;

        let window = Span::new(Time::new(start.0, start.1), Time::new(end.0, end.1)).unwrap();
        let report = CadenceCompiler::new().preview(score, &window).unwrap();
        report
            .projected
            .moments()
            .iter()
            .filter_map(|moment| match moment.intent() {
                Intent::Sample(sample) => Some(sample.sample_id.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn concat_lowering_maps_to_score_kind_concat() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::concat(vec![note_node("a"), note_node("b")]),
        )]);
        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();

        assert_eq!(sample_ids_in_window(score, (0, 1), (1, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(score, (1, 1), (2, 1)), vec!["b"]);
        assert!(sample_ids_in_window(score, (0, 1), (1, 1)).len() < 2);
    }

    #[test]
    fn tessera_chained_slow_program_matches_flatten_cycle_offsets() {
        use tessera::prelude::{
            AtomTile, Container, ContainerAxis, ContainerId, ContainerKind, ContainerSurfaceTile,
            InputEndpoint, InputPort, NodeId, NoteAtom, OutputNode, RootRelation,
            RootSurfaceNodeKind, ScalarAtom, StreamSource, StreamTarget, TesseraCompiler,
            TesseraProgram, TransformKind, TransformNode,
        };

        let mut root_nodes = BTreeMap::new();
        let mut containers = BTreeMap::new();
        for (id, tile) in [
            ("a", AtomTile::Note(NoteAtom::new("a"))),
            ("b", AtomTile::Note(NoteAtom::new("b"))),
        ] {
            containers.insert(
                ContainerId::new(id),
                Container {
                    kind: ContainerKind::Sequence,
                    axis: ContainerAxis::Time,
                    stack: vec![ContainerSurfaceTile::Atom(tile)],
                },
            );
            root_nodes.insert(
                NodeId::new(id),
                RootSurfaceNodeKind::Container {
                    container: ContainerId::new(id),
                },
            );
        }
        containers.insert(
            ContainerId::new("factor"),
            Container {
                kind: ContainerKind::Sequence,
                axis: ContainerAxis::Time,
                stack: vec![ContainerSurfaceTile::Atom(AtomTile::Scalar(
                    ScalarAtom::integer(3),
                ))],
            },
        );
        root_nodes.insert(
            NodeId::new("factor"),
            RootSurfaceNodeKind::Container {
                container: ContainerId::new("factor"),
            },
        );
        root_nodes.insert(
            NodeId::new("slow"),
            RootSurfaceNodeKind::Transform(TransformNode::new(TransformKind::Slow)),
        );
        root_nodes.insert(
            NodeId::new("out"),
            RootSurfaceNodeKind::Output(OutputNode::default()),
        );

        let ir = TesseraCompiler::new()
            .compile_ir(&TesseraProgram {
                root_nodes,
                containers,
                relations: vec![
                    RootRelation::FlowsTo {
                        from: StreamSource::node(NodeId::new("a")),
                        to: StreamTarget::TransformInput {
                            node: NodeId::new("slow"),
                            endpoint: InputEndpoint::Socket(InputPort::new("main")),
                        },
                    },
                    RootRelation::FlowsTo {
                        from: StreamSource::node(NodeId::new("factor")),
                        to: StreamTarget::TransformInput {
                            node: NodeId::new("slow"),
                            endpoint: InputEndpoint::Socket(InputPort::new("factor")),
                        },
                    },
                    RootRelation::ChainedTo {
                        from: StreamSource::node(NodeId::new("slow")),
                        to: NodeId::new("b"),
                    },
                    RootRelation::FlowsTo {
                        from: StreamSource::node(NodeId::new("b")),
                        to: StreamTarget::OutputInput {
                            node: NodeId::new("out"),
                            endpoint: InputEndpoint::GroupMember {
                                group: tessera::prelude::PortGroupId::new("inputs"),
                                member: tessera::prelude::PortMemberId::new("main"),
                            },
                        },
                    },
                ],
            })
            .expect("chained slow program should compile");

        let flattened = ir.outputs[0].stream();
        assert_eq!(flattened.events[1].span.start.0, Rational::from_integer(3));

        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();
        assert_eq!(sample_ids_in_window(score, (3, 1), (4, 1)), vec!["b"]);
    }
}
