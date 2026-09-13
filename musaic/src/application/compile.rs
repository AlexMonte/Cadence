//! The single boundary from Musaic's authored document to Tessera's pattern IR.

use std::collections::{BTreeMap, BTreeSet};

use crate::{application::session::MusaicProject, domain::document::export_document_program};
use tessera::prelude::{
    AuthoredTesseraProgram, ConnectionRule, ContainerId, ContainerSurfaceTile, InputEndpoint,
    NodeId, NodeInputRole, NodeSignature, PatternIr, PortCountRule, RootRelation,
    RootSurfaceNodeKind, StreamTarget, TesseraCompiler,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCompileError {
    pub message: String,
}

pub fn compile_project_ir(project: &MusaicProject) -> Result<PatternIr, Vec<ProjectCompileError>> {
    let authored = export_document_program(&project.document).map_err(|error| {
        vec![ProjectCompileError {
            message: format!("Could not export the document: {error:?}"),
        }]
    })?;
    let (authored, _) = executable_program(authored);
    TesseraCompiler::new()
        .compile_authored(&authored)
        .map(|report| report.ir)
        .map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| ProjectCompileError {
                    message: diagnostic.message,
                })
                .collect()
        })
}

pub fn inactive_outputs(project: &MusaicProject) -> Vec<NodeId> {
    let Ok(program) = export_document_program(&project.document) else {
        return Vec::new();
    };
    executable_program(program).1.into_iter().collect()
}

pub fn has_inactive_outputs(project: &MusaicProject) -> bool {
    !inactive_outputs(project).is_empty()
}

/// Selects executable output routes without changing the authored document.
/// Unfed work-in-progress remains editable, while every connected route is
/// still validated by Tessera—including routes with incompatible stream types.
fn executable_program(
    mut program: AuthoredTesseraProgram,
) -> (AuthoredTesseraProgram, BTreeSet<NodeId>) {
    let edges = crate::domain::document::connection_policy::endpoint_connections(&program);
    let mut incoming = BTreeMap::<NodeId, Vec<_>>::new();
    for edge in &edges {
        incoming
            .entry(edge.to.clone())
            .or_default()
            .push(edge.clone());
    }

    let all_outputs = program
        .root_surface
        .nodes
        .iter()
        .filter(|(_, kind)| matches!(kind, RootSurfaceNodeKind::Output(_)))
        .map(|(node, _)| node.clone())
        .collect::<BTreeSet<_>>();
    let active_outputs = all_outputs
        .iter()
        .filter(|output| {
            incoming.get(*output).is_some_and(|sources| {
                sources.iter().any(|edge| {
                    node_resolves_to_stream(&program, &incoming, &edge.from, &mut BTreeSet::new())
                })
            })
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let inactive = all_outputs
        .difference(&active_outputs)
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut retained = BTreeSet::new();
    let mut pending = active_outputs.into_iter().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        if !retained.insert(node.clone()) {
            continue;
        }
        if let Some(edges) = incoming.get(&node) {
            pending.extend(edges.iter().map(|edge| edge.from.clone()));
        }
        if let Some(RootSurfaceNodeKind::Transform(transform)) =
            program.root_surface.nodes.get(&node)
        {
            pending.extend(transform.reference.iter().cloned());
            pending.extend(transform.argument.iter().cloned());
            pending.extend(transform.sequence.iter().map(|(node, _, _)| node.clone()));
        }
    }

    program
        .root_surface
        .nodes
        .retain(|node, _| retained.contains(node));
    program
        .root_surface
        .placements
        .retain(|node, _| retained.contains(node));
    program
        .root_surface
        .bindings
        .retain(|node, _| retained.contains(node));
    program.root_surface.explicit_relations.retain(|relation| {
        let (source, target) = relation_nodes(relation);
        retained.contains(source) && retained.contains(target)
    });

    let mut containers = BTreeSet::new();
    for node in program.root_surface.nodes.values() {
        if let RootSurfaceNodeKind::Container { container } = node {
            collect_container_tree(&program, container, &mut containers);
        }
    }
    program
        .containers
        .retain(|container, _| containers.contains(container));
    (program, inactive)
}

fn node_resolves_to_stream(
    program: &AuthoredTesseraProgram,
    incoming: &BTreeMap<
        NodeId,
        Vec<crate::domain::document::connection_policy::EndpointConnection>,
    >,
    node: &NodeId,
    visiting: &mut BTreeSet<NodeId>,
) -> bool {
    if !visiting.insert(node.clone()) {
        return false;
    }
    let resolves = match program.root_surface.nodes.get(node) {
        Some(RootSurfaceNodeKind::Container { .. } | RootSurfaceNodeKind::Scalar(_)) => true,
        Some(RootSurfaceNodeKind::Transform(transform)) => {
            signature_resolves(program, incoming, node, &transform.signature, visiting)
        }
        Some(RootSurfaceNodeKind::FlowControl(control)) => {
            signature_resolves(program, incoming, node, &control.signature, visiting)
        }
        Some(RootSurfaceNodeKind::Output(_)) | None => false,
    };
    visiting.remove(node);
    resolves
}

fn signature_resolves(
    program: &AuthoredTesseraProgram,
    incoming: &BTreeMap<
        NodeId,
        Vec<crate::domain::document::connection_policy::EndpointConnection>,
    >,
    node: &NodeId,
    signature: &NodeSignature,
    visiting: &mut BTreeSet<NodeId>,
) -> bool {
    let edges = incoming.get(node).map(Vec::as_slice).unwrap_or_default();
    let sockets_resolve = signature.input_sockets.iter().all(|socket| {
        let resolves = edges.iter().any(|edge| {
            edge.input == InputEndpoint::Socket(socket.port.clone())
                && node_resolves_to_stream(program, incoming, &edge.from, visiting)
        });
        if matches!(socket.role, NodeInputRole::Main)
            || matches!(socket.connection, ConnectionRule::Required)
        {
            resolves || socket.default.is_some()
        } else {
            true
        }
    });
    sockets_resolve
        && signature.input_groups.iter().all(|group| {
            let sources = edges
                .iter()
                .filter(|edge| {
                    matches!(&edge.input, InputEndpoint::GroupMember { group: id, .. } if id == &group.group)
                })
                .collect::<Vec<_>>();
            port_count_satisfied(group.count, sources.len() as u32)
                && sources.iter().all(|edge| {
                    node_resolves_to_stream(program, incoming, &edge.from, visiting)
                })
        })
}

fn port_count_satisfied(rule: PortCountRule, count: u32) -> bool {
    match rule {
        PortCountRule::ZeroOrMore => true,
        PortCountRule::OneOrMore => count >= 1,
        PortCountRule::Exactly(expected) => count == expected,
        PortCountRule::Range { min, max } => count >= min && count <= max,
    }
}

fn relation_nodes(relation: &RootRelation) -> (&NodeId, &NodeId) {
    match relation {
        RootRelation::ChainedTo { from, to } => (&from.node, to),
        RootRelation::FlowsTo { from, to } => {
            let target = match to {
                StreamTarget::OutputInput { node, .. }
                | StreamTarget::TransformInput { node, .. }
                | StreamTarget::FlowControlInput { node, .. } => node,
            };
            (&from.node, target)
        }
    }
}

fn collect_container_tree(
    program: &AuthoredTesseraProgram,
    container: &ContainerId,
    retained: &mut BTreeSet<ContainerId>,
) {
    if !retained.insert(container.clone()) {
        return;
    }
    let Some(definition) = program.containers.get(container) else {
        return;
    };
    for tile in &definition.stack {
        if let ContainerSurfaceTile::NestedContainer(nested) = tile {
            collect_container_tree(program, nested, retained);
        }
    }
}
