use std::collections::BTreeMap;

use crate::application::CompileContext;
use crate::domain::{
    FlowControlKind, FlowControlNode, FlowControlPolicy, InputEndpoint, OutputEndpoint, OutputPort,
    PatternNodeIr, PatternStream, PortGroupId, PortMemberId,
};

use super::flow_policy::{
    NodeInputs, apply_choice_policy, apply_switch_policy, first_socket_input,
};

pub type NodeInputsIr = BTreeMap<InputEndpoint, Vec<PatternNodeIr>>;
pub type NodeOutputsIr = BTreeMap<OutputEndpoint, PatternNodeIr>;

pub fn apply_flow_control_policy_ir(
    control: &FlowControlNode,
    inputs: NodeInputsIr,
    cycle_index: usize,
) -> NodeOutputsIr {
    let flat_inputs = inputs
        .iter()
        .map(|(endpoint, nodes)| {
            (
                endpoint.clone(),
                nodes.iter().map(|node| node.flatten()).collect::<Vec<_>>(),
            )
        })
        .collect::<NodeInputs>();

    match control.kind {
        FlowControlKind::Mask => {
            let main = first_ir_input(
                &inputs,
                InputEndpoint::Socket(crate::domain::InputPort::new("main")),
            )
            .unwrap_or_else(|| PatternNodeIr::event_stream(PatternStream::default()));
            let mask = first_ir_input(
                &inputs,
                InputEndpoint::Socket(crate::domain::InputPort::new("mask")),
            )
            .unwrap_or_else(|| PatternNodeIr::event_stream(PatternStream::default()));
            let node = if matches!(control.policy, FlowControlPolicy::MaskClip) {
                PatternNodeIr::mask_clip(main, mask)
            } else {
                PatternNodeIr::event_stream(super::flow_policy::apply_mask_policy(
                    control,
                    main.flatten(),
                    mask.flatten(),
                ))
            };
            return BTreeMap::from_iter([(OutputEndpoint::Socket(OutputPort::new("out")), node)]);
        }
        FlowControlKind::Split => {
            let main = first_ir_input(
                &inputs,
                InputEndpoint::Socket(crate::domain::InputPort::new("main")),
            )
            .unwrap_or_else(|| PatternNodeIr::event_stream(PatternStream::default()));
            let mut outputs = BTreeMap::new();
            for member in control
                .members
                .outputs
                .get(&PortGroupId::new("branches"))
                .into_iter()
                .flatten()
            {
                let stream =
                    super::flow_policy::apply_split_policy(control, main.flatten(), member);
                outputs.insert(
                    OutputEndpoint::GroupMember {
                        group: PortGroupId::new("branches"),
                        member: member.clone(),
                    },
                    PatternNodeIr::cycle_slots(vec![PatternNodeIr::event_stream(stream)]),
                );
            }
            outputs
        }
        FlowControlKind::Route => {
            let main = first_ir_input(
                &inputs,
                InputEndpoint::Socket(crate::domain::InputPort::new("main")),
            )
            .unwrap_or_else(|| PatternNodeIr::event_stream(PatternStream::default()));
            let control_stream =
                first_socket_input(&flat_inputs, "control").map(|_| main.flatten());
            let mut outputs = BTreeMap::new();
            for member in control
                .members
                .outputs
                .get(&PortGroupId::new("routes"))
                .into_iter()
                .flatten()
            {
                let stream = super::flow_policy::apply_route_policy(
                    control,
                    main.flatten(),
                    control_stream.clone(),
                    member,
                );
                outputs.insert(
                    OutputEndpoint::GroupMember {
                        group: PortGroupId::new("routes"),
                        member: member.clone(),
                    },
                    PatternNodeIr::event_stream(stream),
                );
            }
            return outputs;
        }
        FlowControlKind::Switch => {
            let candidates = collect_group_ir_inputs_in_member_order(
                &inputs,
                &PortGroupId::new("candidates"),
                control.members.inputs.get(&PortGroupId::new("candidates")),
            );
            if matches!(control.policy, FlowControlPolicy::SwitchCycleIndex) {
                let index = cycle_index % candidates.len().max(1);
                return BTreeMap::from_iter([(
                    OutputEndpoint::Socket(OutputPort::new("out")),
                    candidates
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| PatternNodeIr::event_stream(PatternStream::default())),
                )]);
            }
            let policy_ctx = CompileContext {
                cycle_index,
                ..CompileContext::default()
            };
            let stream = apply_switch_policy(
                control,
                candidates.iter().map(|node| node.flatten()).collect(),
                first_socket_input(&flat_inputs, "control"),
                &policy_ctx,
            );
            BTreeMap::from_iter([(
                OutputEndpoint::Socket(OutputPort::new("out")),
                PatternNodeIr::event_stream(stream),
            )])
        }
        FlowControlKind::Choice => {
            let options = collect_group_ir_inputs_in_member_order(
                &inputs,
                &PortGroupId::new("options"),
                control.members.inputs.get(&PortGroupId::new("options")),
            );
            if matches!(control.policy, FlowControlPolicy::ChoiceCycle) {
                let index = cycle_index % options.len().max(1);
                return BTreeMap::from_iter([(
                    OutputEndpoint::Socket(OutputPort::new("out")),
                    options
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| PatternNodeIr::event_stream(PatternStream::default())),
                )]);
            }
            let policy_ctx = CompileContext {
                cycle_index,
                ..CompileContext::default()
            };
            let stream = apply_choice_policy(
                control,
                options.iter().map(|node| node.flatten()).collect(),
                first_socket_input(&flat_inputs, "control"),
                &policy_ctx,
            );
            BTreeMap::from_iter([(
                OutputEndpoint::Socket(OutputPort::new("out")),
                PatternNodeIr::event_stream(stream),
            )])
        }
        FlowControlKind::Merge => {
            let children = collect_group_ir_inputs(&inputs, &PortGroupId::new("streams"));
            let node = match control.policy {
                FlowControlPolicy::MergeAppend => PatternNodeIr::concat(children),
                _ => PatternNodeIr::merge(children),
            };
            BTreeMap::from_iter([(OutputEndpoint::Socket(OutputPort::new("out")), node)])
        }
        FlowControlKind::Layer => BTreeMap::from_iter([(
            OutputEndpoint::Socket(OutputPort::new("out")),
            PatternNodeIr::merge(collect_group_ir_inputs(
                &inputs,
                &PortGroupId::new("streams"),
            )),
        )]),
        FlowControlKind::Mix => BTreeMap::from_iter([(
            OutputEndpoint::Socket(OutputPort::new("out")),
            PatternNodeIr::event_stream(super::flow_policy::apply_mix_policy(
                control,
                collect_group_ir_inputs(&inputs, &PortGroupId::new("streams"))
                    .into_iter()
                    .map(|node| node.flatten())
                    .collect(),
                first_socket_input(&flat_inputs, "amount"),
            )),
        )]),
    }
}

fn first_ir_input(inputs: &NodeInputsIr, endpoint: InputEndpoint) -> Option<PatternNodeIr> {
    inputs
        .get(&endpoint)
        .and_then(|nodes| nodes.first().cloned())
}

fn collect_group_ir_inputs(inputs: &NodeInputsIr, group: &PortGroupId) -> Vec<PatternNodeIr> {
    let mut collected = Vec::new();
    for (endpoint, nodes) in inputs {
        if matches!(endpoint, InputEndpoint::GroupMember { group: endpoint_group, .. } if endpoint_group == group)
        {
            collected.extend(nodes.iter().cloned());
        }
    }
    collected
}

fn collect_group_ir_inputs_in_member_order(
    inputs: &NodeInputsIr,
    group: &PortGroupId,
    members: Option<&Vec<PortMemberId>>,
) -> Vec<PatternNodeIr> {
    let Some(members) = members else {
        return collect_group_ir_inputs(inputs, group);
    };
    members
        .iter()
        .filter_map(|member| {
            inputs
                .get(&InputEndpoint::GroupMember {
                    group: group.clone(),
                    member: member.clone(),
                })
                .and_then(|nodes| nodes.first().cloned())
        })
        .collect()
}
