use std::collections::BTreeMap;

use crate::domain::{ControlEvent, ControlKeyIr, ControlStream, ControlValueIr};
use crate::domain::{
    CycleDuration, CycleSpan, CycleTime, DefaultStreamBehavior, Diagnostic, DiagnosticCategory,
    DiagnosticKind, DiagnosticLocation, EventField, EventValue, FieldValue, InputEndpoint,
    InputPort, NodeId, NormalizedProgram, OutputEndpoint, OutputPort, PatternEvent, PatternNodeIr,
    PatternStream, Rational, StreamSource, TransformKind, TransformNode,
};

use super::relations;

pub type NodeOutputsIr = BTreeMap<OutputEndpoint, PatternNodeIr>;

pub fn compile_transform_node_ir<F>(
    program: &NormalizedProgram,
    node_id: &NodeId,
    transform: &TransformNode,
    mut resolve_source: F,
) -> Result<NodeOutputsIr, Vec<Diagnostic>>
where
    F: FnMut(&StreamSource) -> Result<PatternNodeIr, Vec<Diagnostic>>,
{
    let mut main_ir = None;
    let mut aux_streams = Vec::new();
    for socket in &transform.signature.input_sockets {
        let sources = relations::incoming_socket_sources_normalized(program, node_id, &socket.port);
        if sources.len() > 1 {
            return Err(vec![Diagnostic::new(
                DiagnosticCategory::TransformTopology,
                DiagnosticKind::OptionalSocketMultiplyBound,
                "Input socket received more than one binding.",
                Some(DiagnosticLocation::InputEndpoint {
                    node: node_id.clone(),
                    endpoint: InputEndpoint::Socket(socket.port.clone()),
                }),
            )]);
        }
        let node = if let Some(source) = sources.first() {
            Some(resolve_source(source)?)
        } else {
            socket
                .default
                .as_ref()
                .map(|default| PatternNodeIr::event_stream(default_stream(default)))
        };
        match socket.role {
            crate::domain::NodeInputRole::Main => main_ir = node,
            _ => {
                if let Some(node) = node {
                    aux_streams.push((socket.port.clone(), node));
                }
            }
        }
    }
    let Some(main_ir) = main_ir else {
        return Err(vec![Diagnostic::new(
            DiagnosticCategory::RootRelation,
            DiagnosticKind::TransformMissingMainInput,
            "Transform main input could not be resolved during compilation.",
            Some(DiagnosticLocation::RootNode(node_id.clone())),
        )]);
    };
    let node = apply_transform_ir(node_id, main_ir, aux_streams, transform)?;
    Ok(BTreeMap::from_iter([(
        OutputEndpoint::Socket(OutputPort::new("out")),
        node,
    )]))
}

fn apply_transform_ir(
    node_id: &NodeId,
    main: PatternNodeIr,
    aux_streams: Vec<(InputPort, PatternNodeIr)>,
    transform: &TransformNode,
) -> Result<PatternNodeIr, Vec<Diagnostic>> {
    if let Some(parameter) = transform.kind.parameter_key() {
        validate_parameter_inputs(node_id, parameter, &aux_streams)?;
    }
    let factor = || {
        aux_streams.first().and_then(|(_, node)| constant_scalar(node)).ok_or_else(|| vec![Diagnostic::new(
            DiagnosticCategory::TransformArgument, DiagnosticKind::InvalidTransformArgument,
            "This transform requires a constant scalar operand; a changing pattern is not a literal.",
            Some(DiagnosticLocation::RootNode(node_id.clone())),
        )])
    };
    Ok(match transform.kind {
        TransformKind::Trick => unreachable!("tricks compile in their own argument scope"),
        TransformKind::Instrument => main.map_event_leaves(&mut |stream, periodic| {
            let mut stream = stream.clone();
            for event in &mut stream.events {
                event.source.get_or_insert_with(Default::default).instrument =
                    Some(node_id.clone());
            }
            if periodic {
                PatternNodeIr::cycle_event_stream(stream)
            } else {
                PatternNodeIr::event_stream(stream)
            }
        }),
        TransformKind::Wire => main,
        TransformKind::Delay | TransformKind::Reverb | TransformKind::Compressor => {
            match aux_streams.first() {
                Some((_, controls)) => PatternNodeIr::merge(vec![main, controls.clone()]),
                None => main,
            }
        }
        TransformKind::Slow | TransformKind::Fast => {
            let value = factor()?;
            if value <= Rational::zero() {
                return Err(vec![Diagnostic::new(
                    DiagnosticCategory::TransformArgument,
                    DiagnosticKind::InvalidTransformArgument,
                    "Fast/slow factor must be greater than zero.",
                    Some(DiagnosticLocation::RootNode(node_id.clone())),
                )]);
            }
            PatternNodeIr::time_scale(
                main,
                if transform.kind == TransformKind::Fast {
                    Rational::one() / value
                } else {
                    value
                },
            )
        }
        TransformKind::Rev => PatternNodeIr::reflect_cycle(main),
        TransformKind::Gain => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Gain, EventField::Gain)
        }
        TransformKind::Attack => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Attack, EventField::Attack)
        }
        TransformKind::Decay => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Decay, EventField::Decay)
        }
        TransformKind::Release => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::Release,
            EventField::Release,
        ),
        TransformKind::Pan => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Pan, EventField::Pan)
        }
        TransformKind::HighPassCutoff => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::HighPassCutoff,
            EventField::HighPassCutoff,
        ),
        TransformKind::HighPassResonance => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::HighPassResonance,
            EventField::HighPassResonance,
        ),
        TransformKind::Velocity => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::Velocity,
            EventField::Velocity,
        ),
        TransformKind::ClipLength => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::ClipLength,
            EventField::ClipLength,
        ),
        TransformKind::PostGain => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::PostGain,
            EventField::PostGain,
        ),
        TransformKind::PitchBend => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::PitchBend,
            EventField::PitchBend,
        ),
        TransformKind::Expression => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::Expression,
            EventField::Expression,
        ),
        TransformKind::PlaybackRate => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::PlaybackRate,
            EventField::PlaybackRate,
        ),
        TransformKind::PlaybackStart => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::PlaybackStart,
            EventField::PlaybackStart,
        ),
        TransformKind::PlaybackEnd => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::PlaybackEnd,
            EventField::PlaybackEnd,
        ),
        TransformKind::Reverse => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::Reverse,
            EventField::Reverse,
        ),
        TransformKind::Fit => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Fit, EventField::Fit)
        }
        TransformKind::Loop => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Loop, EventField::Loop)
        }
        TransformKind::Transpose => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::Transpose,
            EventField::Transpose,
        ),
        TransformKind::Gate => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Gate, EventField::Gate)
        }
        TransformKind::Legato => {
            apply_field_modulation_ir(main, &aux_streams, ControlKeyIr::Legato, EventField::Legato)
        }
        TransformKind::Sustain => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::Sustain,
            EventField::Sustain,
        ),
        TransformKind::LowPassCutoff => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::LowPassCutoff,
            EventField::LowPassCutoff,
        ),
        TransformKind::LowPassResonance => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::LowPassResonance,
            EventField::LowPassResonance,
        ),
        TransformKind::SampleVariant => apply_field_modulation_ir(
            main,
            &aux_streams,
            ControlKeyIr::SampleVariant,
            EventField::SampleVariant,
        ),
        TransformKind::Degrade => {
            let probability = factor()?;
            if probability < Rational::zero() || probability > Rational::one() {
                return Err(vec![Diagnostic::new(
                    DiagnosticCategory::TransformArgument,
                    DiagnosticKind::InvalidTransformArgument,
                    "Degrade probability must be within zero and one.",
                    Some(DiagnosticLocation::RootNode(node_id.clone())),
                )]);
            }
            PatternNodeIr::degrade(main, Rational::one() - probability, 0)
        }
    })
}

fn validate_parameter_inputs(
    node_id: &NodeId,
    key: crate::domain::ParameterKey,
    inputs: &[(InputPort, PatternNodeIr)],
) -> Result<(), Vec<Diagnostic>> {
    let spec = key.spec();
    let Some((_, input)) = inputs.first() else {
        return Ok(());
    };
    let mut reasons = Vec::new();
    validate_parameter_tree(input, key, &mut reasons);
    if !spec.accepts_pattern && constant_scalar(input).is_none() {
        reasons.push("This parameter currently requires a constant operand.");
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(reasons
            .into_iter()
            .map(|reason| {
                Diagnostic::new(
                    DiagnosticCategory::TransformArgument,
                    DiagnosticKind::InvalidTransformArgument,
                    format!("{}: {reason}", spec.label),
                    Some(DiagnosticLocation::RootNode(node_id.clone())),
                )
            })
            .collect())
    }
}

fn validate_parameter_tree(
    node: &PatternNodeIr,
    parameter: crate::domain::ParameterKey,
    reasons: &mut Vec<&'static str>,
) {
    use PatternNodeIr as N;
    match node {
        N::FlowProjection {
            control, inputs, ..
        } => {
            for input in inputs {
                let musical = match &input.endpoint {
                    crate::domain::InputEndpoint::GroupMember { .. } => true,
                    crate::domain::InputEndpoint::Socket(port) => control
                        .signature
                        .input_socket(port)
                        .is_some_and(|socket| socket.role == crate::domain::NodeInputRole::Main),
                };
                if musical {
                    for node in &input.nodes {
                        validate_parameter_tree(node, parameter, reasons);
                    }
                }
            }
        }
        N::ControlStream(stream) => {
            for control in &stream.stream.controls {
                let value = control.value.clone().into_field_value();
                if Some(control.key.clone()) != parameter.control_key() {
                    reasons.push("The control values must match the transform.");
                }
                if let Err(reason) = parameter.spec().validate(&value) {
                    reasons.push(reason);
                }
            }
        }
        N::EventStream(stream) | N::CycleEventStream(stream) => {
            for event in &stream.stream.events {
                match event.value {
                    EventValue::Rest => {}
                    EventValue::Scalar { value } => {
                        if let Err(reason) = parameter.spec().validate(&FieldValue::rational(value))
                        {
                            reasons.push(reason);
                        }
                    }
                    _ => reasons.push("Use values matching this parameter, or rests."),
                }
            }
        }
        N::ScalarStream(stream) => {
            for scalar in &stream.stream.values {
                if let Err(reason) = parameter
                    .spec()
                    .validate(&FieldValue::rational(scalar.value))
                {
                    reasons.push(reason);
                }
            }
        }
        N::Merge { children }
        | N::CycleRoute { children }
        | N::CycleSlots { children }
        | N::Concat { children }
        | N::PriorityMerge { children, .. } => {
            for child in children {
                validate_parameter_tree(child, parameter, reasons);
            }
        }
        N::Sequence { children }
        | N::WeightedChoice {
            options: children, ..
        } => {
            for child in children {
                validate_parameter_tree(&child.node, parameter, reasons);
            }
        }
        N::Arrange { segments } => {
            for segment in segments {
                validate_parameter_tree(&segment.node, parameter, reasons);
            }
        }
        N::TimeScale { inner, .. }
        | N::Shift { inner, .. }
        | N::ReflectCycle { inner }
        | N::SpaceShift { inner, .. }
        | N::SpaceScale { inner, .. }
        | N::SpaceReflect { inner, .. }
        | N::Degrade { inner, .. }
        | N::Deduplicate { inner, .. } => validate_parameter_tree(inner, parameter, reasons),
        N::MaskClip { source, .. } => validate_parameter_tree(source, parameter, reasons),
    }
}

fn constant_scalar(node: &PatternNodeIr) -> Option<Rational> {
    match node {
        PatternNodeIr::ScalarStream(node) if node.stream.values.len() == 1 => {
            Some(node.stream.values[0].value)
        }
        PatternNodeIr::EventStream(node) | PatternNodeIr::CycleEventStream(node)
            if node.stream.events.len() == 1 =>
        {
            match node.stream.events[0].value {
                EventValue::Scalar { value } => Some(value),
                _ => None,
            }
        }
        PatternNodeIr::Sequence { children } if children.len() == 1 => {
            constant_scalar(&children[0].node)
        }
        PatternNodeIr::SpaceShift { inner, .. }
        | PatternNodeIr::SpaceScale { inner, .. }
        | PatternNodeIr::SpaceReflect { inner, .. } => constant_scalar(inner),
        _ => None,
    }
}

fn apply_field_modulation_ir(
    main: PatternNodeIr,
    aux_streams: &[(InputPort, PatternNodeIr)],
    control_key: ControlKeyIr,
    field: fn(FieldValue) -> EventField,
) -> PatternNodeIr {
    let Some((_, auxiliary)) = aux_streams.first() else {
        return main;
    };
    if let Some(value) = constant_scalar(auxiliary).filter(|_| !contains_flow_projection(&main)) {
        return main.map_event_leaves(&mut |stream, periodic| {
            let mut stream = stream.clone();
            for event in &mut stream.events {
                event
                    .fields
                    .push(field(if control_key == ControlKeyIr::Gate {
                        FieldValue::bool(!value.is_zero())
                    } else {
                        FieldValue::rational(value)
                    }));
            }
            if periodic {
                PatternNodeIr::cycle_event_stream(stream)
            } else {
                PatternNodeIr::event_stream(stream)
            }
        });
    }
    let controls = auxiliary.map_event_leaves(&mut |stream, _| {
        PatternNodeIr::control_stream(control_stream_from_scalars(stream, control_key.clone()))
    });
    PatternNodeIr::merge(vec![main, controls])
}

fn contains_flow_projection(node: &PatternNodeIr) -> bool {
    use PatternNodeIr as N;
    match node {
        N::FlowProjection { .. } => true,
        N::EventStream(_) | N::CycleEventStream(_) | N::ControlStream(_) | N::ScalarStream(_) => {
            false
        }
        N::Merge { children }
        | N::CycleRoute { children }
        | N::CycleSlots { children }
        | N::Concat { children }
        | N::PriorityMerge { children, .. } => children.iter().any(contains_flow_projection),
        N::Sequence { children }
        | N::WeightedChoice {
            options: children, ..
        } => children
            .iter()
            .any(|child| contains_flow_projection(&child.node)),
        N::Arrange { segments } => segments
            .iter()
            .any(|segment| contains_flow_projection(&segment.node)),
        N::TimeScale { inner, .. }
        | N::Shift { inner, .. }
        | N::ReflectCycle { inner }
        | N::SpaceShift { inner, .. }
        | N::SpaceScale { inner, .. }
        | N::SpaceReflect { inner, .. }
        | N::Degrade { inner, .. }
        | N::Deduplicate { inner, .. } => contains_flow_projection(inner),
        N::MaskClip { source, mask } => {
            contains_flow_projection(source) || contains_flow_projection(mask)
        }
    }
}

fn control_stream_from_scalars(stream: &PatternStream, key: ControlKeyIr) -> ControlStream {
    ControlStream {
        controls: stream
            .events
            .iter()
            .filter_map(|event| match event.value {
                EventValue::Scalar { value } => Some(ControlEvent::new(
                    event.span,
                    key.clone(),
                    if key == ControlKeyIr::Gate {
                        ControlValueIr::bool(!value.is_zero())
                    } else {
                        ControlValueIr::rational(value)
                    },
                )),
                _ => None,
            })
            .collect(),
    }
}

pub(crate) fn default_stream(default: &DefaultStreamBehavior) -> PatternStream {
    match default {
        DefaultStreamBehavior::ConstantScalar { value } => PatternStream {
            events: vec![PatternEvent {
                span: CycleSpan {
                    start: CycleTime(crate::domain::Rational::zero()),
                    duration: CycleDuration(crate::domain::Rational::one()),
                },
                value: crate::domain::EventValue::Scalar { value: *value },
                fields: Vec::new(),
                position: crate::domain::SpatialMotionIr::origin(),
                source: None,
            }],
        },
    }
}
