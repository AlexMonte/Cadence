use crate::domain::{
    AtomExpr, AtomExprKind, MusicalValue, NodeId, NormalizedContainer, NormalizedProgram,
    OutputEndpoint, RootSurfaceNodeKind, StreamShape,
};
use std::collections::BTreeSet;

pub fn stream_shape_compatible(source: StreamShape, target: StreamShape) -> bool {
    matches!(target, StreamShape::Any)
        || matches!(source, StreamShape::Any)
        || source == target
        || matches!(
            (source, target),
            (StreamShape::NotePattern, StreamShape::EventPattern)
        )
        || matches!(
            (source, target),
            (StreamShape::ScalarPattern, StreamShape::ControlPattern)
        )
}

pub fn infer_normalized_container_shape(
    program: &NormalizedProgram,
    container: &NormalizedContainer,
) -> StreamShape {
    let expr_shapes = container
        .exprs
        .iter()
        .map(|expr| infer_expr_shape(program, expr, &mut BTreeSet::new()))
        .collect::<Vec<_>>();
    merge_expr_shapes(expr_shapes)
}

pub fn normalized_node_output_shape(
    program: &NormalizedProgram,
    node_id: &NodeId,
    endpoint: &OutputEndpoint,
    visiting: &mut BTreeSet<NodeId>,
) -> StreamShape {
    if !visiting.insert(node_id.clone()) {
        return StreamShape::Any;
    }
    let shape = match program.root_nodes.get(node_id) {
        Some(RootSurfaceNodeKind::Scalar(_)) => StreamShape::ScalarPattern,
        Some(RootSurfaceNodeKind::Container { container }) => program
            .containers
            .get(container)
            .map(|container| infer_normalized_container_shape(program, container))
            .unwrap_or(StreamShape::Any),
        Some(RootSurfaceNodeKind::Transform(transform))
            if transform.kind == crate::domain::TransformKind::Wire =>
        {
            let sources = super::relations::incoming_socket_sources_normalized(
                program,
                node_id,
                &crate::domain::InputPort::new("main"),
            );
            sources
                .first()
                .map(|source| {
                    normalized_node_output_shape(program, &source.node, &source.endpoint, visiting)
                })
                .unwrap_or(StreamShape::Any)
        }
        Some(RootSurfaceNodeKind::Transform(transform)) => match endpoint {
            OutputEndpoint::Socket(port) => transform
                .signature
                .output_socket(port)
                .map(|spec| spec.shape)
                .unwrap_or(StreamShape::Any),
            OutputEndpoint::GroupMember { group, .. } => transform
                .signature
                .output_group(group)
                .map(|spec| spec.shape)
                .unwrap_or(StreamShape::Any),
        },
        Some(RootSurfaceNodeKind::FlowControl(control)) => match endpoint {
            OutputEndpoint::Socket(port) => control
                .signature
                .output_socket(port)
                .map(|spec| spec.shape)
                .unwrap_or(StreamShape::Any),
            OutputEndpoint::GroupMember { group, .. } => control
                .signature
                .output_group(group)
                .map(|spec| spec.shape)
                .unwrap_or(StreamShape::Any),
        },
        Some(RootSurfaceNodeKind::Output(_)) | None => StreamShape::Any,
    };
    visiting.remove(node_id);
    shape
}

fn infer_expr_shape(
    program: &NormalizedProgram,
    expr: &AtomExpr,
    visiting: &mut BTreeSet<NodeId>,
) -> StreamShape {
    let shape = match &expr.kind {
        AtomExprKind::Value(value) => infer_value_shape(program, value, visiting),
        AtomExprKind::Choice(options) | AtomExprKind::Parallel(options) => merge_expr_shapes(
            options
                .iter()
                .map(|expr| infer_expr_shape(program, expr, visiting))
                .collect(),
        ),
    };
    if shape == StreamShape::ScalarPattern
        && expr
            .modifiers
            .iter()
            .any(|modifier| matches!(modifier, crate::domain::AtomModifier::Scale(_)))
    {
        StreamShape::NotePattern
    } else {
        shape
    }
}

fn infer_value_shape(
    program: &NormalizedProgram,
    value: &MusicalValue,
    visiting: &mut BTreeSet<NodeId>,
) -> StreamShape {
    match value {
        MusicalValue::Note(_) => StreamShape::NotePattern,
        MusicalValue::Sound(_) => StreamShape::EventPattern,
        MusicalValue::Rest => StreamShape::Any,
        MusicalValue::Effect(_) => StreamShape::ControlPattern,
        MusicalValue::Scalar(_) => StreamShape::ScalarPattern,
        MusicalValue::NestedContainer(container_id) => program
            .containers
            .get(container_id)
            .map(|container| infer_normalized_container_shape(program, container))
            .unwrap_or_else(|| {
                let _ = visiting;
                StreamShape::Any
            }),
    }
}

fn merge_expr_shapes(shapes: Vec<StreamShape>) -> StreamShape {
    // Rests carry timing but do not change the type of surrounding values.
    let shapes: Vec<_> = shapes
        .into_iter()
        .filter(|shape| *shape != StreamShape::Any)
        .collect();
    if shapes.is_empty() {
        return StreamShape::Any;
    }
    if shapes
        .iter()
        .all(|shape| matches!(shape, StreamShape::ControlPattern))
    {
        return StreamShape::ControlPattern;
    }
    if shapes
        .iter()
        .all(|shape| matches!(shape, StreamShape::ScalarPattern))
    {
        return StreamShape::ScalarPattern;
    }
    if shapes
        .iter()
        .all(|shape| matches!(shape, StreamShape::NotePattern))
    {
        return StreamShape::NotePattern;
    }
    StreamShape::EventPattern
}
