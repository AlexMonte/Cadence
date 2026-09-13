mod control;
mod query_source;

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy::state::condition::in_state;
use cadence::prelude::{
    Axis, ConflictPolicy, ControlScore, ControlTile, ControlTrack, Coord, DeduplicateKey,
    DeduplicatePolicy, DeduplicateWinner, DegradePolicy, Intent, Moment, Point3,
    PriorityMergePolicy, Score, SpatialMotion, Time as CycleTime, reflect, sample,
};
use tessera::prelude::{
    AxisIr, DeduplicateKeyIr, DeduplicatePolicyIr, DeduplicateWinnerIr, EventField, EventValue,
    FieldValue, NodeId, PatternEvent, PatternIr, PatternNodeIr, PatternStream, Point3Ir,
    PriorityConflictIr, PriorityMergePolicyIr, Rational, SpatialMotionIr,
};

use control::{
    LoweredPattern, combine_lowered, lower_control_stream, lower_event_field,
    lower_pattern_node_as_control, lower_scalar_stream_as_gate, lower_weighted_choice_parts,
    lowered_to_score, shift_lowered, zip_structural,
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
    instrument: Option<Intent>,
    kit: bool,
    sound_intents: BTreeMap<NodeId, Intent>,
    kit_sounds: BTreeSet<NodeId>,
    source_ids: BTreeMap<NodeId, u64>,
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
    lower_tessera_ir_with_sounds(ir, &BTreeMap::new())
}

/// Resolves Sound tile identities carried by events. Outputs only name lanes.
pub fn lower_tessera_ir_with_sounds(
    ir: &PatternIr,
    sound_intents: &BTreeMap<NodeId, Intent>,
) -> (LoweredScores, Vec<AppDiagnostic>) {
    let (scores, diagnostics, _) = lower_tessera_ir_with_sources(ir, sound_intents);
    (scores, diagnostics)
}

/// Keeps authored tile identities alongside the scores in the same revision.
pub fn lower_tessera_ir_with_sources(
    ir: &PatternIr,
    sound_intents: &BTreeMap<NodeId, Intent>,
) -> (LoweredScores, Vec<AppDiagnostic>, BTreeMap<u64, NodeId>) {
    lower_tessera_ir_with_sources_and_kits(ir, sound_intents, &BTreeSet::new())
}

fn lower_tessera_ir_with_sources_and_kits(
    ir: &PatternIr,
    sound_intents: &BTreeMap<NodeId, Intent>,
    kit_sounds: &BTreeSet<NodeId>,
) -> (LoweredScores, Vec<AppDiagnostic>, BTreeMap<u64, NodeId>) {
    let mut ctx = LoweringCtx {
        diagnostics: Vec::new(),
        instrument: None,
        kit: false,
        sound_intents: sound_intents.clone(),
        kit_sounds: kit_sounds.clone(),
        source_ids: BTreeMap::new(),
    };

    let outputs = ir
        .outputs
        .iter()
        .map(|output| {
            ctx.instrument = None;
            ctx.kit = false;
            (
                output.id.clone(),
                lower_pattern_node(&output.root, &mut ctx),
            )
        })
        .collect();

    let sources = ctx
        .source_ids
        .into_iter()
        .map(|(node, id)| (id, node))
        .collect();
    (outputs, ctx.diagnostics, sources)
}

fn lower_pattern_node(node: &PatternNodeIr, ctx: &mut LoweringCtx) -> Score {
    lowered_to_score(lower_pattern_node_lowered(node, ctx), ctx)
}

fn lower_arrangement(
    segments: &[tessera::prelude::TimedPatternIr],
    ctx: &mut LoweringCtx,
    lower_child: impl Fn(&PatternNodeIr, &mut LoweringCtx) -> LoweredPattern,
) -> LoweredPattern {
    let empty = || LoweredPattern {
        events: None,
        controls: None,
    };
    for segment in segments {
        if let Err(error) = segment.validate() {
            ctx.push_unsupported(format!("Arrangement: {error}"));
            return empty();
        }
    }
    let parts: Vec<_> = segments
        .iter()
        .map(|segment| lower_child(&segment.node, ctx))
        .collect();
    let has_events = parts.iter().any(|part| part.events.is_some());
    let has_controls = parts.iter().any(|part| part.controls.is_some());
    let result = (|| -> Result<LoweredPattern, cadence::prelude::ArrangementError> {
        let mut events = Vec::new();
        let mut controls = Vec::new();
        for (segment, part) in segments.iter().zip(parts) {
            let duration = rational_to_time(segment.duration);
            events.push(cadence::prelude::TimedScore::new(
                part.events.unwrap_or_else(Score::empty),
                duration,
                segment.repeats,
            )?);
            controls.push(cadence::prelude::TimedControlScore::new(
                part.controls.unwrap_or_else(ControlScore::empty),
                duration,
                segment.repeats,
            )?);
        }
        let events = Score::arrange(events)?;
        let controls = ControlScore::arrange(controls)?;
        Ok(LoweredPattern {
            events: has_events.then_some(events),
            controls: has_controls.then_some(controls),
        })
    })();
    result.unwrap_or_else(|error| {
        ctx.push_unsupported(format!("Arrangement: {error}"));
        empty()
    })
}

fn lower_pattern_node_lowered(node: &PatternNodeIr, ctx: &mut LoweringCtx) -> LoweredPattern {
    match node {
        PatternNodeIr::FlowProjection { .. } => query_source::lower(node, ctx, false),
        PatternNodeIr::EventStream(inner) => lower_event_stream_lowered(&inner.stream, ctx),
        PatternNodeIr::CycleEventStream(inner) => lower_cycle_event_stream(&inner.stream, ctx),
        PatternNodeIr::Arrange { segments } => {
            lower_arrangement(segments, ctx, lower_pattern_node_lowered)
        }
        PatternNodeIr::Sequence { children } => {
            control::lower_weighted_slots(children, ctx, lower_pattern_node_lowered)
        }
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
        PatternNodeIr::CycleRoute { children } => zip_structural(
            children
                .iter()
                .map(|child| lower_pattern_node_lowered(child, ctx))
                .collect(),
            Score::cycle_route,
            ControlScore::cycle_route,
        ),
        PatternNodeIr::CycleSlots { children } => zip_structural(
            children
                .iter()
                .map(|child| lower_pattern_node_lowered(child, ctx))
                .collect(),
            Score::cycle_slots,
            ControlScore::cycle_slots,
        ),
        PatternNodeIr::TimeScale { inner, factor } => {
            let inner = lower_pattern_node_lowered(inner, ctx);
            LoweredPattern {
                events: inner.events.map(|score| {
                    Score::time_scale(score, CycleTime::ONE / rational_to_time(*factor))
                }),
                controls: inner.controls.map(|controls| {
                    ControlScore::time_scale(controls, CycleTime::ONE / rational_to_time(*factor))
                }),
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
            // Degrade is Score-only in Cadence; control-only trees must not
            // silently pass through as if degrade applied.
            if inner.events.is_none() && inner.controls.is_some() {
                ctx.push_unsupported(
                    "control-tree Degrade (Score-only; ControlScore has no Degrade variant)",
                );
                return LoweredPattern {
                    events: None,
                    controls: None,
                };
            }
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
            if inner.events.is_none() && inner.controls.is_some() {
                ctx.push_unsupported(
                    "control-tree Deduplicate (Score-only; ControlScore has no Deduplicate variant)",
                );
                return LoweredPattern {
                    events: None,
                    controls: None,
                };
            }
            LoweredPattern {
                events: inner
                    .events
                    .map(|score| Score::deduplicate(score, lower_deduplicate_policy(*policy))),
                controls: inner.controls,
            }
        }
        PatternNodeIr::PriorityMerge { children, policy } => {
            let policy = lower_priority_merge_policy(*policy);
            zip_structural(
                children
                    .iter()
                    .map(|child| lower_pattern_node_lowered(child, ctx))
                    .collect(),
                |scores| Score::priority_merge(scores, policy),
                |controls| ControlScore::priority_merge(controls, policy),
            )
        }
        PatternNodeIr::WeightedChoice { options, seed } => {
            lower_weighted_choice_parts(options, *seed, ctx, lower_pattern_node_lowered)
        }
        PatternNodeIr::MaskClip { source, mask } => {
            let source = lower_pattern_node_lowered(source, ctx);
            let mask_controls = lower_pattern_node_as_control(mask, ctx);
            match (source.events, source.controls, mask_controls) {
                (Some(events), controls, Some(mask)) => {
                    let source_score = match controls {
                        Some(controls) => Score::with_controls(events, controls),
                        None => events,
                    };
                    LoweredPattern {
                        events: Some(Score::mask_clip(source_score, mask)),
                        controls: None,
                    }
                }
                (None, Some(controls), Some(mask)) => LoweredPattern {
                    events: None,
                    controls: Some(ControlScore::mask_clip(controls, mask)),
                },
                (Some(events), controls, None) => {
                    ctx.diagnostics.push(AppDiagnostic::Lowering(
                        LoweringDiagnostic::InvalidControlMapping {
                            key: "mask_clip requires gate control mask".into(),
                        },
                    ));
                    LoweredPattern {
                        events: Some(events),
                        controls,
                    }
                }
                (None, Some(controls), None) => {
                    ctx.diagnostics.push(AppDiagnostic::Lowering(
                        LoweringDiagnostic::InvalidControlMapping {
                            key: "mask_clip requires gate control mask".into(),
                        },
                    ));
                    LoweredPattern {
                        events: None,
                        controls: Some(controls),
                    }
                }
                (None, None, _) => {
                    ctx.diagnostics.push(AppDiagnostic::Lowering(
                        LoweringDiagnostic::InvalidControlMapping {
                            key: "mask_clip source produced no events or controls".into(),
                        },
                    ));
                    LoweredPattern {
                        events: None,
                        controls: None,
                    }
                }
            }
        }
        PatternNodeIr::Concat { children } => zip_structural(
            children
                .iter()
                .map(|child| lower_pattern_node_lowered(child, ctx))
                .collect(),
            Score::concat,
            ControlScore::concat,
        ),
    }
}

fn lower_event_stream_lowered(stream: &PatternStream, ctx: &mut LoweringCtx) -> LoweredPattern {
    let notes: Vec<Score> = stream
        .events
        .iter()
        .filter_map(|event| {
            let mut fields = Vec::new();
            let moment = lower_pattern_event(event, ctx, &mut fields)?;
            Some(bind_event_fields(Score::events(vec![moment]), fields))
        })
        .collect();

    let mut events = if notes.is_empty() {
        None
    } else {
        Some(cadence::prelude::merge(notes))
    };

    if let Some(policy) = degrade_policy_from_stream(stream) {
        if let Some(score) = events {
            events = Some(Score::degrade(score, policy));
        }
    }

    LoweredPattern {
        events,
        controls: None,
    }
}

fn lower_cycle_event_stream(stream: &PatternStream, ctx: &mut LoweringCtx) -> LoweredPattern {
    use cadence::prelude::{Tile, Voice, WeightedScore};
    let mut notes = Vec::new();
    let mut origin = None;
    for event in &stream.events {
        let mut fields = Vec::new();
        let Some(moment) = lower_pattern_event(event, ctx, &mut fields) else {
            continue;
        };
        origin = Some(
            origin.map_or(moment.span().start(), |previous: cadence::prelude::Time| {
                previous.min(moment.span().start())
            }),
        );
        let tile = Tile::spanning(
            moment.span().start(),
            moment.span().end(),
            moment.intent().clone(),
        )
        .map(|tile| {
            let mut tile = tile.with_position(moment.position());
            if let Some(value) = moment.value_identity() {
                tile = tile.with_value_identity(value.clone());
            }
            match moment.id() {
                Some(id) => tile.with_id(id.value()),
                None => tile,
            }
        });
        let Some(voice) = tile.and_then(|tile| Voice::new(CycleTime::ONE, vec![tile])) else {
            ctx.push_unsupported("cycle-local event outside its one-cycle period");
            return LoweredPattern {
                events: None,
                controls: None,
            };
        };
        notes.push(bind_event_fields(Score::voice(voice), fields));
    }
    let origin = origin.unwrap_or(CycleTime::ZERO);
    let score = cadence::prelude::merge(notes);
    // Each note owns its fields, while the authored leaf still owns one cycle.
    // A single weighted slot queries its child unchanged; the shifts retain
    // the original leaf origin for concatenation when its first note is late.
    let score = if origin == CycleTime::ZERO {
        score
    } else {
        Score::shift(score, CycleTime::ZERO - origin)
    };
    let score = Score::weighted_cycle_slots(vec![WeightedScore::new(score, CycleTime::ONE)]);
    let score = if origin == CycleTime::ZERO {
        score
    } else {
        Score::shift(score, origin)
    };
    let score = if let Some(policy) = degrade_policy_from_stream(stream) {
        Score::degrade(score, policy)
    } else {
        score
    };
    LoweredPattern {
        events: Some(score),
        controls: None,
    }
}

/// A field belongs to this note, even when another note occupies the same time.
/// Attach it before any enclosing sequence or layer can combine the voices.
fn bind_event_fields(score: Score, fields: Vec<ControlTile>) -> Score {
    if fields.is_empty() {
        return score;
    }
    match ControlTrack::new(CycleTime::ONE, fields) {
        Ok(track) => Score::with_controls(score, ControlScore::track(track)),
        Err(_) => score,
    }
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
    let previous = ctx.instrument.take();
    let previous_kit = ctx.kit;
    let source_instrument = event
        .source
        .as_ref()
        .and_then(|source| source.instrument.as_ref());
    ctx.instrument = source_instrument
        .and_then(|node| ctx.sound_intents.get(node))
        .cloned()
        .or_else(|| previous.clone());
    ctx.kit = source_instrument
        .map(|node| ctx.kit_sounds.contains(node))
        .unwrap_or(previous_kit);
    let intent = lower_event_value(&event.value, ctx);
    ctx.instrument = previous;
    ctx.kit = previous_kit;
    let mut intent = intent?;
    if !control::resolve_sample_region(&event.fields, &mut intent, ctx) {
        return None;
    }
    let start = rational_to_time(event.span.start.0);
    let end = rational_to_time(event.span.end().0);

    if let EventValue::Note { value, octave } = &event.value {
        if let Some(pitch) = chromatic_pitch(value, *octave) {
            if let Ok(tile) = ControlTile::spanning(
                start,
                end,
                cadence::prelude::ControlKey::Pitch,
                cadence::prelude::ControlValue::Scalar(pitch),
            ) {
                field_tiles.push(tile);
            }
        }
    }

    let transpose = event
        .fields
        .iter()
        .filter_map(|field| match field {
            EventField::Transpose(FieldValue::Rational { value }) => {
                Some(value.numerator as f64 / value.denominator as f64)
            }
            _ => None,
        })
        .reduce(|a, b| a + b);
    if let Some(transpose) = transpose {
        if let Ok(tile) = ControlTile::spanning(
            start,
            end,
            cadence::prelude::ControlKey::Transpose,
            cadence::prelude::ControlValue::Scalar(transpose),
        ) {
            field_tiles.push(tile);
        }
    }

    for field in &event.fields {
        if matches!(
            field,
            EventField::Degrade(_)
                | EventField::Transpose(FieldValue::Rational { .. })
                | EventField::Slice(_)
                | EventField::PlaybackStart(_)
                | EventField::PlaybackEnd(_)
        ) {
            continue;
        }
        let Some((key, value)) = lower_event_field(field, ctx) else {
            continue;
        };
        if matches!(intent, Intent::Synth(_)) && !key.spec().support().supports_synth() {
            ctx.push_unsupported(format!(
                "{} requires a connected sample Sound tile",
                key.spec().canonical_name()
            ));
            continue;
        }
        if matches!(intent, Intent::Sample(_)) && !key.spec().support().supports_sample() {
            ctx.push_unsupported(format!(
                "{} does not support the assigned sample instrument",
                key.spec().canonical_name()
            ));
            continue;
        }
        if let Ok(tile) = ControlTile::spanning(start, end, key, value) {
            field_tiles.push(tile);
        }
    }

    let mut moment =
        Moment::spanning(start, end, intent)?.with_value_identity(cadence::prelude::Symbol::new(
            serde_json::to_string(&event.value).expect("musical event values serialize"),
        ));
    if let Some(node) = event
        .source
        .as_ref()
        .and_then(|source| source.node.as_ref())
    {
        let next_id = ctx.source_ids.len() as u64;
        let id = *ctx.source_ids.entry(node.clone()).or_insert(next_id);
        moment = moment.with_id(id);
    }
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
            if ctx.kit {
                ctx.push_unsupported("Kit Sound tiles accept named drum-hit tiles");
                return None;
            }
            if chromatic_pitch(value, *octave).is_some() {
                return Some(
                    ctx.instrument.clone().unwrap_or_else(|| {
                        Intent::synth(cadence::prelude::BuiltInSynthSource::Sine)
                    }),
                );
            }
            let sample_id = match octave {
                Some(octave) => format!("{value}{octave}"),
                None => value.clone(),
            };
            Some(sample(sample_id))
        }
        EventValue::Sound { value } => {
            if !ctx.kit {
                ctx.push_unsupported(format!("named drum hit {value} requires a Kit Sound tile"));
                return None;
            }
            if !["bd", "sd", "hh", "oh"].contains(&value.as_str()) {
                ctx.push_unsupported(format!("unknown built-in drum hit {value}"));
                return None;
            }
            match ctx.instrument.clone() {
                Some(Intent::Sample(mut sample)) => {
                    sample.sample_id = value.clone();
                    Some(Intent::Sample(sample))
                }
                Some(_) => {
                    ctx.push_unsupported("Kit instrument did not resolve to a sample intent");
                    None
                }
                None => {
                    ctx.push_unsupported("named drum hit requires a connected Kit Sound tile");
                    None
                }
            }
        }
        EventValue::Rest => None,
        EventValue::Scalar { .. } => {
            ctx.push_unsupported("standalone scalar event");
            None
        }
    }
}

/// Pitched note spelling is independent of sample identity. MIDI C4 is 60.
fn chromatic_pitch(value: &str, octave: Option<i64>) -> Option<f64> {
    let mut chars = value.chars();
    let natural = match chars.next()?.to_ascii_lowercase() {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let accidental = match chars.next() {
        None => 0,
        Some('#' | '♯') => 1,
        Some('b' | '♭') => -1,
        Some('♮') => 0,
        _ => return None,
    };
    if chars.next().is_some() {
        return None;
    }
    Some(((octave.unwrap_or(4) + 1) * 12 + natural + accidental) as f64)
}

pub(super) fn lower_deduplicate_policy(policy: DeduplicatePolicyIr) -> DeduplicatePolicy {
    let key = match policy.key {
        DeduplicateKeyIr::Lifecycle => DeduplicateKey::Lifecycle,
        DeduplicateKeyIr::WholeSpanAndValue => DeduplicateKey::WholeSpanAndValue,
        DeduplicateKeyIr::StartAndValue => DeduplicateKey::StartAndValue,
    };
    let winner = match policy.winner {
        DeduplicateWinnerIr::First => DeduplicateWinner::First,
        DeduplicateWinnerIr::Last => DeduplicateWinner::Last,
    };
    DeduplicatePolicy::new(key, winner)
}

pub(super) fn lower_priority_merge_policy(policy: PriorityMergePolicyIr) -> PriorityMergePolicy {
    let conflict = match policy.conflict {
        PriorityConflictIr::SameWholeStartAndValue => ConflictPolicy::SameWholeStartAndValue,
        PriorityConflictIr::SameWholeSpanAndValue => ConflictPolicy::SameWholeSpanAndValue,
        PriorityConflictIr::WholeSpanOverlap => ConflictPolicy::WholeSpanOverlap,
    };
    PriorityMergePolicy::new(conflict)
}

pub(super) fn rational_to_time(value: Rational) -> CycleTime {
    CycleTime::new(value.numerator, value.denominator)
}

/// Shared sound binding for realtime, preview, and offline rendering.
pub fn lower_project_ir(
    project: &crate::application::session::MusaicProject,
    ir: &PatternIr,
) -> (LoweredScores, Vec<AppDiagnostic>, BTreeMap<u64, NodeId>) {
    lower_project_scope(project, ir)
}

fn lower_project_scope(
    project: &crate::application::session::MusaicProject,
    ir: &PatternIr,
) -> (LoweredScores, Vec<AppDiagnostic>, BTreeMap<u64, NodeId>) {
    let mut sound_intents = BTreeMap::new();
    let mut kit_sounds = BTreeSet::new();
    let mut binding_errors = Vec::new();
    let assignments: BTreeMap<_, _> = project
        .document
        .graph
        .nodes()
        .filter_map(|node| {
            let crate::domain::document::DocumentNodeKind::Sound(sound) = &node.kind else {
                return None;
            };
            Some((node.id.clone(), sound.definition.clone()))
        })
        .collect();
    for (sound, definition) in &assignments {
        if let crate::domain::instrument::InstrumentSource::Sample(id) = definition.source {
            if !crate::adapter::audio::project_samples::source_available(project, id) {
                binding_errors.push(AppDiagnostic::Lowering(
                    LoweringDiagnostic::UnsupportedPatternNode {
                        node: format!(
                            "Sound {} needs missing sample {} or one of its bank variants",
                            sound.0, id.0
                        ),
                    },
                ));
                continue;
            }
        }
        if matches!(
            definition.source,
            crate::domain::instrument::InstrumentSource::Kit
        ) {
            kit_sounds.insert(sound.clone());
        }
        match definition.intent() {
            Ok(mut intent) => {
                if let (
                    crate::domain::instrument::InstrumentSource::Sample(id),
                    Intent::Sample(sample),
                ) = (&definition.source, &mut intent)
                {
                    if let Some(gain) = project
                        .samples
                        .manifest()
                        .samples
                        .get(id)
                        .and_then(|metadata| metadata.options.default_gain)
                    {
                        sample.gain *= f64::from(gain);
                    }
                }
                sound_intents.insert(sound.clone(), intent);
            }
            Err(error) => binding_errors.push(AppDiagnostic::Lowering(
                LoweringDiagnostic::UnsupportedPatternNode { node: error },
            )),
        }
    }
    let (scores, mut lowering_diagnostics, source_nodes) =
        lower_tessera_ir_with_sources_and_kits(ir, &sound_intents, &kit_sounds);

    lowering_diagnostics.extend(binding_errors);
    (scores, lowering_diagnostics, source_nodes)
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
    project: Res<'_, crate::application::session::MusaicProject>,
    mut runtime: ResMut<'_, crate::application::pipeline::runtime::RuntimeState>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
    mut active_scores: ResMut<'_, cadence::bevy::ActiveScores>,
) {
    let Some(ir) = runtime.pending_ir.take() else {
        return;
    };

    // A mixer/sample change can reuse accepted music while the board contains
    // unfinished syntax. Keep that music's routing instead of capturing edits
    // which were never compiled into this score.
    let activity_routing = runtime
        .pending_activity_routing
        .take()
        .or_else(|| {
            runtime
                .proposed
                .as_ref()
                .map(|(_, _, compiled)| compiled)
                .filter(|compiled| compiled.tessera_ir == ir)
                .map(|compiled| compiled.activity_routing.clone())
        })
        .or_else(|| {
            runtime
                .compiled
                .as_ref()
                .filter(|compiled| compiled.tessera_ir == ir)
                .map(|compiled| compiled.activity_routing.clone())
        })
        .unwrap_or_else(|| {
            super::runtime::activity::ActivityRouting::from_document(&project.document)
        });

    let (scores, lowering_diagnostics, source_nodes) = lower_project_ir(&project, &ir);
    let valid = lowering_diagnostics.is_empty();
    diagnostics.replace_phase(DiagnosticPhase::Lowering, lowering_diagnostics);
    if !valid {
        runtime.revisions.attempted_preparation =
            runtime.revisions.document.max(runtime.revisions.sound);
        return;
    }

    let string_scores: BTreeMap<String, Score> = scores
        .into_iter()
        .map(|(id, score)| (id.0, score))
        .collect();
    let source_revision = runtime.revisions.document.max(runtime.revisions.sound);
    runtime.revisions.attempted_preparation = source_revision;
    let prepared = match cadence::prelude::PreparedScore::new(cadence::prelude::merge(
        string_scores.values().cloned().collect(),
    )) {
        Ok(prepared) => prepared,
        Err(error) => {
            diagnostics.replace_phase(
                DiagnosticPhase::Lowering,
                [AppDiagnostic::Lowering(
                    LoweringDiagnostic::UnsupportedPatternNode {
                        node: error.to_string(),
                    },
                )],
            );
            return;
        }
    };
    let prepared_scores: BTreeMap<String, cadence::prelude::PreparedScore> = match string_scores
        .into_iter()
        .map(|(id, score)| cadence::prelude::PreparedScore::new(score).map(|score| (id, score)))
        .collect()
    {
        Ok(scores) => scores,
        Err(error) => {
            diagnostics.replace_phase(
                DiagnosticPhase::Lowering,
                [AppDiagnostic::Lowering(
                    LoweringDiagnostic::UnsupportedPatternNode {
                        node: error.to_string(),
                    },
                )],
            );
            return;
        }
    };
    active_scores.replace(prepared_scores.len(), prepared.clone());
    runtime.revisions.proposed_audio = source_revision;

    runtime.proposed = Some((
        active_scores.revision,
        source_revision,
        CompiledProject {
            activity_routing,
            tessera_ir: ir,
            source_nodes,
            scores: prepared_scores,
            playback_score: prepared,
        },
    ));
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

    fn preview_score(score: &Score, window: &Span) -> cadence::prelude::PreviewReport {
        let prepared = cadence::prelude::PreparedScore::new(score.clone())
            .expect("test score should fit the preparation budget");
        CadenceCompiler::new().preview(&prepared, window).unwrap()
    }

    #[test]
    fn combined_speed_limits_are_checked_before_project_preview_or_audio_publication() {
        let event = PatternEvent::new(
            CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
            EventValue::Note {
                value: "c".into(),
                octave: Some(4),
            },
        );
        let source = PatternNodeIr::cycle_event_stream(PatternStream::new(vec![event]));
        let fast = PatternNodeIr::time_scale(
            PatternNodeIr::time_scale(source, Rational::new(1, 1000)),
            Rational::new(1, 1000),
        );
        let ir = PatternIr::new(vec![PatternOutput::new(NodeId::new("out"), fast)]);
        let (scores, diagnostics, _) = lower_project_ir(
            &crate::application::session::MusaicProject::new_empty(),
            &ir,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let error = cadence::prelude::PreparedScore::new(cadence::prelude::merge(
            scores.into_values().collect(),
        ))
        .unwrap_err();
        assert!(error.to_string().contains("preparation budget"));
    }

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
        let report = preview_score(score, &window);
        assert!(report.starts().next().is_some());
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
        let report = preview_score(score, &window);

        assert_eq!(report.starts().count(), 1);
        assert_eq!(report.control_updates().count(), 1);
        assert!(matches!(
            report.events[1].kind(),
            EvaluatedEventKind::UpdateVoiceControls { .. }
        ));
        assert!(matches!(
            report.events[1].projected().controls().get(&ControlKey::Gain),
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
        let report = preview_score(score, &window);
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
        let report = preview_score(score, &window);
        assert!(report.events.is_empty());
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
        let report = preview_score(score, &window);

        assert_eq!(report.starts().count(), 1);
        let visible = report.events[0].projected().visible();
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

    fn scoped_note(label: &str, fields: Vec<EventField>, periodic: bool) -> PatternNodeIr {
        let stream = PatternStream::new(vec![
            PatternEvent::new(
                CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
                EventValue::Note {
                    value: label.into(),
                    octave: Some(4),
                },
            )
            .with_fields(fields),
        ]);
        if periodic {
            PatternNodeIr::cycle_event_stream(stream)
        } else {
            PatternNodeIr::event_stream(stream)
        }
    }

    fn scoped_score(node: PatternNodeIr) -> Score {
        let ir = PatternIr::new(vec![PatternOutput::new(NodeId::new("out"), node)]);
        let (mut scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        scores.remove(&NodeId::new("out")).unwrap()
    }

    #[test]
    fn scoped_authored_layer_retains_three_distinct_chord_pitches() {
        use tessera::prelude::{AtomTile, Board, ContainerSurfaceTile, NoteAtom, TesseraCompiler};
        let mut board = Board::new();
        board
            .at(0, 0)
            .named("chord")
            .layer_container(
                ["c", "e", "g"]
                    .into_iter()
                    .map(|label| {
                        ContainerSurfaceTile::Atom(AtomTile::Note(
                            NoteAtom::new(label).with_octave(4),
                        ))
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        board.at(1, 0).named("out").output().unwrap();
        let compiled = TesseraCompiler::new()
            .compile_authored(&board.finish())
            .unwrap();
        let (scores, diagnostics) = lower_tessera_ir(&compiled.ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        for cycle in 0..8 {
            let mut pitches =
                note_pitches_in_window(&scores[&NodeId::new("out")], (cycle, 1), (cycle + 1, 1));
            pitches.sort();
            assert_eq!(pitches, vec![60, 64, 67]);
        }
    }

    #[test]
    fn scoped_note_fields_do_not_leak_between_simultaneous_notes() {
        for periodic in [false, true] {
            let nodes = [("c", Rational::new(1, 4)), ("e", Rational::new(3, 4))]
                .into_iter()
                .map(|(note, gain)| {
                    scoped_note(
                        note,
                        vec![EventField::Gain(FieldValue::rational(gain))],
                        periodic,
                    )
                })
                .collect();
            let score = scoped_score(PatternNodeIr::merge(nodes));
            let report = preview_score(
                &score,
                &Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap(),
            );
            let mut values = report
                .starts()
                .map(|event| {
                    let controls = event.projected().controls();
                    let Some(ControlValue::Scalar(pitch)) = controls.get(&ControlKey::Pitch) else {
                        panic!("missing pitch");
                    };
                    let Some(ControlValue::Scalar(gain)) = controls.get(&ControlKey::Gain) else {
                        panic!("missing gain");
                    };
                    (*pitch as i64, *gain)
                })
                .collect::<Vec<_>>();
            values.sort_by_key(|value| value.0);
            assert_eq!(values, vec![(60, 0.25), (64, 0.75)]);
        }
    }

    #[test]
    fn scoped_flow_gain_preserves_nested_sequence_alternate_and_seek() {
        use tessera::prelude::WeightedPatternIr;
        let sequence = PatternNodeIr::sequence(vec![
            WeightedPatternIr::new(
                Rational::one(),
                PatternNodeIr::cycle_route(vec![
                    scoped_note("c", vec![], true),
                    scoped_note("e", vec![], true),
                ]),
            ),
            WeightedPatternIr::new(Rational::one(), scoped_note("g", vec![], true)),
        ]);
        let branch = PatternNodeIr::merge(vec![
            sequence,
            PatternNodeIr::cycle_route(vec![
                gain_control_node(Rational::zero(), Rational::one(), Rational::new(1, 4)),
                gain_control_node(Rational::zero(), Rational::one(), Rational::new(1, 2)),
            ]),
        ]);
        let other = PatternNodeIr::merge(vec![
            scoped_note("b", vec![], true),
            gain_control_node(Rational::zero(), Rational::one(), Rational::new(3, 4)),
        ]);
        let score = scoped_score(PatternNodeIr::merge(vec![branch, other]));
        for cycle in 0..8 {
            let mut voice_ids = BTreeMap::new();
            for (from, until, expect_last_note) in [
                ((cycle, 1), (cycle + 1, 1), true),
                ((cycle * 4 + 1, 4), (cycle * 3 + 1, 3), false),
            ] {
                let report = preview_score(
                    &score,
                    &Span::new(
                        cadence::prelude::Time::new(from.0, from.1),
                        cadence::prelude::Time::new(until.0, until.1),
                    )
                    .unwrap(),
                );
                if !expect_last_note {
                    assert_eq!(
                        report.starts().count(),
                        0,
                        "seeking inside held notes must not retrigger them"
                    );
                    assert_eq!(report.control_updates().count(), 2);
                }
                let mut values = report
                    .events
                    .iter()
                    .map(|event| {
                        let moment = event.projected();
                        let Some(ControlValue::Scalar(pitch)) =
                            moment.controls().get(&ControlKey::Pitch)
                        else {
                            panic!("missing pitch");
                        };
                        let Some(ControlValue::Scalar(gain)) =
                            moment.controls().get(&ControlKey::Gain)
                        else {
                            panic!("missing gain");
                        };
                        let voice_id = match event.kind() {
                            EvaluatedEventKind::StartVoice { voice_id, .. }
                            | EvaluatedEventKind::UpdateVoiceControls { voice_id, .. } => *voice_id,
                        };
                        if expect_last_note {
                            voice_ids.insert(*pitch as i64, voice_id);
                        } else {
                            assert_eq!(
                                voice_ids.get(&(*pitch as i64)),
                                Some(&voice_id),
                                "seek changed note identity"
                            );
                        }
                        (*pitch as i64, *gain)
                    })
                    .collect::<Vec<_>>();
                values.sort_by_key(|value| value.0);
                let branch_gain = if cycle % 2 == 0 { 0.25 } else { 0.5 };
                let mut expected = vec![(if cycle % 2 == 0 { 60 } else { 64 }, branch_gain)];
                if expect_last_note {
                    expected.push((67, branch_gain));
                }
                expected.push((71, 0.75));
                assert_eq!(
                    values, expected,
                    "cycle {cycle}, direct seek {}",
                    !expect_last_note
                );
            }
        }
    }

    #[test]
    fn scoped_multi_event_leaves_preserve_finite_and_periodic_concat_extent() {
        for periodic in [false, true] {
            let stream = PatternStream::new(
                [("c", 0), ("e", 1)]
                    .into_iter()
                    .map(|(label, half)| {
                        PatternEvent::new(
                            CycleSpan::new(
                                CycleTime(Rational::new(half, 2)),
                                CycleDuration(Rational::new(1, 2)),
                            ),
                            EventValue::Note {
                                value: label.into(),
                                octave: Some(4),
                            },
                        )
                    })
                    .collect(),
            );
            let leaf = if periodic {
                PatternNodeIr::cycle_event_stream(stream)
            } else {
                PatternNodeIr::event_stream(stream)
            };
            let score = scoped_score(PatternNodeIr::concat(vec![
                leaf,
                scoped_note("g", vec![], false),
            ]));
            assert_eq!(note_pitches_in_window(&score, (0, 1), (1, 1)), vec![60, 64]);
            assert_eq!(note_pitches_in_window(&score, (1, 1), (2, 1)), vec![67]);
        }
    }

    fn note_pitches_in_window(
        score: &cadence::prelude::Score,
        start: (i64, i64),
        end: (i64, i64),
    ) -> Vec<i64> {
        use cadence::prelude::Time;

        let window = Span::new(Time::new(start.0, start.1), Time::new(end.0, end.1)).unwrap();
        let report = preview_score(score, &window);
        report
            .starts()
            .map(
                |event| match event.projected().controls().get(&ControlKey::Pitch) {
                    Some(ControlValue::Scalar(pitch)) => *pitch as i64,
                    _ => panic!("note is missing its pitch"),
                },
            )
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

        assert_eq!(note_pitches_in_window(score, (0, 1), (1, 1)), vec![69]);
        assert_eq!(note_pitches_in_window(score, (1, 1), (2, 1)), vec![71]);
        assert!(note_pitches_in_window(score, (0, 1), (1, 1)).len() < 2);
    }

    fn gain_control_node(start: Rational, duration: Rational, gain: Rational) -> PatternNodeIr {
        PatternNodeIr::ControlStream(ControlStreamNodeIr {
            stream: ControlStream::new(vec![ControlEvent::new(
                CycleSpan::new(CycleTime(start), CycleDuration(duration)),
                ControlKeyIr::Gain,
                ControlValueIr::rational(gain),
            )]),
        })
    }

    fn gate_control_node(start: Rational, duration: Rational, open: bool) -> PatternNodeIr {
        PatternNodeIr::ControlStream(ControlStreamNodeIr {
            stream: ControlStream::new(vec![ControlEvent::new(
                CycleSpan::new(CycleTime(start), CycleDuration(duration)),
                ControlKeyIr::Gate,
                ControlValueIr::bool(open),
            )]),
        })
    }

    fn note_with_gain(label: &str, gain: Rational) -> PatternNodeIr {
        PatternNodeIr::merge(vec![
            note_node(label),
            gain_control_node(Rational::zero(), Rational::one(), gain),
        ])
    }

    #[test]
    fn concat_lowering_preserves_control_children() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::concat(vec![
                note_with_gain("a", Rational::one()),
                note_with_gain("b", Rational::new(1, 2)),
            ]),
        )]);
        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();

        let window_a =
            Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap();
        let report_a = preview_score(score, &window_a);
        assert_eq!(note_pitches_in_window(score, (0, 1), (1, 1)), vec![69]);
        assert!(matches!(
            report_a.events[0].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 1.0).abs() < f64::EPSILON
        ));

        let window_b = Span::new(
            cadence::prelude::Time::ONE,
            cadence::prelude::Time::new(2, 1),
        )
        .unwrap();
        let report_b = preview_score(score, &window_b);
        assert_eq!(note_pitches_in_window(score, (1, 1), (2, 1)), vec![71]);
        assert!(matches!(
            report_b.events[0].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.5).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn cycle_route_lowering_preserves_control_children() {
        // Finite event leaves occur only at their authored time. Pair a single
        // note with CycleRoute controls so control-tree routing is observable.
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::merge(vec![
                note_node("pad"),
                PatternNodeIr::cycle_route(vec![
                    gain_control_node(Rational::zero(), Rational::one(), Rational::one()),
                    gain_control_node(Rational::zero(), Rational::one(), Rational::new(1, 2)),
                ]),
            ]),
        )]);
        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();

        let report_0 = preview_score(
            score,
            &Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap(),
        );
        assert_eq!(report_0.starts().count(), 1);
        assert!(matches!(
            report_0.events[0].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 1.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn mask_clip_control_source_uses_mask_semantics_not_merge() {
        // Merge(event, MaskClip(gain, gate)) must clip gain visibility — not
        // silently merge gate+gain as peer controls.
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::merge(vec![
                note_node("pad"),
                PatternNodeIr::mask_clip(
                    gain_control_node(Rational::zero(), Rational::one(), Rational::one()),
                    gate_control_node(Rational::new(1, 4), Rational::new(1, 2), true),
                ),
            ]),
        )]);
        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();
        let window = Span::new(cadence::prelude::Time::ZERO, cadence::prelude::Time::ONE).unwrap();
        let report = preview_score(score, &window);

        assert_eq!(report.starts().count(), 1);
        // Gate must not appear as an attached control lane.
        assert!(
            report.events[0]
                .projected()
                .controls()
                .get(&ControlKey::Gate)
                .is_none()
        );
        assert!(matches!(
            report.events[0].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 1.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn control_tree_degrade_emits_unsupported_diagnostic() {
        let ir = PatternIr::new(vec![PatternOutput::new(
            NodeId::new("out"),
            PatternNodeIr::merge(vec![
                note_node("pad"),
                PatternNodeIr::Degrade {
                    inner: Box::new(gain_control_node(
                        Rational::zero(),
                        Rational::one(),
                        Rational::one(),
                    )),
                    keep_probability: Rational::new(1, 2),
                    seed: 0,
                },
            ]),
        )]);
        let (_scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic,
                    AppDiagnostic::Lowering(LoweringDiagnostic::UnsupportedPatternNode { node })
                        if node.contains("control-tree Degrade")
                )
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn tessera_chained_slow_program_matches_flatten_cycle_offsets() {
        use tessera::prelude::{
            AtomTile, AuthoredTesseraProgram, Container, ContainerAxis, ContainerId, ContainerKind,
            ContainerSurfaceTile, InputEndpoint, InputPort, NodeId, NoteAtom, OutputNode,
            RootPlacement, RootRelation, RootSurface, RootSurfaceNodeKind, ScalarAtom,
            StreamSource, StreamTarget, TesseraCompiler, TransformKind, TransformNode,
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
                    source_nodes: Default::default(),
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
                source_nodes: Default::default(),
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

        let placements = root_nodes
            .keys()
            .enumerate()
            .map(|(index, node)| (node.clone(), RootPlacement::unit(index as i32 * 3, 0)))
            .collect();
        let compiled = TesseraCompiler::new()
            .compile_authored(&AuthoredTesseraProgram {
                root_surface: RootSurface {
                    nodes: root_nodes,
                    placements,
                    bindings: Default::default(),
                    explicit_relations: vec![
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
                },
                containers,
            })
            .expect("chained slow program should compile");
        let ir = compiled.ir;

        let flattened = ir.outputs[0].stream();
        assert_eq!(flattened.events[1].span.start.0, Rational::from_integer(3));

        let (scores, diagnostics) = lower_tessera_ir(&ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let score = scores.get(&NodeId::new("out")).unwrap();
        assert_eq!(note_pitches_in_window(score, (3, 1), (4, 1)), vec![71]);
    }
}
