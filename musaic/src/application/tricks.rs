//! Named tile-code references, validated at the authoring boundary.
use crate::{
    application::session::MusaicProject,
    domain::{
        document::{DocumentNodeKind, MusaicDocument},
        tricks::TrickDefinition,
    },
};
use std::collections::BTreeSet;
use tessera::prelude::NodeId;

fn upstream(document: &MusaicDocument, source: &NodeId) -> BTreeSet<NodeId> {
    let edges = crate::domain::document::export_document_program(document)
        .map(|program| crate::domain::document::connection_policy::endpoint_connections(&program))
        .unwrap_or_default();
    let mut seen = BTreeSet::new();
    let mut pending = vec![source.clone()];
    while let Some(node) = pending.pop() {
        if seen.insert(node.clone()) {
            pending.extend(
                edges
                    .iter()
                    .filter(|e| e.to == node)
                    .map(|e| e.from.clone()),
            );
            if let Some(DocumentNodeKind::TrickInstance(t)) =
                document.graph.node(&node).map(|n| &n.kind)
            {
                if let Some(d) = document.tricks.get(&t.prototype.0) {
                    pending.push(d.source.clone());
                }
            }
        }
    }
    seen
}

pub fn define(
    document: &mut MusaicDocument,
    source: NodeId,
    name: String,
    input: Option<NodeId>,
) -> Result<u64, String> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err("Give the trick a name of 1–128 characters".into());
    }
    if !can_define(document, &source) {
        return Err(
            "Select a pattern, value, processing, or Sound tile to save its code as a trick".into(),
        );
    }
    if let Some(input) = &input {
        if input == &source || !upstream(document, &source).contains(input) {
            return Err("The trick input must be an upstream pattern in its code".into());
        }
    }
    if document
        .tricks
        .values()
        .any(|d| d.name.eq_ignore_ascii_case(&name))
    {
        return Err("A trick already has that name".into());
    }
    let id = document
        .tricks
        .keys()
        .next_back()
        .copied()
        .unwrap_or(999)
        .checked_add(1)
        .ok_or("Too many tricks")?;
    document.tricks.insert(
        id,
        TrickDefinition {
            name,
            source,
            input,
        },
    );
    Ok(id)
}

pub fn validate(document: &MusaicDocument) -> Result<(), String> {
    for (id, definition) in &document.tricks {
        if *id < 1000
            || definition.name.trim().is_empty()
            || definition.name.len() > 128
            || definition.name.chars().any(char::is_control)
        {
            return Err("Invalid trick definition".into());
        }
        if !document.graph.contains_node(&definition.source)
            || definition
                .input
                .as_ref()
                .is_some_and(|n| !document.graph.contains_node(n))
        {
            return Err(format!(
                "{} still uses these tiles; keep its source and input",
                definition.name
            ));
        }
        if let Some(input) = &definition.input {
            if input == &definition.source
                || !upstream(document, &definition.source).contains(input)
            {
                return Err(format!("{} has an input outside its code", definition.name));
            }
        }
    }
    for (id, definition) in &document.tricks {
        if upstream(document, &definition.source).iter().any(|node| matches!(document.graph.node(node).map(|n| &n.kind), Some(DocumentNodeKind::TrickInstance(t)) if t.prototype.0 == *id)) { return Err(format!("{} recursively calls itself", definition.name)); }
    }
    for node in document.graph.nodes() {
        if let DocumentNodeKind::TrickInstance(t) = &node.kind {
            if t.prototype.0 >= 1000 && !document.tricks.contains_key(&t.prototype.0) {
                return Err("Trick tile refers to a missing definition".into());
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileCodePaint {
    pub linked_sources: Vec<(u64, String, usize)>,
    pub linked_count: usize,
    pub output_name: Option<String>,
    pub automatic_name: String,
    pub suggested_name: Option<String>,
    pub inputs: Vec<(NodeId, String)>,
    pub definition: Option<(u64, String, NodeId)>,
}
pub fn paint(project: &MusaicProject, node: &NodeId) -> TileCodePaint {
    let document = &project.document;
    if let Some(DocumentNodeKind::Output(output)) = document.graph.node(node).map(|n| &n.kind) {
        return TileCodePaint {
            output_name: Some(output.name.clone()),
            automatic_name: crate::application::outputs::timeline_name(project, node),
            ..Default::default()
        };
    }
    let linked_sources = crate::application::trick_uses::source_owners(document, node);
    if let Some(DocumentNodeKind::TrickInstance(t)) = document.graph.node(node).map(|n| &n.kind) {
        if let Some(d) = document.tricks.get(&t.prototype.0) {
            return TileCodePaint {
                definition: Some((t.prototype.0, d.name.clone(), d.source.clone())),
                linked_count: crate::application::trick_uses::instance_count(
                    document,
                    t.prototype.0,
                ),
                linked_sources,
                ..Default::default()
            };
        }
    }
    if !can_define(document, node) {
        return TileCodePaint {
            linked_sources,
            ..Default::default()
        };
    }
    let inputs = upstream(document, node)
        .into_iter()
        .filter(|id| id != node)
        .filter(|id| can_define(document, id))
        .map(|id| {
            let label = crate::application::editor::tile_inspect_title(
                &crate::domain::document::DocumentQueries::new(document),
                &id,
            );
            (id, label)
        })
        .collect();
    let suggested_name = (1..)
        .map(|n| format!("Trick {n}"))
        .find(|name| !document.tricks.values().any(|d| &d.name == name));
    TileCodePaint {
        linked_sources,
        inputs,
        suggested_name,
        ..Default::default()
    }
}

fn can_define(document: &MusaicDocument, node: &NodeId) -> bool {
    match document.graph.node(node).map(|n| &n.kind) {
        Some(
            DocumentNodeKind::Container(_)
            | DocumentNodeKind::Sound(_)
            | DocumentNodeKind::TrickInstance(_)
            | DocumentNodeKind::FlowControl(_)
            | DocumentNodeKind::Arrangement(_),
        ) => true,
        Some(DocumentNodeKind::Atom(atom)) => {
            atom.atom.numeric_rational().is_some()
                && document
                    .graph
                    .location_of(node)
                    .is_some_and(|l| l.surface == document.root_surface)
        }
        _ => false,
    }
}
