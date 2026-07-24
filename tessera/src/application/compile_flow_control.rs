use std::collections::BTreeMap;

use crate::domain::{
    ConnectionRule, Diagnostic, DiagnosticCategory, DiagnosticKind, DiagnosticLocation,
    FlowControlNode, InputEndpoint, NodeId, NormalizedProgram, PatternNodeIr, PortCountRule,
    StreamSource,
};

use super::compile_context::CompileContext;
use super::compile_transform::default_stream;
use super::flow_policy_ir::{NodeInputsIr, NodeOutputsIr, apply_flow_control_policy_ir};
use super::relations;

pub fn compile_flow_control_node_ir<F>(
    program: &NormalizedProgram,
    node_id: &NodeId,
    node: &FlowControlNode,
    ctx: &CompileContext,
    mut resolve_source: F,
) -> Result<NodeOutputsIr, Vec<Diagnostic>>
where
    F: FnMut(&StreamSource) -> Result<PatternNodeIr, Vec<Diagnostic>>,
{
    let mut inputs: NodeInputsIr = BTreeMap::new();

    for socket in &node.signature.input_sockets {
        let endpoint = InputEndpoint::Socket(socket.port.clone());
        let sources = relations::incoming_socket_sources_normalized(program, node_id, &socket.port);
        if sources.len() > 1 {
            return Err(vec![Diagnostic::new(
                DiagnosticCategory::FlowControlTopology,
                DiagnosticKind::OptionalSocketMultiplyBound,
                "Input socket received more than one binding.",
                Some(DiagnosticLocation::InputEndpoint {
                    node: node_id.clone(),
                    endpoint,
                }),
            )]);
        }
        if let Some(source) = sources.first() {
            inputs.insert(endpoint, vec![resolve_source(source)?]);
        } else if let Some(default) = socket.default.as_ref() {
            inputs.insert(
                endpoint,
                vec![PatternNodeIr::event_stream(default_stream(default))],
            );
        } else if matches!(socket.connection, ConnectionRule::Required) {
            return Err(vec![Diagnostic::new(
                DiagnosticCategory::FlowControlTopology,
                DiagnosticKind::RequiredSocketMissing,
                "Required input socket is missing during flow-control compilation.",
                Some(DiagnosticLocation::InputEndpoint {
                    node: node_id.clone(),
                    endpoint,
                }),
            )]);
        }
    }

    for group in &node.signature.input_groups {
        let bindings =
            relations::incoming_group_bindings_normalized(program, node_id, &group.group);
        if !port_count_satisfied(group.count, bindings.len() as u32) {
            return Err(vec![Diagnostic::new(
                DiagnosticCategory::FlowControlTopology,
                DiagnosticKind::PortCountViolation,
                "Input group binding count does not satisfy the group count rule.",
                Some(DiagnosticLocation::InputEndpoint {
                    node: node_id.clone(),
                    endpoint: InputEndpoint::GroupMember {
                        group: group.group.clone(),
                        member: crate::domain::PortMemberId::new("*"),
                    },
                }),
            )]);
        }
        for (endpoint, source) in bindings {
            inputs.insert(endpoint, vec![resolve_source(&source)?]);
        }
    }

    Ok(apply_flow_control_policy_ir(node, inputs, ctx.cycle_index))
}

fn port_count_satisfied(rule: PortCountRule, count: u32) -> bool {
    match rule {
        PortCountRule::ZeroOrMore => true,
        PortCountRule::OneOrMore => count >= 1,
        PortCountRule::Exactly(expected) => count == expected,
        PortCountRule::Range { min, max } => count >= min && count <= max,
    }
}
