use cadence::prelude::{
    ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue, Score, SignedUnitValue,
    Symbol, Time as CycleTime, UnitValue, WeightedControlScore, WeightedScore,
};
use tessera::prelude::{
    ControlEvent, ControlKeyIr, ControlStream, ControlValueIr, EventField, FieldValue,
    PatternNodeIr, Rational, ScalarStream, WeightedPatternIr,
};

use crate::infrastructure::diagnostics::{AppDiagnostic, LoweringDiagnostic};

use super::{LoweringCtx, lower_priority_merge_policy, rational_to_time};

/// Event-shaped and/or control-shaped lowering result.
pub(super) struct LoweredPattern {
    pub events: Option<cadence::prelude::Score>,
    pub controls: Option<ControlScore>,
}

pub(super) fn lower_control_key(key: &ControlKeyIr, _ctx: &mut LoweringCtx) -> Option<ControlKey> {
    let mapped = match key {
        ControlKeyIr::Gate => ControlKey::Gate,
        ControlKeyIr::Legato => ControlKey::Legato,
        ControlKeyIr::Transpose => ControlKey::Transpose,
        ControlKeyIr::SampleBank => ControlKey::SampleBank,
        ControlKeyIr::SampleVariant => ControlKey::SampleVariant,
        ControlKeyIr::Gain => ControlKey::Gain,
        ControlKeyIr::PostGain => ControlKey::PostGain,
        ControlKeyIr::Pan => ControlKey::Pan,
        ControlKeyIr::Velocity => ControlKey::Velocity,
        ControlKeyIr::ClipLength => ControlKey::ClipLength,
        ControlKeyIr::Expression => ControlKey::Expression,

        ControlKeyIr::Pitch => ControlKey::Pitch,
        ControlKeyIr::PitchBend => ControlKey::PitchBend,
        ControlKeyIr::PlaybackRate => ControlKey::PlaybackRate,
        ControlKeyIr::PlaybackStart => ControlKey::PlaybackStart,
        ControlKeyIr::PlaybackEnd => ControlKey::PlaybackEnd,
        ControlKeyIr::Reverse => ControlKey::Reverse,
        ControlKeyIr::Fit => ControlKey::Fit,
        ControlKeyIr::Loop => ControlKey::Loop,
        ControlKeyIr::Attack => ControlKey::Attack,
        ControlKeyIr::Decay => ControlKey::Decay,
        ControlKeyIr::Sustain => ControlKey::Sustain,
        ControlKeyIr::Release => ControlKey::Release,
        ControlKeyIr::LowPassCutoff => ControlKey::LowPassCutoff,
        ControlKeyIr::LowPassResonance => ControlKey::LowPassResonance,
        ControlKeyIr::HighPassCutoff => ControlKey::HighPassCutoff,
        ControlKeyIr::HighPassResonance => ControlKey::HighPassResonance,
        ControlKeyIr::ReverbSend => ControlKey::ReverbSend,
        ControlKeyIr::DelaySend => ControlKey::DelaySend,
        ControlKeyIr::Compressor => ControlKey::Compressor,
        ControlKeyIr::Select(name) => ControlKey::Select(Symbol::new(name.clone())),
        ControlKeyIr::Custom(name) => ControlKey::Custom(Symbol::new(name.clone())),
    };

    if matches!(key, ControlKeyIr::Select(_) | ControlKeyIr::Custom(_)) {
        // Mapped successfully; custom/select are supported as domain keys.
    }

    Some(mapped)
}

pub(super) fn lower_control_value(
    key: &ControlKey,
    value: &ControlValueIr,
    ctx: &mut LoweringCtx,
) -> Option<ControlValue> {
    let result = match (key, value) {
        (_, ControlValueIr::Modulation { value }) => {
            use tessera::prelude::{ModulationWaveform as W, ParameterKey as P};
            let parameter = match key {
                ControlKey::Gain => P::Gain,
                ControlKey::Velocity => P::Velocity,
                ControlKey::PlaybackRate => P::PlaybackRate,
                ControlKey::LowPassCutoff => P::LowPassCutoff,
                ControlKey::Transpose => P::Transpose,
                _ => {
                    ctx.push_unsupported("This lane does not support signal modulation.");
                    return None;
                }
            };
            if let Err(reason) = value.validate_for(parameter) {
                ctx.push_unsupported(reason);
                return None;
            }
            let waveform = match value.waveform {
                W::Sine => cadence::prelude::Waveform::Sine,
                W::Saw => cadence::prelude::Waveform::Saw,
                W::Triangle => cadence::prelude::Waveform::Tri,
                W::Square => cadence::prelude::Waveform::Square,
                W::SmoothNoise => cadence::prelude::Waveform::Perlin { seed: value.seed },
                W::Random => cadence::prelude::Waveform::Random { seed: value.seed },
                W::SteppedNoise => cadence::prelude::Waveform::Rand { seed: value.seed },
                W::Ramp => cadence::prelude::Waveform::Ramp,
            };
            let min = rational_to_f64(value.minimum);
            let max = rational_to_f64(value.maximum);
            let (bias, depth) = if value.waveform == W::Ramp {
                (min, max - min)
            } else {
                ((min + max) / 2.0, (max - min) / 2.0)
            };
            ControlValue::Signal(
                cadence::prelude::Signal::new(waveform)
                    .with_rate(cadence::prelude::Time::new(
                        value.rate.numerator,
                        value.rate.denominator,
                    ))
                    .with_phase(cadence::prelude::Time::new(
                        value.phase.numerator,
                        value.phase.denominator,
                    ))
                    .with_bias(bias)
                    .with_depth(depth),
            )
        }
        (_, ControlValueIr::Delay { value }) => {
            if let Err(reason) = value.validate() {
                ctx.push_unsupported(reason);
                return None;
            }
            ControlValue::Delay(cadence::prelude::DelaySettings::new(
                UnitValue::new(rational_to_f64(value.amount)).unwrap(),
                std::time::Duration::from_secs_f64(rational_to_f64(value.time)),
                UnitValue::new(rational_to_f64(value.feedback)).unwrap(),
                UnitValue::new(rational_to_f64(value.damping)).unwrap(),
            ))
        }
        (_, ControlValueIr::Reverb { value }) => {
            if let Err(reason) = value.validate() {
                ctx.push_unsupported(reason);
                return None;
            }
            ControlValue::Reverb(cadence::prelude::ReverbSettings::new(
                UnitValue::new(rational_to_f64(value.amount)).unwrap(),
                std::time::Duration::from_secs_f64(rational_to_f64(value.decay)),
                UnitValue::new(rational_to_f64(value.damping)).unwrap(),
            ))
        }
        (_, ControlValueIr::Compressor { value }) => {
            if let Err(reason) = value.validate() {
                ctx.push_unsupported(reason);
                return None;
            }
            ControlValue::Compressor(
                cadence::prelude::CompressorSettings::new(
                    UnitValue::new(rational_to_f64(value.threshold)).unwrap(),
                    rational_to_f64(value.ratio),
                    std::time::Duration::from_secs_f64(rational_to_f64(value.attack)),
                    std::time::Duration::from_secs_f64(rational_to_f64(value.release)),
                )
                .with_knee_db(rational_to_f64(value.knee_db)),
            )
        }
        (ControlKey::Gate, ControlValueIr::Bool { value }) => ControlValue::Bool(*value),
        (ControlKey::Gate, ControlValueIr::Rational { value }) => {
            ControlValue::Bool(!value.is_zero())
        }
        (
            ControlKey::Reverse | ControlKey::Fit | ControlKey::Loop | ControlKey::SustainPedal,
            ControlValueIr::Bool { value },
        ) => ControlValue::Bool(*value),
        (
            ControlKey::Reverse | ControlKey::Fit | ControlKey::Loop | ControlKey::SustainPedal,
            ControlValueIr::Rational { value },
        ) => ControlValue::Bool(!value.is_zero()),
        (ControlKey::Velocity | ControlKey::Sustain, ControlValueIr::Rational { value }) => {
            let scalar = rational_to_f64(*value);
            UnitValue::new(scalar)
                .map(ControlValue::Unipolar)
                .unwrap_or(ControlValue::Scalar(scalar))
        }
        (ControlKey::Expression | ControlKey::ModWheel, ControlValueIr::Rational { value }) => {
            let scalar = rational_to_f64(*value);
            UnitValue::new(scalar)
                .map(ControlValue::Unipolar)
                .unwrap_or(ControlValue::Scalar(scalar))
        }
        (ControlKey::PitchBend | ControlKey::Pan, ControlValueIr::Rational { value }) => {
            let scalar = rational_to_f64(*value);
            SignedUnitValue::new(scalar)
                .map(ControlValue::Bipolar)
                .unwrap_or(ControlValue::Scalar(scalar))
        }
        (_, ControlValueIr::Rational { value }) => ControlValue::Scalar(rational_to_f64(*value)),
        (_, ControlValueIr::Bool { value }) => ControlValue::Bool(*value),
        (_, ControlValueIr::Symbol { value }) => ControlValue::Choice(Symbol::new(value.clone())),
    };

    if result.validate_for(key).is_err() {
        ctx.diagnostics.push(AppDiagnostic::Lowering(
            LoweringDiagnostic::InvalidControlMapping {
                key: format!("{key:?}"),
            },
        ));
        return None;
    }

    Some(result)
}

pub(super) fn lower_field_value(
    key: &ControlKey,
    value: &FieldValue,
    ctx: &mut LoweringCtx,
) -> Option<ControlValue> {
    let ir = match value {
        FieldValue::Modulation { value } => ControlValueIr::Modulation { value: *value },
        FieldValue::Delay { value } => ControlValueIr::Delay { value: *value },
        FieldValue::Reverb { value } => ControlValueIr::Reverb { value: *value },
        FieldValue::Compressor { value } => ControlValueIr::Compressor { value: *value },
        FieldValue::Slice { .. } => {
            ctx.push_unsupported(
                "A slice is an owned source selection, not a scalar control value",
            );
            return None;
        }
        FieldValue::Rational { value } => ControlValueIr::Rational { value: *value },
        FieldValue::Bool { value } => ControlValueIr::Bool { value: *value },
        FieldValue::Symbol { value } => ControlValueIr::Symbol {
            value: value.clone(),
        },
    };
    lower_control_value(key, &ir, ctx)
}

pub(super) fn lower_event_field(
    field: &EventField,
    ctx: &mut LoweringCtx,
) -> Option<(ControlKey, ControlValue)> {
    let (key_ir, value) = match field {
        EventField::Gate(value) => (ControlKeyIr::Gate, value),
        EventField::Legato(value) => (ControlKeyIr::Legato, value),
        EventField::SampleBank(value) => (ControlKeyIr::SampleBank, value),
        EventField::SampleVariant(value) => (ControlKeyIr::SampleVariant, value),
        EventField::Gain(value) => (ControlKeyIr::Gain, value),
        EventField::PostGain(value) => (ControlKeyIr::PostGain, value),
        EventField::Pan(value) => (ControlKeyIr::Pan, value),
        EventField::Velocity(value) => (ControlKeyIr::Velocity, value),
        EventField::ClipLength(value) => (ControlKeyIr::ClipLength, value),
        EventField::Expression(value) => (ControlKeyIr::Expression, value),

        EventField::Pitch(value) => (ControlKeyIr::Pitch, value),
        EventField::PitchBend(value) => (ControlKeyIr::PitchBend, value),
        EventField::PlaybackRate(value) => (ControlKeyIr::PlaybackRate, value),
        EventField::PlaybackStart(value) => (ControlKeyIr::PlaybackStart, value),
        EventField::PlaybackEnd(value) => (ControlKeyIr::PlaybackEnd, value),
        EventField::Reverse(value) => (ControlKeyIr::Reverse, value),
        EventField::Fit(value) => (ControlKeyIr::Fit, value),
        EventField::Loop(value) => (ControlKeyIr::Loop, value),
        EventField::Slice(_) => {
            ctx.push_unsupported("A slice must be resolved against its assigned sample source");
            return None;
        }
        EventField::Attack(value) => (ControlKeyIr::Attack, value),
        EventField::Decay(value) => (ControlKeyIr::Decay, value),
        EventField::Sustain(value) => (ControlKeyIr::Sustain, value),
        EventField::Release(value) => (ControlKeyIr::Release, value),
        EventField::LowPassCutoff(value) => (ControlKeyIr::LowPassCutoff, value),
        EventField::LowPassResonance(value) => (ControlKeyIr::LowPassResonance, value),
        EventField::HighPassCutoff(value) => (ControlKeyIr::HighPassCutoff, value),
        EventField::HighPassResonance(value) => (ControlKeyIr::HighPassResonance, value),
        EventField::ReverbSend(value) => (ControlKeyIr::ReverbSend, value),
        EventField::DelaySend(value) => (ControlKeyIr::DelaySend, value),
        EventField::Compressor(value) => (ControlKeyIr::Compressor, value),
        EventField::Select(value) => {
            let name = match value {
                FieldValue::Symbol { value } => value.clone(),
                _ => {
                    ctx.push_unsupported("select field requires symbol value");
                    return None;
                }
            };
            let key = ControlKey::Select(Symbol::new(name));
            let control_value = lower_field_value(&key, value, ctx)?;
            return Some((key, control_value));
        }
        EventField::Custom { key, value } => {
            let control_key = lower_control_key(&ControlKeyIr::Custom(key.clone()), ctx)?;
            let control_value = lower_field_value(&control_key, value, ctx)?;
            return Some((control_key, control_value));
        }
        EventField::Transpose(value) => (ControlKeyIr::Transpose, value),
        EventField::Degrade(_) => return None,
        EventField::Elongate(_) | EventField::Replicate(_) | EventField::RandomChoice => {
            ctx.push_unsupported(format!("event field {field:?}"));
            return None;
        }
    };

    let key = lower_control_key(&key_ir, ctx)?;
    let control_value = lower_field_value(&key, value, ctx)?;
    Some((key, control_value))
}

/// Resolve region operands as one source operation before attaching note controls.
/// Bounds are absolute normalized sample positions. A slice divides that effective
/// region, regardless of where the region and slice tiles occur in the note.
pub(super) fn resolve_sample_region(
    fields: &[EventField],
    intent: &mut cadence::prelude::Intent,
    ctx: &mut LoweringCtx,
) -> bool {
    use tessera::prelude::ParameterKey;
    if !fields.iter().any(|field| {
        matches!(
            field,
            EventField::PlaybackStart(_) | EventField::PlaybackEnd(_) | EventField::Slice(_)
        )
    }) {
        return true;
    }
    let cadence::prelude::Intent::Sample(sample) = intent else {
        ctx.push_unsupported("Sample region and slice tiles require a connected sample Sound tile");
        return false;
    };
    let mut start = sample.start;
    let mut end = sample.end;
    let mut slice = None;
    for field in fields {
        let (key, value) = match field {
            EventField::PlaybackStart(value) => (ParameterKey::PlaybackStart, value),
            EventField::PlaybackEnd(value) => (ParameterKey::PlaybackEnd, value),
            EventField::Slice(value) => (ParameterKey::Slice, value),
            _ => continue,
        };
        if let Err(reason) = key.spec().validate(value) {
            ctx.push_unsupported(format!("{}: {reason}", key.spec().label));
            return false;
        }
        match (key, value) {
            (ParameterKey::PlaybackStart, FieldValue::Rational { value }) => {
                start = rational_to_f64(*value)
            }
            (ParameterKey::PlaybackEnd, FieldValue::Rational { value }) => {
                end = rational_to_f64(*value)
            }
            (ParameterKey::Slice, FieldValue::Slice { index, count }) => {
                if slice.replace((*index, *count)).is_some() {
                    ctx.push_unsupported(
                        "A note can select only one slice; each slice owns its index and count",
                    );
                    return false;
                }
            }
            _ => unreachable!("validated source operand"),
        }
    }
    if !start.is_finite() || !end.is_finite() || start < 0.0 || start >= end || end > 1.0 {
        ctx.push_unsupported("Sample region must satisfy 0 <= start < end <= 1");
        return false;
    }
    let mut selected = sample.clone().region(start, end);
    if let Some((index, count)) = slice {
        match selected.slice(index, count) {
            Ok(value) => selected = value,
            Err(error) => {
                ctx.push_unsupported(format!("Invalid sample slice: {error:?}"));
                return false;
            }
        }
    }
    *sample = selected;
    true
}

fn rational_to_f64(value: Rational) -> f64 {
    value.numerator as f64 / value.denominator as f64
}

pub(super) fn lower_control_stream(
    stream: &ControlStream,
    ctx: &mut LoweringCtx,
) -> Option<ControlScore> {
    let mut tiles = Vec::new();
    for control in &stream.controls {
        let key = lower_control_key(&control.key, ctx)?;
        let value = lower_control_value(&key, &control.value, ctx)?;
        let start = rational_to_time(control.span.start.0);
        let end = rational_to_time(control.span.end().0);
        let tile = ControlTile::spanning(start, end, key, value).ok()?;
        tiles.push(tile);
    }

    if tiles.is_empty() {
        return None;
    }

    ControlTrack::new(CycleTime::ONE, tiles)
        .ok()
        .map(ControlScore::track)
}

pub(super) fn lower_scalar_stream_as_gate(
    stream: &ScalarStream,
    ctx: &mut LoweringCtx,
) -> Option<ControlScore> {
    let controls = stream
        .values
        .iter()
        .map(|event| {
            ControlEvent::new(
                event.span,
                ControlKeyIr::Gate,
                ControlValueIr::bool(!event.value.is_zero()),
            )
        })
        .collect();
    lower_control_stream(&ControlStream::new(controls), ctx)
}

pub(super) fn lower_control_node(node: &PatternNodeIr, ctx: &mut LoweringCtx) -> LoweredPattern {
    match node {
        PatternNodeIr::FlowProjection { .. } => super::query_source::lower(node, ctx, true),
        PatternNodeIr::Arrange { segments } => {
            super::lower_arrangement(segments, ctx, lower_control_node)
        }
        PatternNodeIr::Sequence { children } => {
            lower_weighted_slots(children, ctx, lower_control_node)
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
                .map(|child| lower_control_node(child, ctx))
                .collect(),
            ctx,
        ),
        PatternNodeIr::CycleRoute { children } => zip_structural(
            children
                .iter()
                .map(|child| lower_control_node(child, ctx))
                .collect(),
            Score::cycle_route,
            ControlScore::cycle_route,
        ),
        PatternNodeIr::CycleSlots { children } => zip_structural(
            children
                .iter()
                .map(|child| lower_control_node(child, ctx))
                .collect(),
            Score::cycle_slots,
            ControlScore::cycle_slots,
        ),
        PatternNodeIr::TimeScale { inner, factor } => {
            let inner = lower_control_node(inner, ctx);
            LoweredPattern {
                events: None,
                controls: inner.controls.map(|controls| {
                    ControlScore::time_scale(controls, CycleTime::ONE / rational_to_time(*factor))
                }),
            }
        }
        PatternNodeIr::Shift { inner, offset } => {
            let inner = lower_control_node(inner, ctx);
            LoweredPattern {
                events: None,
                controls: inner
                    .controls
                    .map(|controls| ControlScore::shift(controls, rational_to_time(offset.0))),
            }
        }
        PatternNodeIr::ReflectCycle { inner } => {
            let inner = lower_control_node(inner, ctx);
            LoweredPattern {
                events: None,
                controls: inner.controls.map(ControlScore::reflect_cycle),
            }
        }
        // Spatial transforms only move event positions; control lanes carry no
        // position, so they pass through untouched.
        PatternNodeIr::SpaceShift { inner, .. }
        | PatternNodeIr::SpaceScale { inner, .. }
        | PatternNodeIr::SpaceReflect { inner, .. } => lower_control_node(inner, ctx),
        PatternNodeIr::Degrade { .. } => {
            ctx.push_unsupported(
                "control-tree Degrade (Score-only; ControlScore has no Degrade variant)",
            );
            LoweredPattern {
                events: None,
                controls: None,
            }
        }
        PatternNodeIr::Deduplicate { .. } => {
            ctx.push_unsupported(
                "control-tree Deduplicate (Score-only; ControlScore has no Deduplicate variant)",
            );
            LoweredPattern {
                events: None,
                controls: None,
            }
        }
        PatternNodeIr::PriorityMerge { children, policy } => {
            let policy = lower_priority_merge_policy(*policy);
            zip_structural(
                children
                    .iter()
                    .map(|child| lower_control_node(child, ctx))
                    .collect(),
                |scores| Score::priority_merge(scores, policy),
                |controls| ControlScore::priority_merge(controls, policy),
            )
        }
        PatternNodeIr::WeightedChoice { options, seed } => {
            lower_weighted_choice_parts(options, *seed, ctx, lower_control_node)
        }
        // Intentional: EventStream is event-only (HOST_API). Control lowering
        // discards it without a diagnostic — there is no control material to map.
        PatternNodeIr::EventStream(_) | PatternNodeIr::CycleEventStream(_) => LoweredPattern {
            events: None,
            controls: None,
        },
        PatternNodeIr::MaskClip { source, mask } => {
            let source = lower_control_node(source, ctx);
            let mask_controls = source_mask_controls(mask, ctx);
            match (source.controls, mask_controls) {
                (Some(source), Some(mask)) => LoweredPattern {
                    events: None,
                    controls: Some(ControlScore::mask_clip(source, mask)),
                },
                (None, _) => {
                    ctx.diagnostics.push(AppDiagnostic::Lowering(
                        LoweringDiagnostic::InvalidControlMapping {
                            key: "mask_clip control source produced no controls".into(),
                        },
                    ));
                    LoweredPattern {
                        events: None,
                        controls: None,
                    }
                }
                (Some(_), None) => {
                    // Diagnostic already emitted by source_mask_controls.
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
                .map(|child| lower_control_node(child, ctx))
                .collect(),
            Score::concat,
            ControlScore::concat,
        ),
    }
}

pub(super) fn lower_weighted_slots(
    children: &[WeightedPatternIr],
    ctx: &mut LoweringCtx,
    lower_child: fn(&PatternNodeIr, &mut LoweringCtx) -> LoweredPattern,
) -> LoweredPattern {
    let parts: Vec<_> = children
        .iter()
        .map(|child| {
            (
                lower_child(&child.node, ctx),
                rational_to_time(child.weight),
            )
        })
        .collect();
    let events = parts
        .iter()
        .any(|(part, _)| part.events.is_some())
        .then(|| {
            Score::weighted_cycle_slots(
                parts
                    .iter()
                    .map(|(part, weight)| {
                        WeightedScore::new(
                            part.events.clone().unwrap_or_else(Score::empty),
                            *weight,
                        )
                    })
                    .collect(),
            )
        });
    let controls = parts
        .iter()
        .any(|(part, _)| part.controls.is_some())
        .then(|| {
            ControlScore::weighted_cycle_slots(
                parts
                    .iter()
                    .map(|(part, weight)| {
                        WeightedControlScore::new(
                            part.controls.clone().unwrap_or_else(ControlScore::empty),
                            *weight,
                        )
                    })
                    .collect(),
            )
        });
    LoweredPattern { events, controls }
}

fn source_mask_controls(mask: &PatternNodeIr, ctx: &mut LoweringCtx) -> Option<ControlScore> {
    let mask = lower_control_node(mask, ctx);
    match mask.controls {
        Some(controls) => Some(controls),
        None => {
            ctx.diagnostics.push(AppDiagnostic::Lowering(
                LoweringDiagnostic::InvalidControlMapping {
                    key: "mask_clip requires gate control mask".into(),
                },
            ));
            None
        }
    }
}

/// Preserve child cardinality on both event and control trees so structural
/// ops (cycle route/slots, concat, priority merge) stay aligned.
pub(super) fn zip_structural(
    parts: Vec<LoweredPattern>,
    wrap_events: impl FnOnce(Vec<Score>) -> Score,
    wrap_controls: impl FnOnce(Vec<ControlScore>) -> ControlScore,
) -> LoweredPattern {
    let has_events = parts.iter().any(|part| part.events.is_some());
    let has_controls = parts.iter().any(|part| part.controls.is_some());
    LoweredPattern {
        events: has_events.then(|| {
            wrap_events(
                parts
                    .iter()
                    .map(|part| part.events.clone().unwrap_or_else(Score::empty))
                    .collect(),
            )
        }),
        controls: has_controls.then(|| {
            wrap_controls(
                parts
                    .iter()
                    .map(|part| part.controls.clone().unwrap_or_else(ControlScore::empty))
                    .collect(),
            )
        }),
    }
}

pub(super) fn lower_weighted_choice_parts(
    options: &[WeightedPatternIr],
    seed: u64,
    ctx: &mut LoweringCtx,
    lower_child: impl Fn(&PatternNodeIr, &mut LoweringCtx) -> LoweredPattern,
) -> LoweredPattern {
    let parts: Vec<(LoweredPattern, CycleTime)> = options
        .iter()
        .map(|option| {
            (
                lower_child(&option.node, ctx),
                rational_to_time(option.weight),
            )
        })
        .collect();
    let has_events = parts.iter().any(|(part, _)| part.events.is_some());
    let has_controls = parts.iter().any(|(part, _)| part.controls.is_some());
    LoweredPattern {
        events: has_events.then(|| {
            Score::weighted_choice(
                parts
                    .iter()
                    .map(|(part, weight)| {
                        WeightedScore::new(
                            part.events.clone().unwrap_or_else(Score::empty),
                            *weight,
                        )
                    })
                    .collect(),
                seed,
            )
        }),
        controls: has_controls.then(|| {
            ControlScore::weighted_choice(
                parts
                    .iter()
                    .map(|(part, weight)| {
                        WeightedControlScore::new(
                            part.controls.clone().unwrap_or_else(ControlScore::empty),
                            *weight,
                        )
                    })
                    .collect(),
                seed,
            )
        }),
    }
}

pub(super) fn lower_pattern_node_as_control(
    node: &PatternNodeIr,
    ctx: &mut LoweringCtx,
) -> Option<ControlScore> {
    let lowered = lower_control_node(node, ctx);
    if lowered.events.is_some() {
        ctx.diagnostics.push(AppDiagnostic::Lowering(
            LoweringDiagnostic::InvalidControlMapping {
                key: "expected control-shaped mask".into(),
            },
        ));
        return None;
    }
    lowered.controls
}

pub(super) fn combine_lowered(
    parts: Vec<LoweredPattern>,
    _ctx: &mut LoweringCtx,
) -> LoweredPattern {
    let mut events = Vec::new();
    let mut controls = Vec::new();

    for part in parts {
        if let Some(score) = part.events {
            events.push(score);
        }
        if let Some(control) = part.controls {
            controls.push(control);
        }
    }

    let events = match events.len() {
        0 => None,
        1 => Some(events.remove(0)),
        _ => Some(cadence::prelude::merge(events)),
    };
    let controls = match controls.len() {
        0 => None,
        1 => Some(controls.remove(0)),
        _ => Some(ControlScore::merge(controls)),
    };
    // Merge(event pattern, control pattern) is the scope of an explicit flow
    // modifier. Resolve that scope here; lifting its controls into a parent
    // layer would also modify the unrelated sibling voices in that layer.
    match (events, controls) {
        (Some(events), Some(controls)) => LoweredPattern {
            events: Some(Score::with_controls(events, controls)),
            controls: None,
        },
        (events, controls) => LoweredPattern { events, controls },
    }
}

pub(super) fn shift_lowered(mut lowered: LoweredPattern, offset: CycleTime) -> LoweredPattern {
    if offset == CycleTime::ZERO {
        return lowered;
    }
    if let Some(score) = lowered.events.take() {
        lowered.events = Some(cadence::prelude::shift(score, offset));
    }
    if let Some(controls) = lowered.controls.take() {
        lowered.controls = Some(ControlScore::shift(controls, offset));
    }
    lowered
}

pub(super) fn lowered_to_score(
    lowered: LoweredPattern,
    ctx: &mut LoweringCtx,
) -> cadence::prelude::Score {
    match (lowered.events, lowered.controls) {
        (Some(events), Some(controls)) => cadence::prelude::Score::with_controls(events, controls),
        (Some(events), None) => events,
        (None, Some(_)) => {
            ctx.push_unsupported("control-only pattern output");
            cadence::prelude::Score::empty()
        }
        (None, None) => cadence::prelude::Score::empty(),
    }
}
