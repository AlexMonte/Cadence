use crate::domain::{
    InputEndpoint, InputPort, NodeId, NormalizedProgram, PortGroupId, RootRelation, StreamSource,
    StreamTarget, TesseraProgram,
};

pub fn incoming_socket_sources(
    relations: &[RootRelation],
    node_id: &NodeId,
    port: &InputPort,
) -> Vec<StreamSource> {
    let mut sources = Vec::new();
    for relation in relations {
        let RootRelation::FlowsTo { from, to } = relation else {
            continue;
        };
        let matches_socket = match to {
            StreamTarget::TransformInput { node, endpoint }
            | StreamTarget::FlowControlInput { node, endpoint }
            | StreamTarget::OutputInput { node, endpoint } => {
                node == node_id
                    && matches!(endpoint, InputEndpoint::Socket(target_port) if target_port == port)
            }
        };
        if matches_socket {
            sources.push(from.clone());
        }
    }
    sources
}

pub fn incoming_chain_sources(relations: &[RootRelation], node_id: &NodeId) -> Vec<StreamSource> {
    let mut sources = Vec::new();
    for relation in relations {
        if let RootRelation::ChainedTo { from, to } = relation
            && to == node_id
        {
            sources.push(from.clone());
        }
    }
    sources
}

pub fn incoming_group_bindings(
    relations: &[RootRelation],
    node_id: &NodeId,
    group: &PortGroupId,
) -> Vec<(InputEndpoint, StreamSource)> {
    let mut bindings = Vec::new();
    for relation in relations {
        let RootRelation::FlowsTo { from, to } = relation else {
            continue;
        };
        let endpoint = match to {
            StreamTarget::TransformInput { node, endpoint }
            | StreamTarget::FlowControlInput { node, endpoint }
            | StreamTarget::OutputInput { node, endpoint }
                if node == node_id =>
            {
                endpoint
            }
            _ => continue,
        };
        if let InputEndpoint::GroupMember {
            group: target_group,
            member,
        } = endpoint
            && target_group == group
        {
            bindings.push((
                InputEndpoint::GroupMember {
                    group: target_group.clone(),
                    member: member.clone(),
                },
                from.clone(),
            ));
        }
    }
    bindings
}

pub fn incoming_flow_sources_to_group(
    relations: &[RootRelation],
    node_id: &NodeId,
    group: &PortGroupId,
) -> Vec<StreamSource> {
    incoming_group_bindings(relations, node_id, group)
        .into_iter()
        .map(|(_, source)| source)
        .collect()
}

pub fn incoming_output_group_sources(
    relations: &[RootRelation],
    node_id: &NodeId,
    group: &PortGroupId,
) -> Vec<StreamSource> {
    let mut sources = Vec::new();
    for relation in relations {
        if let RootRelation::FlowsTo { from, to } = relation
            && matches!(to, StreamTarget::OutputInput { node, endpoint: InputEndpoint::GroupMember { group: target_group, .. } } if node == node_id && target_group == group)
        {
            sources.push(from.clone());
        }
    }
    sources
}

pub fn incoming_socket_sources_normalized(
    program: &NormalizedProgram,
    node_id: &NodeId,
    port: &InputPort,
) -> Vec<StreamSource> {
    incoming_socket_sources(&program.relations, node_id, port)
}

pub fn incoming_chain_sources_normalized(
    program: &NormalizedProgram,
    node_id: &NodeId,
) -> Vec<StreamSource> {
    incoming_chain_sources(&program.relations, node_id)
}

pub fn incoming_group_bindings_normalized(
    program: &NormalizedProgram,
    node_id: &NodeId,
    group: &PortGroupId,
) -> Vec<(InputEndpoint, StreamSource)> {
    incoming_group_bindings(&program.relations, node_id, group)
}

pub fn incoming_socket_sources_program(
    program: &TesseraProgram,
    node_id: &NodeId,
    port: &InputPort,
) -> Vec<StreamSource> {
    incoming_socket_sources(&program.relations, node_id, port)
}

pub fn incoming_flow_sources_to_group_program(
    program: &TesseraProgram,
    node_id: &NodeId,
    group: &PortGroupId,
) -> Vec<StreamSource> {
    incoming_flow_sources_to_group(&program.relations, node_id, group)
}
