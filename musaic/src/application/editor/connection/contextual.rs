//! Contextual connection planning is pure; committing it updates canonical bindings.
use crate::application::editor::{DiagnosticSeverity, EditorTransactionDiagnostic};
use crate::domain::document::connection_policy::{
    self as policy, AuthoredEdge, ConnectionPolicyError, EndpointConnection,
};
use crate::domain::document::{self, MusaicDocument};
use std::collections::{BTreeMap, BTreeSet};
use tessera::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextualConnectionPlan {
    pub bindings: BTreeMap<NodeId, NodeSpatialBindings>,
    pub explicit_relations: Vec<RootRelation>,
    pub added: Vec<EndpointConnection>,
    pub feedback: Vec<String>,
}
fn adjacent(program: &AuthoredTesseraProgram, a: &NodeId, b: &NodeId) -> bool {
    [
        SpatialSide::West,
        SpatialSide::North,
        SpatialSide::East,
        SpatialSide::South,
    ]
    .into_iter()
    .any(|side| {
        document::neighbor_at_side(program, a, side).as_ref() == Some(b)
            || document::neighbor_at_side(program, b, side).as_ref() == Some(a)
    })
}
fn clear_node(program: &mut AuthoredTesseraProgram, node: &NodeId) {
    let mut bindings = policy::effective_bindings(program, node);
    for side in bindings
        .inputs
        .values_mut()
        .chain(bindings.outputs.values_mut())
    {
        *side = SpatialSide::Off;
    }
    program.root_surface.bindings.insert(node.clone(), bindings);
}
fn hint_rank(
    program: &AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
    edge: &AuthoredEdge,
) -> usize {
    let rank =
        |actual: Option<&SpatialSide>, desired| usize::from(actual.copied() != Some(desired));
    rank(
        policy::effective_bindings(program, from)
            .outputs
            .get(&edge.output),
        edge.side,
    ) + rank(
        policy::effective_bindings(program, to)
            .inputs
            .get(&edge.input),
        edge.side.opposite(),
    )
}

fn restore_existing_endpoint(
    program: &mut AuthoredTesseraProgram,
    old: &EndpointConnection,
    side: SpatialSide,
) {
    if !policy::endpoints_compatible(program, old) {
        return;
    }
    let connected = policy::endpoint_connections(program);
    let source = policy::effective_bindings(program, &old.from);
    let target = policy::effective_bindings(program, &old.to);
    if !source.outputs.contains_key(&old.output) || !target.inputs.contains_key(&old.input) {
        return;
    }
    let output_used = |port: &OutputEndpoint| {
        connected
            .iter()
            .any(|edge| edge.from == old.from && &edge.output == port)
    };
    let input_used = |port: &InputEndpoint| {
        connected
            .iter()
            .any(|edge| edge.to == old.to && &edge.input == port)
    };
    if output_used(&old.output) && source.outputs.get(&old.output) != Some(&side) {
        return;
    }
    if input_used(&old.input) && !connected.contains(old) {
        return;
    }
    let clear_outputs: Vec<_> = source
        .outputs
        .iter()
        .filter(|(port, bound)| **port != old.output && **bound == side)
        .map(|(port, _)| port.clone())
        .collect();
    let clear_inputs: Vec<_> = target
        .inputs
        .iter()
        .filter(|(port, bound)| {
            **port != old.input
                && **bound == side.opposite()
                && !connected.iter().any(|edge| {
                    edge.from == old.from
                        && edge.output == old.output
                        && edge.to == old.to
                        && &edge.input == *port
                })
        })
        .map(|(port, _)| port.clone())
        .collect();
    if clear_outputs.iter().any(output_used) || clear_inputs.iter().any(input_used) {
        return;
    }
    let edge = AuthoredEdge {
        explicit: false,
        side,
        kind: document::PortSlotState::Output,
        output: old.output.clone(),
        input: old.input.clone(),
        clear_outputs,
        clear_inputs,
    };
    let mut trial = program.clone();
    policy::apply_edge(&mut trial, &old.from, &old.to, &edge);
    let after = policy::endpoint_connections(&trial);
    if connected.is_subset(&after)
        && after.contains(old)
        && after.difference(&connected).all(|edge| edge == old)
    {
        *program = trial;
    }
}

/// Preview placement or movement without modifying source documents. Existing
/// edges outside the affected tiles are protected; compatible surviving edges
/// keep their exact named endpoints. New edges use only compatible free ports.
pub fn plan_contextual_connections(
    previous: &AuthoredTesseraProgram,
    current: &AuthoredTesseraProgram,
    affected: &[NodeId],
    preferred: Option<&NodeId>,
) -> Result<ContextualConnectionPlan, String> {
    let affected: BTreeSet<_> = affected
        .iter()
        .filter(|n| current.root_surface.placements.contains_key(*n))
        .cloned()
        .collect();
    let before = policy::endpoint_connections(previous);
    let protected: BTreeSet<_> = before
        .iter()
        .filter(|c| !affected.contains(&c.from) && !affected.contains(&c.to))
        .cloned()
        .collect();
    let mut candidate = current.clone();
    for node in &affected {
        clear_node(&mut candidate, node);
    }
    // A disconnected destination must not latch onto a different tile when its source moves.
    for edge in &before {
        if affected.contains(&edge.from) && !affected.contains(&edge.to) {
            let mut binding = policy::effective_bindings(&candidate, &edge.to);
            binding.inputs.insert(edge.input.clone(), SpatialSide::Off);
            candidate
                .root_surface
                .bindings
                .insert(edge.to.clone(), binding);
        }
    }
    candidate
        .root_surface
        .explicit_relations
        .retain(|relation| {
            let edge = policy::explicit_connection(relation);
            !affected.contains(&edge.from) && !affected.contains(&edge.to)
        });
    // Deliberate cables keep their exact endpoint identities when either tile moves.
    // Preserve their authored sides; only automatic spatial connections follow adjacency.
    for relation in &previous.root_surface.explicit_relations {
        let old = policy::explicit_connection(relation);
        if !(affected.contains(&old.from) || affected.contains(&old.to)) {
            continue;
        }
        if !candidate.root_surface.nodes.contains_key(&old.from)
            || !candidate.root_surface.nodes.contains_key(&old.to)
        {
            continue;
        }
        if !policy::endpoints_compatible(&candidate, &old) {
            return Err("This edit would invalidate an existing cable's endpoint type.".into());
        }
        let mut source = policy::effective_bindings(&candidate, &old.from);
        let mut target = policy::effective_bindings(&candidate, &old.to);
        let old_source = policy::effective_bindings(previous, &old.from);
        let old_target = policy::effective_bindings(previous, &old.to);
        if let Some(side) = old_source.outputs.get(&old.output) {
            source.outputs.insert(old.output.clone(), *side);
        }
        if let Some(side) = old_target.inputs.get(&old.input) {
            target.inputs.insert(old.input.clone(), *side);
        }
        candidate.root_surface.bindings.insert(old.from, source);
        candidate.root_surface.bindings.insert(old.to, target);
        candidate
            .root_surface
            .explicit_relations
            .push(relation.clone());
    }
    // Preserve the exact named role when a connected pair remains adjacent,
    // including when a tile moves around the other tile to a different side.
    // Existing shared sides win when one output cannot serve two new sides.
    let mut surviving = Vec::new();
    for old in before
        .iter()
        .filter(|c| affected.contains(&c.from) || affected.contains(&c.to))
    {
        if candidate
            .root_surface
            .explicit_relations
            .iter()
            .any(|relation| policy::explicit_connection(relation) == *old)
        {
            continue;
        }
        let old_source = policy::effective_bindings(previous, &old.from);
        let Some(side) = old_source.outputs.get(&old.output).copied() else {
            continue;
        };
        let shared = [
            side,
            SpatialSide::West,
            SpatialSide::North,
            SpatialSide::East,
            SpatialSide::South,
        ]
        .into_iter()
        .find(|side| {
            document::neighbor_at_side(&candidate, &old.to, side.opposite()).as_ref()
                == Some(&old.from)
        });
        if let Some(shared) = shared {
            surviving.push((usize::from(shared != side), old, shared));
        }
    }
    surviving.sort_by_key(|(rank, _, _)| *rank);
    for (_, old, side) in surviving {
        restore_existing_endpoint(&mut candidate, old, side);
    }
    if !protected.is_subset(&policy::endpoint_connections(&candidate)) {
        return Err("This position would block an existing connection on a shared edge. Choose a free edge.".into());
    }
    if !policy::endpoint_connections(&candidate).is_subset(&before) {
        return Err(
            "This position would redirect a connection on a shared edge. Choose a free edge."
                .into(),
        );
    }
    let mut feedback = Vec::new();
    let mut visited = BTreeSet::new();
    for node in &affected {
        let mut neighbors: Vec<_> = candidate
            .root_surface
            .placements
            .keys()
            .filter(|other| *other != node && adjacent(&candidate, node, other))
            .cloned()
            .collect();
        neighbors.sort_by_key(|other| (usize::from(Some(other) != preferred), other.clone()));
        for other in neighbors {
            let mut pair = [node.clone(), other.clone()];
            pair.sort();
            if !visited.insert(pair) {
                continue;
            }
            let existing = policy::endpoint_connections(&candidate);
            if existing
                .iter()
                .any(|c| (&c.from == node && c.to == other) || (c.from == other && &c.to == node))
            {
                continue;
            }
            let directions = [(&other, node), (node, &other)];
            let mut options = Vec::new();
            let mut errors = Vec::new();
            for (from, to) in directions {
                match policy::authorize_connection(&candidate, from, to) {
                    Ok(edge) => options.push((
                        hint_rank(current, from, to, &edge),
                        from.clone(),
                        to.clone(),
                        edge,
                    )),
                    Err(error) => errors.push(error),
                }
            }
            options.sort_by_key(|o| o.0);
            if let Some((_, from, to, edge)) = options.into_iter().next() {
                policy::apply_edge(&mut candidate, &from, &to, &edge);
            } else {
                let error = errors
                    .iter()
                    .find(|e| {
                        matches!(
                            e,
                            ConnectionPolicyError::Occupied
                                | ConnectionPolicyError::Ambiguous
                                | ConnectionPolicyError::Cycle
                        )
                    })
                    .or_else(|| {
                        errors
                            .iter()
                            .find(|e| **e != ConnectionPolicyError::NotAdjacent)
                    });
                if let Some(error) = error {
                    feedback.push(format!("No automatic connection: {error}."));
                }
            }
        }
    }
    // Keep unused authored directions whenever they remain harmless. Existing
    // tiles retain their saved settings; new tiles retain safe initial hints.
    for node in &affected {
        let hints = if previous.root_surface.nodes.contains_key(node) {
            previous
        } else {
            current
        };
        let old = policy::effective_bindings(hints, node);
        for (input, side) in old.inputs {
            let occupied = policy::endpoint_connections(&candidate);
            if occupied
                .iter()
                .any(|edge| &edge.to == node && edge.input == input)
            {
                continue;
            }
            if side.is_enabled() && document::neighbor_at_side(&candidate, node, side).is_some() {
                continue;
            }
            if let Some(bindings) = candidate.root_surface.bindings.get_mut(node) {
                if bindings.inputs.contains_key(&input) {
                    bindings.inputs.insert(input, side);
                }
            }
        }
        for (output, side) in old.outputs {
            let occupied = policy::endpoint_connections(&candidate);
            if occupied
                .iter()
                .any(|edge| &edge.from == node && edge.output == output)
            {
                continue;
            }
            let mut trial = candidate.clone();
            if let Some(bindings) = trial.root_surface.bindings.get_mut(node) {
                if !bindings.outputs.contains_key(&output) {
                    continue;
                }
                bindings.outputs.insert(output, side);
            }
            if policy::endpoint_connections(&trial) == occupied {
                candidate = trial;
            }
        }
    }
    let after = policy::endpoint_connections(&candidate);
    if !protected.is_subset(&after) {
        return Err("The proposed connection would change another tile's wiring.".into());
    }
    feedback.sort();
    feedback.dedup();
    Ok(ContextualConnectionPlan {
        bindings: candidate.root_surface.bindings,
        explicit_relations: candidate.root_surface.explicit_relations,
        added: after.difference(&before).cloned().collect(),
        feedback,
    })
}

/// Call after a root placement or movement and before recording history.
pub fn reconnect_after_edit(
    document: &mut MusaicDocument,
    previous: &AuthoredTesseraProgram,
    affected: &[NodeId],
    preferred: Option<&NodeId>,
) -> Result<Vec<EditorTransactionDiagnostic>, String> {
    let mut program = document::export_document_program(document)
        .map_err(|error| format!("Cannot export this document: {error:?}"))?;
    let plan = plan_contextual_connections(previous, &program, affected, preferred)?;
    program.root_surface.bindings = plan.bindings;
    program.root_surface.explicit_relations = plan.explicit_relations;
    document.replace_connections_from(&program);
    document.validate()?;
    Ok(plan
        .feedback
        .into_iter()
        .map(|message| EditorTransactionDiagnostic {
            severity: DiagnosticSeverity::Info,
            message,
        })
        .collect())
}
