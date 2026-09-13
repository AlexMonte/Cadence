use crate::domain::{
    DeduplicatePolicyIr, FlowControlKind, FlowControlNode, FlowControlPolicy, FlowInputIr,
    InputEndpoint, OutputEndpoint, OutputPort, PatternNodeIr, PatternStream, PortGroupId,
    PortMemberId, PriorityMergePolicyIr,
};
use std::collections::BTreeMap;

pub type NodeInputsIr = BTreeMap<InputEndpoint, Vec<PatternNodeIr>>;
pub type NodeOutputsIr = BTreeMap<OutputEndpoint, PatternNodeIr>;

pub fn apply_flow_control_policy_ir(
    control: &FlowControlNode,
    inputs: NodeInputsIr,
    _cycle_index: usize,
) -> NodeOutputsIr {
    let out = OutputEndpoint::Socket(OutputPort::new("out"));
    let native = match control.kind {
        FlowControlKind::Layer => Some(PatternNodeIr::merge(group_inputs(
            &inputs, "streams", control,
        ))),
        FlowControlKind::Merge => {
            let children = group_inputs(&inputs, "streams", control);
            Some(match control.policy {
                FlowControlPolicy::MergeAppend => PatternNodeIr::concat(children),
                FlowControlPolicy::MergePriority => PatternNodeIr::priority_merge(
                    children,
                    PriorityMergePolicyIr::whole_span_overlap(),
                ),
                FlowControlPolicy::MergeDeduplicate => PatternNodeIr::deduplicate(
                    PatternNodeIr::merge(children),
                    DeduplicatePolicyIr::whole_span_and_value(),
                ),
                _ => PatternNodeIr::merge(children),
            })
        }
        FlowControlKind::Mask if matches!(control.policy, FlowControlPolicy::MaskClip) => {
            let first = |name: &str| {
                inputs
                    .get(&InputEndpoint::Socket(crate::domain::InputPort::new(name)))
                    .and_then(|nodes| nodes.first())
                    .cloned()
                    .unwrap_or_else(|| PatternNodeIr::event_stream(PatternStream::default()))
            };
            let mask = first("mask").map_event_leaves(&mut |stream, _| {
                PatternNodeIr::scalar_stream(crate::domain::ScalarStream::new(
                    stream
                        .events
                        .iter()
                        .filter_map(|event| match event.value {
                            crate::domain::EventValue::Scalar { value } => {
                                Some(crate::domain::ScalarEvent::new(event.span, value))
                            }
                            _ => None,
                        })
                        .collect(),
                ))
            });
            Some(PatternNodeIr::mask_clip(first("main"), mask))
        }
        _ => None,
    };
    if let Some(node) = native {
        return BTreeMap::from([(out, node)]);
    }
    let inputs = inputs
        .into_iter()
        .map(|(endpoint, nodes)| FlowInputIr { endpoint, nodes })
        .collect::<Vec<_>>();
    let outputs = control
        .signature
        .output_sockets
        .iter()
        .map(|socket| OutputEndpoint::Socket(socket.port.clone()))
        .chain(control.members.outputs.iter().flat_map(|(group, members)| {
            members.iter().map(|member| OutputEndpoint::GroupMember {
                group: group.clone(),
                member: member.clone(),
            })
        }))
        .collect::<Vec<_>>();
    outputs
        .into_iter()
        .map(|output| {
            (
                output.clone(),
                PatternNodeIr::FlowProjection {
                    control: control.clone(),
                    inputs: inputs.clone(),
                    output,
                },
            )
        })
        .collect()
}
fn group_inputs(
    inputs: &NodeInputsIr,
    group: &str,
    control: &FlowControlNode,
) -> Vec<PatternNodeIr> {
    let group = PortGroupId::new(group);
    let members = control
        .members
        .inputs
        .get(&group)
        .cloned()
        .unwrap_or_else(|| {
            inputs
                .keys()
                .filter_map(|endpoint| match endpoint {
                    InputEndpoint::GroupMember { group: g, member } if g == &group => {
                        Some(member.clone())
                    }
                    _ => None,
                })
                .collect::<Vec<PortMemberId>>()
        });
    members
        .into_iter()
        .flat_map(|member| {
            inputs
                .get(&InputEndpoint::GroupMember {
                    group: group.clone(),
                    member,
                })
                .into_iter()
                .flatten()
                .cloned()
        })
        .collect()
}
