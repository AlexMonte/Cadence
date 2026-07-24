use cadence::prelude::{
    ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue, SignedUnitValue, Symbol,
    Time as CycleTime, UnitValue,
};
use tessera::prelude::{
    ControlEvent, ControlKeyIr, ControlStream, ControlValueIr, EventField, FieldValue,
    PatternNodeIr, Rational, ScalarStream,
};

use crate::infrastructure::diagnostics::{AppDiagnostic, LoweringDiagnostic};

use super::{LoweringCtx, rational_to_time};

/// Event-shaped and/or control-shaped lowering result.
pub(super) struct LoweredPattern {
    pub events: Option<cadence::prelude::Score>,
    pub controls: Option<ControlScore>,
}

pub(super) fn lower_control_key(key: &ControlKeyIr, _ctx: &mut LoweringCtx) -> Option<ControlKey> {
    let mapped = match key {
        ControlKeyIr::Gate => ControlKey::Gate,
        ControlKeyIr::Gain => ControlKey::Gain,
        ControlKeyIr::PostGain => ControlKey::PostGain,
        ControlKeyIr::Pitch => ControlKey::Pitch,
        ControlKeyIr::PitchBend => ControlKey::PitchBend,
        ControlKeyIr::PlaybackRate => ControlKey::PlaybackRate,
        ControlKeyIr::PlaybackStart => ControlKey::PlaybackStart,
        ControlKeyIr::PlaybackEnd => ControlKey::PlaybackEnd,
        ControlKeyIr::Reverse => ControlKey::Reverse,
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
        (ControlKey::Gate, ControlValueIr::Bool { value }) => ControlValue::Bool(*value),
        (ControlKey::Gate, ControlValueIr::Rational { value }) => {
            ControlValue::Bool(!value.is_zero())
        }
        (ControlKey::Reverse | ControlKey::SustainPedal, ControlValueIr::Bool { value }) => {
            ControlValue::Bool(*value)
        }
        (ControlKey::Reverse | ControlKey::SustainPedal, ControlValueIr::Rational { value }) => {
            ControlValue::Bool(!value.is_zero())
        }
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
        (ControlKey::PitchBend, ControlValueIr::Rational { value }) => {
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
        EventField::Gain(value) => (ControlKeyIr::Gain, value),
        EventField::PostGain(value) => (ControlKeyIr::PostGain, value),
        EventField::Pitch(value) => (ControlKeyIr::Pitch, value),
        EventField::PitchBend(value) => (ControlKeyIr::PitchBend, value),
        EventField::PlaybackRate(value) => (ControlKeyIr::PlaybackRate, value),
        EventField::PlaybackStart(value) => (ControlKeyIr::PlaybackStart, value),
        EventField::PlaybackEnd(value) => (ControlKeyIr::PlaybackEnd, value),
        EventField::Reverse(value) => (ControlKeyIr::Reverse, value),
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
        EventField::Transpose(value) => {
            let control_key =
                lower_control_key(&ControlKeyIr::Custom("transpose".to_string()), ctx)?;
            let control_value = lower_field_value(&control_key, value, ctx)?;
            return Some((control_key, control_value));
        }
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
        PatternNodeIr::CycleRoute { children } => LoweredPattern {
            events: None,
            controls: Some(ControlScore::cycle_route(
                children
                    .iter()
                    .filter_map(|child| lower_control_node(child, ctx).controls)
                    .collect(),
            )),
        },
        PatternNodeIr::CycleSlots { children } => LoweredPattern {
            events: None,
            controls: Some(ControlScore::cycle_slots(
                children
                    .iter()
                    .filter_map(|child| lower_control_node(child, ctx).controls)
                    .collect(),
            )),
        },
        PatternNodeIr::TimeScale { inner, factor } => {
            let inner = lower_control_node(inner, ctx);
            LoweredPattern {
                events: None,
                controls: inner
                    .controls
                    .map(|controls| ControlScore::time_scale(controls, rational_to_time(*factor))),
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
        PatternNodeIr::Degrade { .. }
        | PatternNodeIr::Deduplicate { .. }
        | PatternNodeIr::PriorityMerge { .. }
        | PatternNodeIr::WeightedChoice { .. } => {
            ctx.push_unsupported(format!("control lowering for {node:?}"));
            LoweredPattern {
                events: None,
                controls: None,
            }
        }
        PatternNodeIr::EventStream(_) => LoweredPattern {
            events: None,
            controls: None,
        },
        PatternNodeIr::MaskClip { source, mask } => {
            let source = lower_control_node(source, ctx);
            let mask = lower_control_node(mask, ctx);
            LoweredPattern {
                events: None,
                controls: match (source.controls, mask.controls) {
                    (Some(source), Some(mask)) => Some(ControlScore::merge(vec![source, mask])),
                    (None, Some(mask)) => Some(mask),
                    (Some(source), None) => Some(source),
                    (None, None) => None,
                },
            }
        }
        PatternNodeIr::Concat { children } => {
            let controls: Vec<_> = children
                .iter()
                .filter_map(|child| lower_control_node(child, ctx).controls)
                .collect();
            LoweredPattern {
                events: None,
                controls: if controls.is_empty() {
                    None
                } else {
                    Some(ControlScore::concat(controls))
                },
            }
        }
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

    LoweredPattern {
        events: match events.len() {
            0 => None,
            1 => Some(events.remove(0)),
            _ => Some(cadence::prelude::merge(events)),
        },
        controls: match controls.len() {
            0 => None,
            1 => Some(controls.remove(0)),
            _ => Some(ControlScore::merge(controls)),
        },
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
