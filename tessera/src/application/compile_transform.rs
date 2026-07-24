use std::collections::BTreeMap;

use crate::domain::{ControlEvent, ControlKeyIr, ControlStream, ControlValueIr};
use crate::domain::{
    CycleDuration, CycleSpan, CycleTime, DefaultStreamBehavior, Diagnostic, DiagnosticCategory,
    DiagnosticKind, DiagnosticLocation, EventField, EventValue, FieldValue, InputEndpoint,
    InputPort, NodeId, NormalizedProgram, OutputEndpoint, OutputPort, PatternEvent, PatternNodeIr,
    PatternStream, Rational, StreamSource, TransformKind, TransformNode,
};

use super::relations;

pub type TransformAuxInputs = Vec<(InputPort, PatternStream)>;
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
    let aux_flat: TransformAuxInputs = aux_streams
        .into_iter()
        .map(|(port, node)| (port, node.flatten()))
        .collect();
    Ok(match transform.kind {
        TransformKind::Slow => {
            let factor = scalar_factor_from_aux(node_id, &aux_flat)?;
            PatternNodeIr::time_scale(main, factor)
        }
        TransformKind::Fast => {
            let factor = scalar_factor_from_aux(node_id, &aux_flat)?;
            PatternNodeIr::time_scale(main, Rational::one() / factor)
        }
        TransformKind::Rev => PatternNodeIr::reflect_cycle(main),
        TransformKind::Gain => {
            apply_field_modulation_ir(main, &aux_flat, ControlKeyIr::Gain, EventField::Gain)
        }
        TransformKind::Attack => {
            apply_field_modulation_ir(main, &aux_flat, ControlKeyIr::Attack, EventField::Attack)
        }
        TransformKind::Transpose => apply_field_modulation_ir(
            main,
            &aux_flat,
            ControlKeyIr::Custom("transpose".to_string()),
            EventField::Transpose,
        ),
        TransformKind::Degrade => PatternNodeIr::event_stream(annotate_with_aux(
            main.flatten(),
            aux_flat,
            EventField::Degrade,
        )),
    })
}

fn apply_field_modulation_ir(
    main: PatternNodeIr,
    aux_streams: &TransformAuxInputs,
    control_key: ControlKeyIr,
    field: fn(FieldValue) -> EventField,
) -> PatternNodeIr {
    let Some((_, aux_stream)) = aux_streams.first() else {
        return main;
    };
    if is_single_scalar_literal(aux_stream) {
        return PatternNodeIr::event_stream(annotate_with_aux(
            main.flatten(),
            aux_streams.to_vec(),
            field,
        ));
    }
    PatternNodeIr::merge(vec![
        main,
        PatternNodeIr::control_stream(control_stream_from_scalars(aux_stream, control_key)),
    ])
}

fn is_single_scalar_literal(stream: &PatternStream) -> bool {
    stream.events.len() == 1 && matches!(stream.events[0].value, EventValue::Scalar { .. })
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
                    ControlValueIr::rational(value),
                )),
                _ => None,
            })
            .collect(),
    }
}

fn scalar_factor_from_aux(
    node_id: &NodeId,
    aux_streams: &TransformAuxInputs,
) -> Result<Rational, Vec<Diagnostic>> {
    let factor = aux_streams
        .first()
        .and_then(|(_, stream)| stream.events.first())
        .and_then(|event| match event.value {
            crate::domain::EventValue::Scalar { value } => Some(value),
            _ => None,
        })
        .unwrap_or_else(crate::domain::Rational::one);
    if factor <= crate::domain::Rational::zero() {
        return Err(vec![Diagnostic::new(
            DiagnosticCategory::TransformArgument,
            DiagnosticKind::InvalidTransformArgument,
            "Fast/slow factor must be greater than zero.",
            Some(DiagnosticLocation::RootNode(node_id.clone())),
        )]);
    }
    Ok(factor)
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

fn annotate_with_aux(
    mut stream: PatternStream,
    aux_streams: TransformAuxInputs,
    constructor: fn(FieldValue) -> EventField,
) -> PatternStream {
    let field = aux_streams
        .first()
        .and_then(|(_, stream)| stream.events.first())
        .and_then(|event| match event.value {
            crate::domain::EventValue::Scalar { value } => {
                Some(constructor(FieldValue::Rational { value }))
            }
            _ => None,
        });
    if let Some(field) = field {
        for event in &mut stream.events {
            event.fields.push(field.clone());
        }
    }
    stream
}
