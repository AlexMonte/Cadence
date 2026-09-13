//! Targeted changes to existing flow policies and endpoint routing. Signatures
//! and imported group identities remain intact unless a member is explicitly edited.
use crate::{
    application::session::MusaicProject,
    domain::{document::DocumentNodeKind, flow},
};
use serde::{Deserialize, Serialize};
use tessera::prelude::{
    FlowControlPolicy, InputEndpoint, NodeId, OutputEndpoint, PortGroupId, PortMemberId,
    SpatialSide,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowEdit {
    Policy(FlowControlPolicy),
    Input {
        endpoint: InputEndpoint,
        side: SpatialSide,
    },
    Output {
        endpoint: OutputEndpoint,
        side: SpatialSide,
    },
    AddMember {
        group: PortGroupId,
        input: bool,
    },
    RenameMember {
        group: PortGroupId,
        member: PortMemberId,
        name: String,
        input: bool,
    },
    RemoveMember {
        group: PortGroupId,
        member: PortMemberId,
        input: bool,
    },
}

pub(super) fn apply(
    project: &mut MusaicProject,
    node: &NodeId,
    edit: &FlowEdit,
) -> Result<(), String> {
    let mut candidate = project.document.clone();
    let Some(DocumentNodeKind::FlowControl(current)) = candidate.graph.node(node).map(|n| &n.kind)
    else {
        return Err("Select a flow tile first".into());
    };
    let mut control = current.clone();
    let mut program = crate::domain::document::export_document_program(&candidate)
        .map_err(|error| format!("Cannot export this document: {error:?}"))?;
    let mut bindings = program
        .root_surface
        .bindings
        .get(node)
        .cloned()
        .unwrap_or_else(|| {
            tessera::prelude::default_spatial_bindings(
                &tessera::prelude::RootSurfaceNodeKind::FlowControl(control.clone()),
            )
        });
    let allowed_side = |side: &SpatialSide| {
        matches!(
            side,
            SpatialSide::North
                | SpatialSide::East
                | SpatialSide::South
                | SpatialSide::West
                | SpatialSide::Off
        )
    };
    match edit {
        FlowEdit::Policy(policy) => {
            if !flow::policies(&control)
                .iter()
                .any(|p| std::mem::discriminant(p) == std::mem::discriminant(policy))
            {
                return Err("This policy belongs to another flow kind".into());
            }
            control.policy = policy.clone();
        }
        FlowEdit::Input { endpoint, side } => {
            if !allowed_side(side) || !flow::inputs(&control).contains(endpoint) {
                return Err("This input endpoint does not exist".into());
            }
            bindings.inputs.insert(endpoint.clone(), *side);
        }
        FlowEdit::Output { endpoint, side } => {
            if !allowed_side(side) || !flow::outputs(&control).contains(endpoint) {
                return Err("This output endpoint does not exist".into());
            }
            bindings.outputs.insert(endpoint.clone(), *side);
        }
        FlowEdit::AddMember { group, input }
        | FlowEdit::RemoveMember { group, input, .. }
        | FlowEdit::RenameMember { group, input, .. } => {
            let count = if *input {
                control.signature.input_group(group).map(|g| g.count)
            } else {
                control.signature.output_group(group).map(|g| g.count)
            }
            .ok_or("This port group does not exist")?;
            let members = if *input {
                control.members.inputs.entry(group.clone()).or_default()
            } else {
                control.members.outputs.entry(group.clone()).or_default()
            };
            let (min, max) = match count {
                tessera::prelude::PortCountRule::Exactly(n) => (n, n),
                tessera::prelude::PortCountRule::Range { min, max } => (min, max),
                tessera::prelude::PortCountRule::OneOrMore => (1, u32::MAX),
                tessera::prelude::PortCountRule::ZeroOrMore => (0, u32::MAX),
            };
            match edit {
                FlowEdit::RenameMember { member, name, .. } => {
                    let name = name.trim();
                    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
                        return Err(
                            "Choose a short, nonempty member name without line breaks".into()
                        );
                    }
                    let index = members
                        .iter()
                        .position(|m| m == member)
                        .ok_or("This member does not exist")?;
                    let name = PortMemberId::new(name);
                    if members.iter().any(|m| m == &name && m != member) {
                        return Err("Each member in this group needs a unique name".into());
                    }
                    members[index] = name.clone();
                    if *input {
                        let side = bindings
                            .inputs
                            .remove(&InputEndpoint::GroupMember {
                                group: group.clone(),
                                member: member.clone(),
                            })
                            .unwrap_or(SpatialSide::Off);
                        bindings.inputs.insert(
                            InputEndpoint::GroupMember {
                                group: group.clone(),
                                member: name,
                            },
                            side,
                        );
                    } else {
                        let side = bindings
                            .outputs
                            .remove(&OutputEndpoint::GroupMember {
                                group: group.clone(),
                                member: member.clone(),
                            })
                            .unwrap_or(SpatialSide::Off);
                        bindings.outputs.insert(
                            OutputEndpoint::GroupMember {
                                group: group.clone(),
                                member: name,
                            },
                            side,
                        );
                    }
                }
                FlowEdit::AddMember { .. } => {
                    if members.len() >= max.min(128) as usize {
                        return Err("This group has reached its member limit".into());
                    }
                    let name = (1..)
                        .map(|n| PortMemberId::new(format!("{n}")))
                        .find(|m| !members.contains(m))
                        .unwrap();
                    members.push(name.clone());
                    if *input {
                        bindings.inputs.insert(
                            InputEndpoint::GroupMember {
                                group: group.clone(),
                                member: name,
                            },
                            SpatialSide::Off,
                        );
                    } else {
                        bindings.outputs.insert(
                            OutputEndpoint::GroupMember {
                                group: group.clone(),
                                member: name,
                            },
                            SpatialSide::Off,
                        );
                    }
                }
                FlowEdit::RemoveMember { member, .. } => {
                    if members.len() <= min as usize {
                        return Err("This group requires its remaining members".into());
                    }
                    let index = members
                        .iter()
                        .position(|m| m == member)
                        .ok_or("This member does not exist")?;
                    members.remove(index);
                    if *input {
                        bindings.inputs.remove(&InputEndpoint::GroupMember {
                            group: group.clone(),
                            member: member.clone(),
                        });
                    } else {
                        bindings.outputs.remove(&OutputEndpoint::GroupMember {
                            group: group.clone(),
                            member: member.clone(),
                        });
                    }
                }
                _ => unreachable!(),
            }
        }
    }
    control.members.validate()?;
    update_explicit_connections(&mut program.root_surface.explicit_relations, node, edit);
    candidate.graph.set_flow_control(node, control)?;
    program.root_surface.bindings.insert(node.clone(), bindings);
    candidate.replace_connections_from(&program);
    candidate.validate()?;
    project.document = candidate;
    Ok(())
}

// An explicitly connected edge owns endpoint identity. Editing that
// endpoint must update the edge too, or a stale edge overrides the new side.
fn update_explicit_connections(
    relations: &mut Vec<tessera::prelude::RootRelation>,
    node: &NodeId,
    edit: &FlowEdit,
) {
    use tessera::prelude::{RootRelation, StreamTarget};
    relations.retain_mut(|relation| {
        let (from, target) = match relation {
            RootRelation::FlowsTo { from, to } => (from, Some(to)),
            RootRelation::ChainedTo { from, .. } => (from, None),
        };
        let target = target.map(|to| match to {
            StreamTarget::TransformInput { node, endpoint }
            | StreamTarget::FlowControlInput { node, endpoint }
            | StreamTarget::OutputInput { node, endpoint } => (node, endpoint),
        });
        match edit {
            FlowEdit::Input { endpoint, .. } => {
                !target.is_some_and(|(id, input)| id == node && input == endpoint)
            }
            FlowEdit::Output { endpoint, .. } => from.node != *node || from.endpoint != *endpoint,
            FlowEdit::RenameMember {
                group,
                member,
                name,
                input,
            } => {
                if *input {
                    if let Some((
                        id,
                        InputEndpoint::GroupMember {
                            group: g,
                            member: m,
                        },
                    )) = target
                    {
                        if id == node && g == group && m == member {
                            *m = PortMemberId::new(name.trim());
                        }
                    }
                } else if from.node == *node {
                    if let OutputEndpoint::GroupMember {
                        group: g,
                        member: m,
                    } = &mut from.endpoint
                    {
                        if g == group && m == member {
                            *m = PortMemberId::new(name.trim());
                        }
                    }
                }
                true
            }
            FlowEdit::RemoveMember {
                group,
                member,
                input,
            } => {
                if *input {
                    !target.is_some_and(|(id, endpoint)| {
                        id == node
                            && *endpoint
                                == InputEndpoint::GroupMember {
                                    group: group.clone(),
                                    member: member.clone(),
                                }
                    })
                } else {
                    from.node != *node
                        || from.endpoint
                            != OutputEndpoint::GroupMember {
                                group: group.clone(),
                                member: member.clone(),
                            }
                }
            }
            _ => true,
        }
    });
}
