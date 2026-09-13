//! Flow policies are applied to complete onset cycles, then clipped by visibility.
//! A held note is always routed using its original onset, including after a seek.
use super::{
    CycleDuration, CycleSpan, CycleTime, EventField, EventValue, FieldValue, FlowControlKind as K,
    FlowControlNode, FlowControlPolicy as P, FlowInputIr, InputEndpoint, InputPort, OutputEndpoint,
    PatternEvent, PatternStream, PortGroupId, Rational,
};
use std::collections::{BTreeMap, BTreeSet};

type Streams = BTreeMap<InputEndpoint, PatternStream>;
fn floor(value: Rational) -> i64 {
    value.numerator.div_euclid(value.denominator)
}
fn intersects(a: CycleSpan, b: CycleSpan) -> bool {
    a.start < b.end() && b.start < a.end()
}
fn unit(cycle: i64) -> CycleSpan {
    CycleSpan::new(
        CycleTime(Rational::from_integer(cycle)),
        CycleDuration(Rational::one()),
    )
}
fn input_streams(inputs: &[FlowInputIr], span: CycleSpan) -> Streams {
    inputs
        .iter()
        .map(|input| {
            (
                input.endpoint.clone(),
                PatternStream::layer(input.nodes.iter().map(|node| node.query(span)).collect()),
            )
        })
        .collect()
}
fn socket<'a>(inputs: &'a Streams, name: &str) -> Option<&'a PatternStream> {
    inputs.get(&InputEndpoint::Socket(InputPort::new(name)))
}
fn group(
    inputs: &Streams,
    control: &FlowControlNode,
    name: &str,
    cycle: i64,
) -> Vec<PatternStream> {
    control
        .members
        .inputs
        .get(&PortGroupId::new(name))
        .into_iter()
        .flatten()
        .map(|member| {
            let stream = inputs
                .get(&InputEndpoint::GroupMember {
                    group: PortGroupId::new(name),
                    member: member.clone(),
                })
                .cloned()
                .unwrap_or_default();
            onsets(stream, cycle)
        })
        .collect()
}
fn onsets(mut stream: PatternStream, cycle: i64) -> PatternStream {
    stream
        .events
        .retain(|event| floor(event.span.start.0) == cycle);
    stream
        .events
        .sort_by_key(|event| (event.span.start, event.span.end()));
    stream
}
fn scalar_at(stream: Option<&PatternStream>, time: Rational) -> Option<Rational> {
    stream?.events.iter().find_map(|event| {
        if time < event.span.start.0 || time >= event.span.end().0 {
            return None;
        }
        match event.value {
            EventValue::Scalar { value } => Some(value),
            EventValue::Rest => None,
            _ => Some(Rational::one()),
        }
    })
}
fn active_at(stream: Option<&PatternStream>, time: Rational) -> bool {
    scalar_at(stream, time).is_some_and(|value| value > Rational::zero())
}
fn index(value: Rational, len: usize) -> usize {
    if len == 0 || value <= Rational::zero() {
        0
    } else {
        (value.numerator.div_euclid(value.denominator) as u64 % len as u64) as usize
    }
}
fn random_index(cycle: i64, len: usize) -> usize {
    let mut seed = (cycle as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    if len == 0 {
        0
    } else {
        ((seed ^ (seed >> 31)) % len as u64) as usize
    }
}
fn weighted_index(value: Rational, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let total = (len as u128) * (len as u128 + 1) / 2;
    let position = ((value.numerator as i128).unsigned_abs() / (value.denominator as u128)) % total;
    let mut cursor = 0u128;
    for choice in 0..len {
        cursor += choice as u128 + 1;
        if position < cursor {
            return choice;
        }
    }
    len - 1
}
fn output_member<'a>(
    output: &'a OutputEndpoint,
    control: &FlowControlNode,
) -> (&'a str, usize, usize) {
    match output {
        OutputEndpoint::GroupMember { group, member } => {
            let members = control.members.outputs.get(group);
            (
                &member.0,
                members
                    .and_then(|m| m.iter().position(|m| m == member))
                    .unwrap_or(0),
                members.map_or(1, Vec::len).max(1),
            )
        }
        _ => ("", 0, 1),
    }
}
fn gain(event: &mut PatternEvent, value: Rational) {
    event
        .fields
        .push(EventField::Gain(FieldValue::rational(value)));
}
fn average_gain(stream: &PatternStream) -> Option<Rational> {
    let values = stream
        .events
        .iter()
        .filter_map(|event| {
            event.fields.iter().rev().find_map(|field| match field {
                EventField::Gain(FieldValue::Rational { value }) => Some(*value),
                _ => None,
            })
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(
            values.iter().copied().fold(Rational::zero(), |a, b| a + b)
                / Rational::from_integer(stream.events.len() as i64),
        )
    }
}
fn field_label(event: &PatternEvent, label: &str) -> bool {
    if label == "plain" {
        return event.fields.is_empty();
    }
    event.fields.iter().any(|field| {
        let name = match field {
            EventField::Custom { key, .. } => return key == label,
            EventField::Gate(_) => "gate",
            EventField::Legato(_) => "legato",
            EventField::SampleBank(_) => "sample_bank",
            EventField::SampleVariant(_) => "sample_variant",
            EventField::Gain(_) => "gain",
            EventField::PostGain(_) => "post_gain",
            EventField::Pan(_) => "pan",
            EventField::Expression(_) => "expression",
            EventField::ClipLength(_) => "clip_length",
            EventField::Velocity(_) => "velocity",
            EventField::Pitch(_) => "pitch",
            EventField::PitchBend(_) => "pitch_bend",
            EventField::PlaybackRate(_) => "playback_rate",
            EventField::PlaybackStart(_) => "playback_start",
            EventField::PlaybackEnd(_) => "playback_end",
            EventField::Reverse(_) => "reverse",
            EventField::Fit(_) => "fit",
            EventField::Loop(_) => "loop",
            EventField::Slice(_) => "slice",
            EventField::Attack(_) => "attack",
            EventField::Decay(_) => "decay",
            EventField::Sustain(_) => "sustain",
            EventField::Release(_) => "release",
            EventField::LowPassCutoff(_) => "low_pass_cutoff",
            EventField::LowPassResonance(_) => "low_pass_resonance",
            EventField::HighPassCutoff(_) => "high_pass_cutoff",
            EventField::HighPassResonance(_) => "high_pass_resonance",
            EventField::ReverbSend(_) => "reverb_send",
            EventField::DelaySend(_) => "delay_send",
            EventField::Compressor(_) => "compressor",
            EventField::Select(_) => "select",
            EventField::Elongate(_) => "elongate",
            EventField::Replicate(_) => "replicate",
            EventField::Degrade(_) => "degrade",
            EventField::RandomChoice => "random_choice",
            EventField::Transpose(_) => "transpose",
        };
        name == label
    })
}

pub(super) fn query(
    control: &FlowControlNode,
    inputs: &[FlowInputIr],
    output: &OutputEndpoint,
    span: CycleSpan,
) -> PatternStream {
    // Query intersecting whole inputs first, so long notes reveal the onset cycles
    // needed for their routing decisions without scanning from time zero.
    let visible = input_streams(inputs, span);
    let cycles = visible
        .values()
        .flat_map(|stream| stream.events.iter().map(|event| floor(event.span.start.0)))
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for cycle in cycles {
        let streams = input_streams(inputs, unit(cycle));
        let main = onsets(socket(&streams, "main").cloned().unwrap_or_default(), cycle);
        let (member, member_index, member_count) = output_member(output, control);
        let produced = match control.kind {
            K::Split | K::Route => {
                let control_stream = socket(&streams, "control");
                PatternStream::new(main.events.into_iter().enumerate().filter_map(|(event_index,event)| {
                    let keep=match &control.policy {
                        P::SplitCopyToAll=>true,
                        P::SplitByIndexModulo|P::RouteByIndexModulo=>event_index%member_count==member_index,
                        P::SplitByEventField|P::RouteByEventField=>field_label(&event,member),
                        P::SplitByPitchRange {threshold_octave}=> {
                            let high=matches!(event.value,EventValue::Note {octave:Some(octave),..} if octave>=*threshold_octave);
                            let want_high=matches!(member,"high"|"upper"|"even") || (member_index==0 && !matches!(member,"low"|"lower"|"odd"));
                            high==want_high
                        }
                        P::RouteByLabel=>matches!(&event.value,EventValue::Note {value,..} | EventValue::Sound {value} if value==member),
                        P::RouteByControlValue=>index(scalar_at(control_stream,event.span.start.0).unwrap_or_else(Rational::zero),member_count)==member_index,
                        _=>false,
                    };keep.then_some(event)
                }).collect())
            }
            K::Mask => {
                let mut output = Vec::new();
                for mut event in main.events {
                    // The mask can change inside a held event. Query its whole
                    // span for clipping and its original onset for gate/scale.
                    let mask_inputs = input_streams(inputs, event.span);
                    let mask = socket(&mask_inputs, "mask");
                    match control.policy {
                        P::MaskGate if active_at(mask, event.span.start.0) => output.push(event),
                        P::MaskInvertGate if !active_at(mask, event.span.start.0) => {
                            output.push(event)
                        }
                        P::MaskScale => {
                            if let Some(value) = scalar_at(mask, event.span.start.0) {
                                gain(&mut event, value);
                            }
                            output.push(event);
                        }
                        P::MaskClip => {
                            for mask in mask.into_iter().flat_map(|s| &s.events) {
                                if !matches!(mask.value,EventValue::Scalar {value} if value>Rational::zero())
                                {
                                    continue;
                                }
                                let start = event.span.start.max(mask.span.start);
                                let end = event.span.end().min(mask.span.end());
                                if start < end {
                                    let mut clipped = event.clone();
                                    clipped.span =
                                        CycleSpan::new(start, CycleDuration(end.0 - start.0));
                                    output.push(clipped);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                PatternStream::new(output)
            }
            K::Switch | K::Choice => {
                let options = group(
                    &streams,
                    control,
                    if control.kind == K::Switch {
                        "candidates"
                    } else {
                        "options"
                    },
                    cycle,
                );
                let count = options.len();
                let control_stream = socket(&streams, "control");
                PatternStream::new(
                    options
                        .into_iter()
                        .enumerate()
                        .flat_map(|(option, stream)| {
                            stream.events.into_iter().filter(move |event| {
                                let selected = match control.policy {
                                    P::SwitchCycleIndex | P::ChoiceCycle => {
                                        cycle.rem_euclid(count.max(1) as i64) as usize
                                    }
                                    P::SwitchSeededRandom | P::ChoiceSeededRandom => {
                                        random_index(cycle, count)
                                    }
                                    P::ChoiceWeighted => weighted_index(
                                        scalar_at(control_stream, event.span.start.0)
                                            .unwrap_or_else(Rational::one),
                                        count,
                                    ),
                                    _ => index(
                                        scalar_at(control_stream, event.span.start.0)
                                            .unwrap_or_else(Rational::zero),
                                        count,
                                    ),
                                };
                                option == selected
                            })
                        })
                        .collect(),
                )
            }
            K::Mix => {
                let mut layered = PatternStream::layer(group(&streams, control, "streams", cycle));
                let average = average_gain(&layered);
                for event in &mut layered.events {
                    let amount = scalar_at(socket(&streams, "amount"), event.span.start.0)
                        .unwrap_or_else(Rational::one);
                    match control.policy {
                        P::MixFieldBlend
                            if !event
                                .fields
                                .iter()
                                .any(|f| matches!(f, EventField::Gain(_))) =>
                        {
                            gain(event, average.unwrap_or_else(Rational::one))
                        }
                        P::MixWeighted => gain(event, amount),
                        P::MixGainAverage => gain(event, average.unwrap_or(amount)),
                        _ => {}
                    }
                }
                layered
            }
            // Native structural IR owns these policies; the projection is not
            // constructed for Layer or Merge by the compiler.
            K::Layer | K::Merge => PatternStream::layer(group(&streams, control, "streams", cycle)),
        };
        result.extend(
            produced
                .events
                .into_iter()
                .filter(|event| intersects(event.span, span)),
        );
    }
    result.sort_by_key(|event| (event.span.start, event.span.end()));
    PatternStream::new(result)
}
