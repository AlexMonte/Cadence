use std::collections::{BTreeMap, BTreeSet};

use crate::application::{
    compile_container_ir, compile_context::CompileContext, compile_flow_control_node_ir,
    compile_transform_node_ir, relations,
};
use crate::domain::{
    Diagnostic, NodeId, NormalizedProgram, OutputEndpoint, OutputPort, PatternIr, PatternNodeIr,
    PatternOutput, PatternStream, PortGroupId, RootSurfaceNodeKind, StreamSource,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NodeOutputKey {
    node: NodeId,
    endpoint: OutputEndpoint,
}

type NodeOutputsIr = BTreeMap<OutputEndpoint, PatternNodeIr>;

pub fn compile_normalized_program(
    program: &NormalizedProgram,
    ctx: &mut CompileContext,
) -> Result<PatternIr, Vec<Diagnostic>> {
    let mut cache = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    let mut outputs = Vec::new();

    for (node_id, node) in &program.root_nodes {
        if let RootSurfaceNodeKind::Output(output) = node {
            let endpoint = output
                .signature
                .input_groups
                .first()
                .map(|group| group.group.clone())
                .unwrap_or_else(|| PortGroupId::new("inputs"));
            let sources =
                relations::incoming_output_group_sources(&program.relations, node_id, &endpoint);
            let mut children = Vec::new();
            for source in sources {
                children.push(compile_source_ir(
                    program,
                    &source,
                    ctx,
                    &mut cache,
                    &mut visiting,
                )?);
            }
            let root = match children.len() {
                0 => PatternNodeIr::event_stream(PatternStream::default()),
                1 => children.remove(0),
                _ => PatternNodeIr::merge(children),
            };
            outputs.push(PatternOutput::new(node_id.clone(), root));
        }
    }

    Ok(PatternIr { outputs })
}

fn compile_source_ir(
    program: &NormalizedProgram,
    source: &StreamSource,
    ctx: &mut CompileContext,
    cache: &mut BTreeMap<NodeOutputKey, PatternNodeIr>,
    visiting: &mut BTreeSet<NodeOutputKey>,
) -> Result<PatternNodeIr, Vec<Diagnostic>> {
    compile_node_output_ir(
        program,
        source.node.clone(),
        source.endpoint.clone(),
        ctx,
        cache,
        visiting,
    )
}

fn compile_node_output_ir(
    program: &NormalizedProgram,
    node: NodeId,
    endpoint: OutputEndpoint,
    ctx: &mut CompileContext,
    cache: &mut BTreeMap<NodeOutputKey, PatternNodeIr>,
    visiting: &mut BTreeSet<NodeOutputKey>,
) -> Result<PatternNodeIr, Vec<Diagnostic>> {
    let key = NodeOutputKey {
        node: node.clone(),
        endpoint: endpoint.clone(),
    };
    if let Some(node_ir) = cache.get(&key) {
        return Ok(node_ir.clone());
    }
    if !visiting.insert(key.clone()) {
        return Err(vec![Diagnostic::new(
            crate::domain::DiagnosticCategory::Cycle,
            crate::domain::DiagnosticKind::RootCycle,
            "Compilation encountered a root cycle before validation resolved it.",
            Some(crate::domain::DiagnosticLocation::RootNode(node)),
        )]);
    }

    let outputs = compile_node_all_outputs_ir(program, &key.node, ctx, cache, visiting)?;
    visiting.remove(&key);
    for (output_endpoint, node_ir) in outputs {
        cache.insert(
            NodeOutputKey {
                node: key.node.clone(),
                endpoint: output_endpoint,
            },
            node_ir,
        );
    }
    cache.get(&key).cloned().ok_or_else(|| {
        vec![Diagnostic::new(
            crate::domain::DiagnosticCategory::Compile,
            crate::domain::DiagnosticKind::CompileFailed,
            "Compiled node did not produce the requested output endpoint.",
            Some(crate::domain::DiagnosticLocation::RootNode(
                key.node.clone(),
            )),
        )]
    })
}

fn compile_node_all_outputs_ir(
    program: &NormalizedProgram,
    node_id: &NodeId,
    ctx: &mut CompileContext,
    cache: &mut BTreeMap<NodeOutputKey, PatternNodeIr>,
    visiting: &mut BTreeSet<NodeOutputKey>,
) -> Result<NodeOutputsIr, Vec<Diagnostic>> {
    match program.root_nodes.get(node_id) {
        Some(RootSurfaceNodeKind::Scalar(scalar)) => Ok(BTreeMap::from_iter([(
            OutputEndpoint::Socket(OutputPort::new("out")),
            PatternNodeIr::scalar_stream(crate::domain::ScalarStream::new(vec![
                crate::domain::ScalarEvent::new(
                    crate::domain::CycleSpan::new(
                        crate::domain::CycleTime(crate::domain::Rational::zero()),
                        crate::domain::CycleDuration(crate::domain::Rational::one()),
                    ),
                    scalar.value,
                ),
            ])),
        )])),

        Some(RootSurfaceNodeKind::Container { container }) => {
            compile_container_outputs_ir(program, node_id, container, ctx, cache, visiting)
        }
        Some(RootSurfaceNodeKind::Transform(transform))
            if transform.kind == crate::domain::TransformKind::Trick =>
        {
            let error = |message: &str| {
                vec![Diagnostic::new(
                    crate::domain::DiagnosticCategory::Compile,
                    crate::domain::DiagnosticKind::CompileFailed,
                    message,
                    Some(crate::domain::DiagnosticLocation::RootNode(node_id.clone())),
                )]
            };
            ctx.trick_calls += 1;
            if ctx.trick_calls > 65_536 {
                return Err(error("Trick expansion exceeds 65536 calls"));
            }
            if !transform.sequence.is_empty() {
                let mut segments = Vec::new();
                for (source, duration, repeats) in &transform.sequence {
                    let body = compile_node_output_ir(
                        program,
                        source.clone(),
                        OutputEndpoint::Socket(OutputPort::new("out")),
                        ctx,
                        cache,
                        visiting,
                    )?;
                    segments.push(crate::domain::TimedPatternIr::new(
                        *duration, *repeats, body,
                    ));
                }
                return Ok(BTreeMap::from([(
                    OutputEndpoint::Socket(OutputPort::new("out")),
                    PatternNodeIr::arrange(segments),
                )]));
            }
            let source = transform
                .reference
                .as_ref()
                .ok_or_else(|| error("Trick has no definition"))?;
            if visiting.len() > 128 {
                return Err(error("Trick nesting exceeds 128 calls"));
            }
            let mut scope = BTreeMap::new();
            if let Some(argument) = &transform.argument {
                let sources = relations::incoming_socket_sources_normalized(
                    program,
                    node_id,
                    &crate::domain::InputPort::new("main"),
                );
                if sources.len() != 1 {
                    return Err(error("This trick needs one input pattern"));
                }
                let input = compile_source_ir(program, &sources[0], ctx, cache, visiting)?;
                scope.insert(
                    NodeOutputKey {
                        node: argument.clone(),
                        endpoint: OutputEndpoint::Socket(OutputPort::new("out")),
                    },
                    input,
                );
            }
            let body = compile_node_output_ir(
                program,
                source.clone(),
                OutputEndpoint::Socket(OutputPort::new("out")),
                ctx,
                &mut scope,
                visiting,
            )?;
            Ok(BTreeMap::from([(
                OutputEndpoint::Socket(OutputPort::new("out")),
                body,
            )]))
        }
        Some(RootSurfaceNodeKind::Transform(transform)) => {
            compile_transform_outputs_ir(program, node_id, transform, ctx, cache, visiting)
        }
        Some(RootSurfaceNodeKind::FlowControl(control)) => {
            compile_flow_control_outputs_ir(program, node_id, control, ctx, cache, visiting)
        }
        Some(RootSurfaceNodeKind::Output(_)) => Err(vec![Diagnostic::new(
            crate::domain::DiagnosticCategory::RootRelation,
            crate::domain::DiagnosticKind::OutputCannotProduceStream,
            "Outputs consume streams and cannot be compiled as produced streams.",
            Some(crate::domain::DiagnosticLocation::RootNode(node_id.clone())),
        )]),
        None if program
            .containers
            .contains_key(&crate::domain::ContainerId::new(node_id.0.clone())) =>
        {
            compile_container_outputs_ir(
                program,
                node_id,
                &crate::domain::ContainerId::new(node_id.0.clone()),
                ctx,
                cache,
                visiting,
            )
        }
        None => Err(vec![Diagnostic::new(
            crate::domain::DiagnosticCategory::Placement,
            crate::domain::DiagnosticKind::MissingContainer,
            "Node is missing from the root surface.",
            Some(crate::domain::DiagnosticLocation::RootNode(node_id.clone())),
        )]),
    }
}

fn compile_container_outputs_ir(
    program: &NormalizedProgram,
    node_id: &NodeId,
    container_id: &crate::domain::ContainerId,
    ctx: &mut CompileContext,
    cache: &mut BTreeMap<NodeOutputKey, PatternNodeIr>,
    visiting: &mut BTreeSet<NodeOutputKey>,
) -> Result<NodeOutputsIr, Vec<Diagnostic>> {
    let normalized = program
        .containers
        .get(container_id)
        .cloned()
        .ok_or_else(|| {
            vec![Diagnostic::new(
                crate::domain::DiagnosticCategory::Placement,
                crate::domain::DiagnosticKind::MissingContainer,
                "Container is missing from normalized program.",
                Some(crate::domain::DiagnosticLocation::RootNode(node_id.clone())),
            )]
        })?;
    let chain_sources = relations::incoming_chain_sources_normalized(program, node_id);
    if chain_sources.len() > 1 {
        return Err(vec![Diagnostic::new(
            crate::domain::DiagnosticCategory::RootRelation,
            crate::domain::DiagnosticKind::InvalidChainTarget,
            "A container may only have one incoming ChainedTo source in this pass.",
            Some(crate::domain::DiagnosticLocation::RootNode(node_id.clone())),
        )]);
    }
    let chain_ir = if let Some(source) = chain_sources.first() {
        Some(compile_source_ir(program, source, ctx, cache, visiting)?)
    } else {
        None
    };
    let local = compile_container_ir(program, &normalized, ctx)?;
    let node = if let Some(left) = chain_ir {
        PatternNodeIr::concat(vec![left, local])
    } else {
        local
    };

    Ok(BTreeMap::from_iter([(
        OutputEndpoint::Socket(OutputPort::new("out")),
        node,
    )]))
}

fn compile_transform_outputs_ir(
    program: &NormalizedProgram,
    node_id: &NodeId,
    transform: &crate::domain::TransformNode,
    ctx: &mut CompileContext,
    cache: &mut BTreeMap<NodeOutputKey, PatternNodeIr>,
    visiting: &mut BTreeSet<NodeOutputKey>,
) -> Result<NodeOutputsIr, Vec<Diagnostic>> {
    compile_transform_node_ir(program, node_id, transform, |source| {
        compile_source_ir(program, source, ctx, cache, visiting)
    })
}

fn compile_flow_control_outputs_ir(
    program: &NormalizedProgram,
    node_id: &NodeId,
    control: &crate::domain::FlowControlNode,
    ctx: &mut CompileContext,
    cache: &mut BTreeMap<NodeOutputKey, PatternNodeIr>,
    visiting: &mut BTreeSet<NodeOutputKey>,
) -> Result<NodeOutputsIr, Vec<Diagnostic>> {
    let policy_ctx = CompileContext {
        trick_calls: ctx.trick_calls,
        cycle_index: ctx.cycle_index,
        provenance_stack: ctx.provenance_stack.clone(),
    };
    compile_flow_control_node_ir(program, node_id, control, &policy_ctx, |source| {
        compile_source_ir(program, source, ctx, cache, visiting)
    })
}
