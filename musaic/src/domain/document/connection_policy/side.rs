//! A side control edits declared endpoint bindings, never only its display marker.
use super::*;

pub fn authorize_side_cycle(
    program: &AuthoredTesseraProgram,
    node: &NodeId,
    side: SpatialSide,
) -> Result<AuthoredTesseraProgram, ConnectionPolicyError> {
    if !side.is_enabled() {
        return Err(ConnectionPolicyError::InvalidSide);
    }
    let kind = program
        .root_surface
        .nodes
        .get(node)
        .filter(|_| program.root_surface.placements.contains_key(node))
        .ok_or(ConnectionPolicyError::MissingPlacement)?;
    let declared = effective_bindings(program, node);
    let current = program
        .root_surface
        .bindings
        .get(node)
        .cloned()
        .unwrap_or_default();
    let state = crate::domain::document::port_state_for_side(
        &crate::domain::flow::port_config(&current),
        side,
    );
    let before = endpoint_connections(program);
    // Roles whose endpoints are all occupied on other sides are unavailable.
    // Skipping them lets a fully connected input keep its role while a free
    // output is assigned on another side, and still lets an input be disabled.
    let free_input = declared.inputs.keys().any(|endpoint| {
        current.inputs.get(endpoint) == Some(&side)
            || !before
                .iter()
                .any(|edge| &edge.to == node && &edge.input == endpoint)
    });
    let free_output = declared.outputs.keys().any(|endpoint| {
        current.outputs.get(endpoint) == Some(&side)
            || !before
                .iter()
                .any(|edge| &edge.from == node && &edge.output == endpoint)
    });
    let roles: Vec<_> = match state {
        PortSlotState::None => [
            (free_input, PortSlotState::Input),
            (free_output, PortSlotState::Output),
        ]
        .into_iter()
        .filter_map(|(free, role)| free.then_some(role))
        .collect(),
        PortSlotState::Input => {
            if free_output {
                vec![PortSlotState::Output, PortSlotState::None]
            } else {
                vec![PortSlotState::None]
            }
        }
        PortSlotState::Output => vec![PortSlotState::None],
    };
    let removable: BTreeSet<_> = before
        .iter()
        .filter(|edge| {
            (&edge.from == node && current.outputs.get(&edge.output) == Some(&side))
                || (&edge.to == node && current.inputs.get(&edge.input) == Some(&side))
        })
        .cloned()
        .collect();
    let mut bindings = current;
    for endpoint in declared.inputs.keys() {
        bindings
            .inputs
            .entry(endpoint.clone())
            .or_insert(SpatialSide::Off);
    }
    for endpoint in declared.outputs.keys() {
        bindings
            .outputs
            .entry(endpoint.clone())
            .or_insert(SpatialSide::Off);
    }
    for bound in bindings
        .inputs
        .values_mut()
        .chain(bindings.outputs.values_mut())
    {
        if *bound == side {
            *bound = SpatialSide::Off;
        }
    }
    let mut failure = ConnectionPolicyError::Occupied;
    for next in roles {
        let mut bindings = bindings.clone();
        let attempt = (|| -> Result<AuthoredTesseraProgram, ConnectionPolicyError> {
            let mut candidate = program.clone();
            candidate
                .root_surface
                .explicit_relations
                .retain(|relation| !removable.contains(&explicit_connection(relation)));
            // Disabling a source also disables its exact spatial destination inputs.
            // Otherwise Tessera sees a dangling enabled input and cannot compile silence.
            for edge in removable.iter().filter(|edge| &edge.from == node) {
                if let Some(target) = candidate.root_surface.bindings.get_mut(&edge.to) {
                    target.inputs.insert(edge.input.clone(), SpatialSide::Off);
                }
            }
            candidate
                .root_surface
                .bindings
                .insert(node.clone(), bindings.clone());
            if next != PortSlotState::None {
                if let Some(neighbor) = neighbor_at_side(program, node, side) {
                    let (from, to) = if next == PortSlotState::Output {
                        (node, &neighbor)
                    } else {
                        (&neighbor, node)
                    };
                    let edge = authorize_connection(&candidate, from, to)?;
                    let requested_side = if next == PortSlotState::Output {
                        side
                    } else {
                        side.opposite()
                    };
                    if edge.side != requested_side {
                        return Err(ConnectionPolicyError::Ambiguous);
                    }
                    apply_edge(&mut candidate, from, to, &edge);
                } else if next == PortSlotState::Output {
                    let mut endpoints = match kind {
                        RootSurfaceNodeKind::FlowControl(flow) => {
                            crate::domain::flow::outputs(flow)
                        }
                        _ => bindings.outputs.keys().cloned().collect(),
                    };
                    endpoints.sort_by_key(|endpoint| {
                        bindings.outputs.get(endpoint) != Some(&SpatialSide::Off)
                    });
                    let endpoint = endpoints
                        .into_iter()
                        .find(|endpoint| {
                            !before
                                .iter()
                                .any(|edge| &edge.from == node && &edge.output == endpoint)
                        })
                        .ok_or(ConnectionPolicyError::Occupied)?;
                    bindings.outputs.insert(endpoint, side);
                    candidate
                        .root_surface
                        .bindings
                        .insert(node.clone(), bindings);
                } else {
                    let mut endpoints = match kind {
                        RootSurfaceNodeKind::FlowControl(flow) => crate::domain::flow::inputs(flow),
                        _ => bindings.inputs.keys().cloned().collect(),
                    };
                    endpoints.sort_by_key(|endpoint| {
                        bindings.inputs.get(endpoint) != Some(&SpatialSide::Off)
                    });
                    let endpoint = endpoints
                        .into_iter()
                        .find(|endpoint| {
                            !before
                                .iter()
                                .any(|edge| &edge.to == node && &edge.input == endpoint)
                        })
                        .ok_or(ConnectionPolicyError::Occupied)?;
                    bindings.inputs.insert(endpoint, side);
                    candidate
                        .root_surface
                        .bindings
                        .insert(node.clone(), bindings);
                }
            }
            let after = endpoint_connections(&candidate);
            if before
                .difference(&after)
                .any(|edge| !removable.contains(edge))
            {
                return Err(ConnectionPolicyError::Occupied);
            }
            if removable.iter().any(|edge| after.contains(edge)) {
                // Explicit non-spatial relations cannot be disabled by a side button.
                return Err(ConnectionPolicyError::Occupied);
            }
            for edge in after.difference(&before) {
                if !endpoints_compatible(&candidate, edge) {
                    return Err(ConnectionPolicyError::Incompatible);
                }
                if path_exists(&after, &edge.to, &edge.from) {
                    return Err(ConnectionPolicyError::Cycle);
                }
            }
            Ok(candidate)
        })();
        match attempt {
            Ok(candidate) => return Ok(candidate),
            Err(error) => failure = error,
        }
    }
    Err(failure)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(name: &str) -> NodeId {
        NodeId::new(name)
    }
    fn output() -> OutputEndpoint {
        OutputEndpoint::Socket(OutputPort::new("out"))
    }
    fn input() -> InputEndpoint {
        InputEndpoint::GroupMember {
            group: PortGroupId::new("inputs"),
            member: PortMemberId::new("main"),
        }
    }
    fn branch(name: &str) -> OutputEndpoint {
        OutputEndpoint::GroupMember {
            group: PortGroupId::new("branches"),
            member: PortMemberId::new(name),
        }
    }
    #[test]
    fn source_sink_and_transform_sides_use_only_free_supported_roles() {
        let mut board = Board::new();
        board
            .at(0, 0)
            .named("source")
            .sequence(SequenceStack::new().note("c").build())
            .unwrap();
        board.at(5, 0).named("sink").output().unwrap();
        board
            .at(10, 0)
            .named("transform")
            .transform(TransformKind::Gain)
            .unwrap();
        let mut program = board.finish();
        for (node, states) in [
            (
                id("source"),
                vec![PortSlotState::Output, PortSlotState::None],
            ),
            (id("sink"), vec![PortSlotState::Input, PortSlotState::None]),
            (
                id("transform"),
                vec![
                    PortSlotState::Input,
                    PortSlotState::Output,
                    PortSlotState::None,
                ],
            ),
        ] {
            for expected in states {
                program = authorize_side_cycle(&program, &node, SpatialSide::South).unwrap();
                let actual = crate::domain::document::port_state_for_side(
                    &crate::domain::flow::port_config(&program.root_surface.bindings[&node]),
                    SpatialSide::South,
                );
                assert_eq!(actual, expected);
            }
        }
        assert!(
            program.root_surface.bindings[&id("source")]
                .inputs
                .is_empty()
        );
        assert!(
            program.root_surface.bindings[&id("sink")]
                .outputs
                .is_empty()
        );
    }
    #[test]
    fn occupied_named_endpoints_stay_bound_and_a_free_branch_can_be_enabled() {
        let mut board = Board::new();
        board
            .at(-1, 0)
            .named("notes")
            .sequence(SequenceStack::new().note("c").build())
            .unwrap();
        board
            .at(0, 0)
            .named("split")
            .flow_control_node(FlowControlNode::new(FlowControlKind::Split))
            .unwrap();
        board.at(1, 0).named("sink").output().unwrap();
        let mut before = board.finish();
        let split = id("split");
        before
            .root_surface
            .bindings
            .get_mut(&split)
            .unwrap()
            .outputs = std::collections::BTreeMap::from([
            (branch("even"), SpatialSide::East),
            (branch("odd"), SpatialSide::Off),
        ]);
        let after = authorize_side_cycle(&before, &split, SpatialSide::North).unwrap();
        assert_eq!(
            after.root_surface.bindings[&split].outputs[&branch("odd")],
            SpatialSide::North
        );
        assert_eq!(
            after.root_surface.bindings[&split].outputs[&branch("even")],
            SpatialSide::East
        );
        assert_eq!(
            after.root_surface.bindings[&split].inputs,
            before.root_surface.bindings[&split].inputs
        );
        assert_eq!(endpoint_connections(&after), endpoint_connections(&before));
        // A single source has no second endpoint to move onto a new side.
        assert_eq!(
            authorize_side_cycle(&before, &id("notes"), SpatialSide::South).unwrap_err(),
            ConnectionPolicyError::Occupied
        );
    }
    #[test]
    fn incompatible_neighbor_rejects_without_changing_any_binding() {
        let mut board = Board::new();
        board
            .at(0, 0)
            .named("number")
            .scalar(Rational::one())
            .unwrap();
        board.at(1, 0).named("sink").output().unwrap();
        let mut program = board.finish();
        program
            .root_surface
            .bindings
            .get_mut(&id("number"))
            .unwrap()
            .outputs
            .insert(output(), SpatialSide::Off);
        assert_eq!(
            authorize_side_cycle(&program, &id("number"), SpatialSide::East).unwrap_err(),
            ConnectionPolicyError::Incompatible
        );
        assert_eq!(
            program.root_surface.bindings[&id("number")].outputs[&output()],
            SpatialSide::Off
        );
        assert_eq!(
            program.root_surface.bindings[&id("sink")].inputs[&input()],
            SpatialSide::West
        );
        assert!(endpoint_connections(&program).is_empty());
    }
}
