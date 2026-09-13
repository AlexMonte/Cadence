//! Query-time language flow stays recursive in an immutable planning source.
use super::{
    LoweringCtx,
    control::{self, LoweredPattern},
    lower_pattern_event, rational_to_time,
};
use cadence::prelude::{
    ControlKey, ControlModelError, ControlQuerySource, ControlScore, ControlValue, Intent, Moment,
    QueryControl, QueryMoment, Score, ScoreQuerySource, Span, Time,
};
use std::collections::{BTreeMap, BTreeSet};
use tessera::prelude::{
    ControlKeyIr, CycleDuration, CycleSpan, CycleTime, EventField, EventValue, NodeId,
    ParameterKey, PatternNodeIr, PatternStreamShape, Rational,
};

#[derive(Debug, Clone)]
struct LanguageSource {
    node: PatternNodeIr,
    instrument: Option<Intent>,
    kit: bool,
    sound_intents: BTreeMap<NodeId, Intent>,
    kit_sounds: BTreeSet<NodeId>,
    sources: BTreeMap<NodeId, u64>,
}
pub(super) fn lower(
    node: &PatternNodeIr,
    ctx: &mut LoweringCtx,
    force_control: bool,
) -> LoweredPattern {
    // Walk every possible musical leaf, including unselected future branches,
    // for validation and source IDs before publishing this immutable snapshot.
    node.map_event_leaves(&mut |stream, _| {
        for event in &stream.events {
            if event.value.is_scalar() || event.value.is_rest() {
                continue;
            }
            let mut normalized = event.clone();
            normalized.span =
                CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()));
            let _ = lower_pattern_event(&normalized, ctx, &mut Vec::new());
        }
        PatternNodeIr::cycle_event_stream(stream.clone())
    });
    let source = LanguageSource {
        node: node.clone(),
        instrument: ctx.instrument.clone(),
        kit: ctx.kit,
        sound_intents: ctx.sound_intents.clone(),
        kit_sounds: ctx.kit_sounds.clone(),
        sources: ctx.source_ids.clone(),
    };
    if force_control || node.shape() != PatternStreamShape::Event {
        LoweredPattern {
            events: None,
            controls: Some(ControlScore::query_source(source)),
        }
    } else {
        LoweredPattern {
            events: Some(Score::query_source(source)),
            controls: None,
        }
    }
}
impl LanguageSource {
    fn context(&self) -> LoweringCtx {
        LoweringCtx {
            diagnostics: Vec::new(),
            instrument: self.instrument.clone(),
            kit: self.kit,
            sound_intents: self.sound_intents.clone(),
            kit_sounds: self.kit_sounds.clone(),
            source_ids: self.sources.clone(),
        }
    }
    fn span(window: &Span) -> CycleSpan {
        CycleSpan::new(
            CycleTime(Rational::new(
                window.start().numerator(),
                window.start().denominator(),
            )),
            CycleDuration(Rational::new(
                (window.end() - window.start()).numerator(),
                (window.end() - window.start()).denominator(),
            )),
        )
    }
    fn failure(ctx: &LoweringCtx) -> Result<(), ControlModelError> {
        if ctx.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(ControlModelError::QuerySource(format!(
                "{:?}",
                ctx.diagnostics
            )))
        }
    }
}
impl ScoreQuerySource for LanguageSource {
    fn query(&self, window: &Span) -> Result<Vec<QueryMoment>, ControlModelError> {
        let stream = self.node.query(Self::span(window));
        let mut ctx = self.context();
        let mut result = Vec::new();
        let mut occurrences = BTreeMap::<String, u64>::new();
        for event in stream.events {
            if event.value.is_rest() {
                continue;
            }
            let identity =
                serde_json::to_string(&(&event.source, &event.value, event.span, event.position))
                    .expect("event identity serializes");
            let ordinal = occurrences.entry(identity.clone()).or_default();
            let mut key = 0xCBF2_9CE4_8422_2325u64;
            for byte in identity.bytes().chain(ordinal.to_le_bytes()) {
                key = (key ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
            }
            *ordinal += 1;
            let mut normalized = event.clone();
            normalized.span =
                CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()));
            let mut controls = Vec::new();
            let Some(local) = lower_pattern_event(&normalized, &mut ctx, &mut controls) else {
                continue;
            };
            let Some(span) = Span::new(
                rational_to_time(event.span.start.0),
                rational_to_time(event.span.end().0),
            ) else {
                continue;
            };
            let mut moment =
                Moment::new(span, local.intent().clone()).with_position(local.position());
            if let Some(id) = local.id() {
                moment = moment.with_id(id);
            }
            if let Some(value) = local.value_identity() {
                moment = moment.with_value_identity(value.clone());
            }
            result.push(QueryMoment {
                moment,
                instance_key: key,
                controls: controls
                    .into_iter()
                    .map(|tile| (tile.key().clone(), tile.value().clone()))
                    .collect(),
            });
        }
        Self::failure(&ctx)?;
        Ok(result)
    }
    fn estimated_work(&self, width: f64) -> f64 {
        work(&self.node, width, 0, &mut 0)
    }
    fn extent(&self) -> Time {
        rational_to_time(self.node.duration().0)
    }
}
impl ControlQuerySource for LanguageSource {
    fn query(&self, window: &Span) -> Result<Vec<QueryControl>, ControlModelError> {
        let mut ctx = self.context();
        let mut result = Vec::new();
        for event in self.node.query(Self::span(window)).events {
            if event.value.is_rest() {
                continue;
            }
            let Some(span) = Span::new(
                rational_to_time(event.span.start.0),
                rational_to_time(event.span.end().0),
            ) else {
                continue;
            };
            if event.fields.is_empty() {
                let active = match event.value {
                    EventValue::Scalar { value } => value > Rational::zero(),
                    _ => true,
                };
                result.push(QueryControl {
                    span,
                    key: ControlKey::Gate,
                    value: ControlValue::Bool(active),
                });
            } else {
                for field in &event.fields {
                    let lowered = if let EventField::Custom { key, value } = field {
                        // ControlStream's pattern view stores the canonical lane
                        // name as a Custom field; recover the actual typed lane.
                        let ir = ParameterKey::ALL
                            .iter()
                            .filter_map(|key| key.control_key())
                            .chain([ControlKeyIr::Pitch])
                            .find(|candidate| candidate.as_str() == key)
                            .unwrap_or_else(|| ControlKeyIr::Custom(key.clone()));
                        control::lower_control_key(&ir, &mut ctx).and_then(|key| {
                            control::lower_field_value(&key, value, &mut ctx)
                                .map(|value| (key, value))
                        })
                    } else {
                        control::lower_event_field(field, &mut ctx)
                    };
                    if let Some((key, value)) = lowered {
                        result.push(QueryControl { span, key, value });
                    }
                }
            }
        }
        Self::failure(&ctx)?;
        Ok(result)
    }
    fn estimated_work(&self, width: f64) -> f64 {
        work(&self.node, width, 0, &mut 0)
    }
    fn extent(&self) -> Time {
        rational_to_time(self.node.duration().0)
    }
}
fn scalar(value: Rational) -> f64 {
    value.numerator as f64 / value.denominator as f64
}
// Conservative structural estimate before recursive language queries allocate.
// Flow queries both visible and complete onset cycles and can compare siblings.
fn work(node: &PatternNodeIr, width: f64, depth: usize, count: &mut usize) -> f64 {
    *count += 1;
    if depth > 64 || *count > 4096 || !width.is_finite() || width > 16_384.0 {
        return f64::INFINITY;
    }
    let next = depth + 1;
    match node {
        PatternNodeIr::EventStream(s) => s.stream.events.len() as f64,
        PatternNodeIr::CycleEventStream(s) => s
            .stream
            .events
            .iter()
            .map(|event| (width + scalar(event.span.duration.0)).ceil() + 1.0)
            .sum(),
        PatternNodeIr::ControlStream(s) => s
            .stream
            .controls
            .iter()
            .map(|event| (width + scalar(event.span.duration.0)).ceil() + 1.0)
            .sum(),
        PatternNodeIr::ScalarStream(s) => s
            .stream
            .values
            .iter()
            .map(|event| (width + scalar(event.span.duration.0)).ceil() + 1.0)
            .sum(),
        PatternNodeIr::FlowProjection {
            control, inputs, ..
        } => {
            let visible = inputs
                .iter()
                .flat_map(|input| &input.nodes)
                .map(|child| work(child, width, next, count))
                .sum::<f64>();
            let onset = inputs
                .iter()
                .flat_map(|input| &input.nodes)
                .map(|child| work(child, 1.0, next, count))
                .sum::<f64>();
            let mask = if control.kind == tessera::prelude::FlowControlKind::Mask {
                let held = inputs
                    .iter()
                    .flat_map(|input| &input.nodes)
                    .map(|child| held_duration(child, next))
                    .fold(0.0, f64::max);
                inputs
                    .iter()
                    .flat_map(|input| &input.nodes)
                    .map(|child| work(child, held, next, count))
                    .sum::<f64>()
            } else {
                0.0
            };
            // Every visible whole input can introduce an onset cycle. Each
            // cycle queries all siblings; masks additionally query the whole
            // duration of each held note. Long notes cannot bypass preflight.
            visible * (1.0 + onset * (1.0 + mask))
        }
        PatternNodeIr::Merge { children }
        | PatternNodeIr::Concat { children }
        | PatternNodeIr::CycleRoute { children }
        | PatternNodeIr::CycleSlots { children }
        | PatternNodeIr::PriorityMerge { children, .. } => children
            .iter()
            .map(|child| work(child, width.max(1.0), next, count))
            .sum(),
        PatternNodeIr::Sequence { children }
        | PatternNodeIr::WeightedChoice {
            options: children, ..
        } => children
            .iter()
            .map(|child| work(&child.node, width.max(1.0), next, count))
            .sum(),
        PatternNodeIr::Arrange { segments } => segments
            .iter()
            .map(|segment| {
                work(
                    &segment.node,
                    width.min(scalar(segment.duration)),
                    next,
                    count,
                ) * ((width / scalar(segment.duration)).ceil() + 2.0)
            })
            .sum(),
        PatternNodeIr::TimeScale { inner, factor } => {
            work(inner, width / scalar(*factor), next, count)
        }
        PatternNodeIr::Shift { inner, .. }
        | PatternNodeIr::ReflectCycle { inner }
        | PatternNodeIr::SpaceShift { inner, .. }
        | PatternNodeIr::SpaceScale { inner, .. }
        | PatternNodeIr::SpaceReflect { inner, .. }
        | PatternNodeIr::Degrade { inner, .. }
        | PatternNodeIr::Deduplicate { inner, .. } => work(inner, width, next, count),
        PatternNodeIr::MaskClip { source, mask } => {
            work(source, width, next, count).max(1.0) * work(mask, width, next, count).max(1.0)
        }
    }
}

fn held_duration(node: &PatternNodeIr, depth: usize) -> f64 {
    if depth > 64 {
        return f64::INFINITY;
    }
    let next = depth + 1;
    match node {
        PatternNodeIr::EventStream(s) | PatternNodeIr::CycleEventStream(s) => s
            .stream
            .events
            .iter()
            .map(|event| scalar(event.span.duration.0))
            .fold(0.0, f64::max),
        PatternNodeIr::ControlStream(s) => s
            .stream
            .controls
            .iter()
            .map(|event| scalar(event.span.duration.0))
            .fold(0.0, f64::max),
        PatternNodeIr::ScalarStream(s) => s
            .stream
            .values
            .iter()
            .map(|event| scalar(event.span.duration.0))
            .fold(0.0, f64::max),
        PatternNodeIr::FlowProjection { inputs, .. } => inputs
            .iter()
            .flat_map(|input| &input.nodes)
            .map(|child| held_duration(child, next))
            .fold(0.0, f64::max),
        PatternNodeIr::Merge { children }
        | PatternNodeIr::Concat { children }
        | PatternNodeIr::CycleRoute { children }
        | PatternNodeIr::CycleSlots { children }
        | PatternNodeIr::PriorityMerge { children, .. } => children
            .iter()
            .map(|child| held_duration(child, next))
            .fold(0.0, f64::max),
        PatternNodeIr::Sequence { children }
        | PatternNodeIr::WeightedChoice {
            options: children, ..
        } => children
            .iter()
            .map(|child| held_duration(&child.node, next))
            .fold(0.0, f64::max),
        PatternNodeIr::Arrange { segments } => segments
            .iter()
            .map(|segment| held_duration(&segment.node, next).min(scalar(segment.duration)))
            .fold(0.0, f64::max),
        PatternNodeIr::TimeScale { inner, factor } => held_duration(inner, next) * scalar(*factor),
        PatternNodeIr::Shift { inner, .. }
        | PatternNodeIr::ReflectCycle { inner }
        | PatternNodeIr::SpaceShift { inner, .. }
        | PatternNodeIr::SpaceScale { inner, .. }
        | PatternNodeIr::SpaceReflect { inner, .. }
        | PatternNodeIr::Degrade { inner, .. }
        | PatternNodeIr::Deduplicate { inner, .. } => held_duration(inner, next),
        PatternNodeIr::MaskClip { source, .. } => held_duration(source, next),
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn an_empty_event_source_cannot_hide_a_dense_mask_query() {
        let empty = PatternNodeIr::cycle_event_stream(Default::default());
        let mask = PatternNodeIr::cycle_event_stream(tessera::prelude::PatternStream::new(vec![
            tessera::prelude::PatternEvent::new(
                CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one())),
                EventValue::Scalar { value: Rational::one() },
            ); 9_000
        ]));
        let node = PatternNodeIr::mask_clip(empty, mask);
        assert!(work(&node, 1.0, 0, &mut 0) > 16_384.0);
    }
}
