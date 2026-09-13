//! Pure endpoint planning shared by placement, explicit connections, and previews.
use super::{PortSlotState, neighbor_at_side};
use std::collections::BTreeSet;
use tessera::prelude::*;
use thiserror::Error;

mod cable;
mod side;
pub use cable::authorize_manual_connection;
pub use side::authorize_side_cycle;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredEdge {
    /// Explicit cable identity survives the absence of spatial adjacency.
    #[serde(default)]
    pub explicit: bool,
    pub side: SpatialSide,
    pub kind: PortSlotState,
    pub output: OutputEndpoint,
    pub input: InputEndpoint,
    /// Unused endpoints sharing this side must be disabled to keep the source unique.
    pub clear_outputs: Vec<OutputEndpoint>,
    pub clear_inputs: Vec<InputEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EndpointConnection {
    pub from: NodeId,
    pub output: OutputEndpoint,
    pub to: NodeId,
    pub input: InputEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectionPolicyError {
    #[error("choose a north, east, south, or west tile side")]
    InvalidSide,
    #[error("a tile cannot connect to itself")]
    SameNode,
    #[error("both tiles must have root-board placements")]
    MissingPlacement,
    #[error("tiles must be edge-adjacent")]
    NotAdjacent,
    #[error("these tiles have no compatible pattern and value ports")]
    Incompatible,
    #[error("the compatible ports are already connected; existing connections were kept")]
    Occupied,
    #[error("this connection would create a feedback loop")]
    Cycle,
    #[error("this shared edge cannot connect uniquely without changing another connection")]
    Ambiguous,
}

pub fn effective_bindings(program: &AuthoredTesseraProgram, node: &NodeId) -> NodeSpatialBindings {
    let mut bindings = program
        .root_surface
        .bindings
        .get(node)
        .cloned()
        .unwrap_or_else(|| {
            program
                .root_surface
                .nodes
                .get(node)
                .map(default_spatial_bindings)
                .unwrap_or_default()
        });
    if let Some(kind) = program.root_surface.nodes.get(node) {
        if let Some(signature) = signature(kind) {
            for port in &signature.input_sockets {
                bindings
                    .inputs
                    .entry(InputEndpoint::Socket(port.port.clone()))
                    .or_insert(SpatialSide::Off);
            }
            for port in &signature.output_sockets {
                bindings
                    .outputs
                    .entry(OutputEndpoint::Socket(port.port.clone()))
                    .or_insert(SpatialSide::Off);
            }
        }
        if let RootSurfaceNodeKind::FlowControl(flow) = kind {
            for endpoint in crate::domain::flow::inputs(flow) {
                bindings.inputs.entry(endpoint).or_insert(SpatialSide::Off);
            }
            for endpoint in crate::domain::flow::outputs(flow) {
                bindings.outputs.entry(endpoint).or_insert(SpatialSide::Off);
            }
        }
    }
    bindings
}

/// Mirrors spatial inference: each target input needs exactly one facing source.
/// Explicit authored edges also reserve their exact endpoints.
pub fn explicit_connection(relation: &RootRelation) -> EndpointConnection {
    let (from, to, input) = match relation {
        RootRelation::ChainedTo { from, to } => {
            (from, to, InputEndpoint::Socket(InputPort::new("main")))
        }
        RootRelation::FlowsTo { from, to } => match to {
            StreamTarget::OutputInput { node, endpoint }
            | StreamTarget::TransformInput { node, endpoint }
            | StreamTarget::FlowControlInput { node, endpoint } => (from, node, endpoint.clone()),
        },
    };
    EndpointConnection {
        from: from.node.clone(),
        output: from.endpoint.clone(),
        to: to.clone(),
        input,
    }
}

pub fn endpoint_connections(program: &AuthoredTesseraProgram) -> BTreeSet<EndpointConnection> {
    let mut result: BTreeSet<_> = program
        .root_surface
        .explicit_relations
        .iter()
        .map(explicit_connection)
        .collect();
    let explicit_inputs: BTreeSet<_> = result
        .iter()
        .map(|edge| (edge.to.clone(), edge.input.clone()))
        .collect();
    for (to, bindings) in &program.root_surface.bindings {
        for (input, side) in &bindings.inputs {
            if !side.is_enabled() || explicit_inputs.contains(&(to.clone(), input.clone())) {
                continue;
            }
            let Some(from) = neighbor_at_side(program, to, *side) else {
                continue;
            };
            let Some(source) = program.root_surface.bindings.get(&from) else {
                continue;
            };
            let outputs: Vec<_> = source
                .outputs
                .iter()
                .filter(|(_, bound)| **bound == side.opposite())
                .map(|(port, _)| port.clone())
                .collect();
            if let [output] = outputs.as_slice() {
                result.insert(EndpointConnection {
                    from,
                    output: output.clone(),
                    to: to.clone(),
                    input: input.clone(),
                });
            }
        }
    }
    result
}

fn signature(node: &RootSurfaceNodeKind) -> Option<&NodeSignature> {
    match node {
        RootSurfaceNodeKind::Transform(t) => Some(&t.signature),
        RootSurfaceNodeKind::FlowControl(t) => Some(&t.signature),
        RootSurfaceNodeKind::Output(t) => Some(&t.signature),
        _ => None,
    }
}
fn input_shape(node: &RootSurfaceNodeKind, endpoint: &InputEndpoint) -> Option<StreamShape> {
    let signature = signature(node)?;
    match endpoint {
        InputEndpoint::Socket(port) => signature.input_socket(port).map(|p| p.shape),
        InputEndpoint::GroupMember { group, .. } => signature.input_group(group).map(|p| p.shape),
    }
}

pub fn endpoints_compatible(program: &AuthoredTesseraProgram, edge: &EndpointConnection) -> bool {
    let Some(source) =
        TesseraCompiler::new().authored_output_shape(program, &edge.from, &edge.output)
    else {
        return false;
    };
    let Some(target) = program
        .root_surface
        .nodes
        .get(&edge.to)
        .and_then(|node| input_shape(node, &edge.input))
    else {
        return false;
    };
    TesseraCompiler::stream_shape_compatible(source, target)
}

pub fn apply_edge(
    program: &mut AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
    edge: &AuthoredEdge,
) {
    let mut source = effective_bindings(program, from);
    for port in &edge.clear_outputs {
        source.outputs.insert(port.clone(), SpatialSide::Off);
    }
    source.outputs.insert(edge.output.clone(), edge.side);
    let mut target = effective_bindings(program, to);
    for port in &edge.clear_inputs {
        target.inputs.insert(port.clone(), SpatialSide::Off);
    }
    target
        .inputs
        .insert(edge.input.clone(), edge.side.opposite());
    program.root_surface.bindings.insert(from.clone(), source);
    program.root_surface.bindings.insert(to.clone(), target);
    if edge.explicit {
        let endpoint = edge.input.clone();
        let target = match program.root_surface.nodes.get(to) {
            Some(RootSurfaceNodeKind::Output(_)) => StreamTarget::OutputInput {
                node: to.clone(),
                endpoint,
            },
            Some(RootSurfaceNodeKind::Transform(_)) => StreamTarget::TransformInput {
                node: to.clone(),
                endpoint,
            },
            Some(RootSurfaceNodeKind::FlowControl(_)) => StreamTarget::FlowControlInput {
                node: to.clone(),
                endpoint,
            },
            _ => return,
        };
        let relation = RootRelation::FlowsTo {
            from: StreamSource {
                node: from.clone(),
                endpoint: edge.output.clone(),
            },
            to: target,
        };
        if !program.root_surface.explicit_relations.contains(&relation) {
            program.root_surface.explicit_relations.push(relation);
        }
    }
}

/// Selects a compatible pair of free endpoints. This never mutates the board.
/// Facing endpoints are preferred, then disabled endpoints, then unused sides.
pub fn authorize_connection(
    program: &AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
) -> Result<AuthoredEdge, ConnectionPolicyError> {
    if from == to {
        return Err(ConnectionPolicyError::SameNode);
    }
    if !program.root_surface.placements.contains_key(from)
        || !program.root_surface.placements.contains_key(to)
    {
        return Err(ConnectionPolicyError::MissingPlacement);
    }
    let side = [
        SpatialSide::North,
        SpatialSide::South,
        SpatialSide::West,
        SpatialSide::East,
    ]
    .into_iter()
    .find(|side| neighbor_at_side(program, to, side.opposite()).as_ref() == Some(from))
    .ok_or(ConnectionPolicyError::NotAdjacent)?;
    authorize_on_side(program, from, to, side, false)
}

fn authorize_on_side(
    program: &AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
    side: SpatialSide,
    explicit: bool,
) -> Result<AuthoredEdge, ConnectionPolicyError> {
    let source = effective_bindings(program, from);
    let target = effective_bindings(program, to);
    let target_kind = program
        .root_surface
        .nodes
        .get(to)
        .ok_or(ConnectionPolicyError::MissingPlacement)?;
    let compiler = TesseraCompiler::new();
    let inputs = match target_kind {
        RootSurfaceNodeKind::FlowControl(flow) => crate::domain::flow::inputs(flow),
        _ => target.inputs.keys().cloned().collect(),
    };
    let outputs = match program.root_surface.nodes.get(from) {
        Some(RootSurfaceNodeKind::FlowControl(flow)) => crate::domain::flow::outputs(flow),
        _ => source.outputs.keys().cloned().collect(),
    };
    let connected = endpoint_connections(program);
    let output_used = |port: &OutputEndpoint| {
        connected
            .iter()
            .any(|c| &c.from == from && &c.output == port)
    };
    let input_used =
        |port: &InputEndpoint| connected.iter().any(|c| &c.to == to && &c.input == port);
    let rank = |bound: SpatialSide, desired: SpatialSide| {
        if bound == desired {
            0
        } else if bound == SpatialSide::Off {
            1
        } else {
            2
        }
    };
    let mut pairs = Vec::new();
    for (out_order, output) in outputs.iter().enumerate() {
        let bound_out = source
            .outputs
            .get(output)
            .copied()
            .unwrap_or(SpatialSide::Off);
        let Some(shape) = compiler.authored_output_shape(program, from, output) else {
            continue;
        };
        for (in_order, input) in inputs.iter().enumerate() {
            let bound_in = target
                .inputs
                .get(input)
                .copied()
                .unwrap_or(SpatialSide::Off);
            let Some(wanted) = input_shape(target_kind, input) else {
                continue;
            };
            if TesseraCompiler::stream_shape_compatible(shape, wanted) {
                // Empty musical containers should prefer a pattern input to an amount.
                let unknown_amount = usize::from(
                    shape == StreamShape::Any
                        && matches!(
                            wanted,
                            StreamShape::ScalarPattern | StreamShape::ControlPattern
                        ),
                );
                pairs.push((
                    unknown_amount,
                    rank(bound_out, side) + rank(bound_in, side.opposite()),
                    out_order,
                    in_order,
                    output.clone(),
                    input.clone(),
                    shape,
                ));
            }
        }
    }
    pairs.sort_by(|a, b| (&a.0, &a.1, &a.2, &a.3).cmp(&(&b.0, &b.1, &b.2, &b.3)));
    if pairs.is_empty() {
        return Err(ConnectionPolicyError::Incompatible);
    }
    let mut failure = ConnectionPolicyError::Occupied;
    for (_, _, _, _, output, input, shape) in pairs {
        let exact = EndpointConnection {
            from: from.clone(),
            output: output.clone(),
            to: to.clone(),
            input: input.clone(),
        };
        let already = connected.contains(&exact);
        if !already && (output_used(&output) || input_used(&input)) {
            continue;
        }
        let clear_outputs: Vec<_> = source
            .outputs
            .iter()
            .filter(|(port, bound)| !explicit && **bound == side && **port != output)
            .map(|(p, _)| p.clone())
            .collect();
        let clear_inputs: Vec<_> = target
            .inputs
            .iter()
            .filter(|(port, bound)| !explicit && **bound == side.opposite() && **port != input)
            .map(|(p, _)| p.clone())
            .collect();
        if clear_outputs.iter().any(output_used) || clear_inputs.iter().any(input_used) {
            continue;
        }
        let edge = AuthoredEdge {
            explicit,
            side,
            kind: if matches!(
                shape,
                StreamShape::ScalarPattern | StreamShape::ControlPattern
            ) {
                PortSlotState::Input
            } else {
                PortSlotState::Output
            },
            output,
            input,
            clear_outputs,
            clear_inputs,
        };
        let mut candidate = program.clone();
        apply_edge(&mut candidate, from, to, &edge);
        let after = endpoint_connections(&candidate);
        if !connected.is_subset(&after)
            || after.difference(&connected).any(|c| c != &exact)
            || !after.contains(&exact)
        {
            failure = ConnectionPolicyError::Ambiguous;
            continue;
        }
        if !already && path_exists(&connected, to, from) {
            failure = ConnectionPolicyError::Cycle;
            continue;
        }
        return Ok(edge);
    }
    Err(failure)
}
fn path_exists(edges: &BTreeSet<EndpointConnection>, from: &NodeId, target: &NodeId) -> bool {
    let mut pending = vec![from.clone()];
    let mut seen = BTreeSet::new();
    while let Some(node) = pending.pop() {
        if &node == target {
            return true;
        }
        if !seen.insert(node.clone()) {
            continue;
        }
        pending.extend(
            edges
                .iter()
                .filter(|c| c.from == node)
                .map(|c| c.to.clone()),
        );
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn program_with_output_at(x: i32, y: i32) -> AuthoredTesseraProgram {
        let mut b = Board::new();
        b.at(0, 0)
            .named("container")
            .footprint(TileFootprint::new(2, 2))
            .sequence(SequenceStack::new().build())
            .unwrap();
        b.at(x, y)
            .named("output")
            .footprint(TileFootprint::new(2, 2))
            .output()
            .unwrap();
        b.finish()
    }
    #[test]
    fn authorizes_a_2x2_root_edge_neighbor() {
        let edge = authorize_connection(
            &program_with_output_at(2, 0),
            &NodeId::new("container"),
            &NodeId::new("output"),
        )
        .unwrap();
        assert_eq!(edge.side, SpatialSide::East);
        assert_eq!(edge.kind, PortSlotState::Output);
    }
    #[test]
    fn rejects_distant_root_tiles() {
        assert_eq!(
            authorize_connection(
                &program_with_output_at(3, 0),
                &NodeId::new("container"),
                &NodeId::new("output")
            ),
            Err(ConnectionPolicyError::NotAdjacent)
        );
    }
    #[test]
    fn rejects_a_connection_to_self() {
        assert_eq!(
            authorize_connection(
                &program_with_output_at(2, 0),
                &NodeId::new("container"),
                &NodeId::new("container")
            ),
            Err(ConnectionPolicyError::SameNode)
        );
    }
}
