//! Inspectable connection plans; commit reuses the existing connection transaction.
use crate::{
    application::{
        editor::{EditorAttention, SelectionState},
        session::MusaicProject,
    },
    domain::document::{self, AuthoredEdge, MusaicDocument, NodeLocation},
};
use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionPlan {
    pub from: NodeId,
    pub to: NodeId,
    pub edge: AuthoredEdge,
    pub source_location: NodeLocation,
    pub target_location: NodeLocation,
}

#[derive(bevy::prelude::Message, Debug, Clone)]
pub struct ConnectionReceipt {
    pub request: u64,
    pub result: Result<(), String>,
}

pub fn plan(
    document: &MusaicDocument,
    from: &NodeId,
    to: &NodeId,
) -> Result<ConnectionPlan, String> {
    let source_location = document
        .graph
        .location_of(from)
        .ok_or("The source tile no longer exists")?;
    let target_location = document
        .graph
        .location_of(to)
        .ok_or("The destination tile no longer exists")?;
    if source_location.surface != document.root_surface
        || target_location.surface != document.root_surface
    {
        return Err(
            "Connect tiles on the pattern board; notes inside a container form its pattern".into(),
        );
    }
    let program = document::export_document_program(document)
        .map_err(|error| format!("Cannot export this document: {error:?}"))?;
    if document::connection_exists(&program, from, to) {
        return Err("That connection already exists.".into());
    }
    let edge = document::connection_policy::authorize_manual_connection(&program, from, to)
        .map_err(|e| e.to_string())?;
    Ok(ConnectionPlan {
        from: from.clone(),
        to: to.clone(),
        edge,
        source_location,
        target_location,
    })
}

/// Called on opening or document change, never as a per-frame board scan.
pub fn destinations(document: &MusaicDocument, source: &NodeId) -> Vec<ConnectionPlan> {
    document
        .graph
        .nodes_on_surface(document.root_surface)
        .into_iter()
        .map(|(_, node)| &node.id)
        .filter(|target| *target != source)
        .filter_map(|target| plan(document, source, target).ok())
        .collect()
}

pub(super) fn apply(
    project: &mut MusaicProject,
    attention: &EditorAttention,
    expected: &ConnectionPlan,
) -> Result<Vec<NodeId>, String> {
    let current = plan(&project.document, &expected.from, &expected.to)?;
    if current != *expected {
        return Err("The route changed. Review the destination and connect again.".into());
    }
    let result = crate::application::editor::transaction::connect_tiles(
        &mut project.document,
        &mut attention.clone(),
        &mut SelectionState::default(),
        expected.from.clone(),
        expected.to.clone(),
    );
    if !result.is_accepted() {
        return Err(result
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join(" · "));
    }
    Ok(vec![expected.to.clone()])
}
