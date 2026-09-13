//! Outputs name timeline lanes; all sound processing belongs to upstream tiles.
use crate::{
    application::session::MusaicProject,
    domain::document::{DocumentNodeKind, MusaicDocument},
};
use tessera::prelude::NodeId;

fn is_sound(document: &MusaicDocument, node: &NodeId) -> bool {
    matches!(
        document.graph.node(node).map(|n| &n.kind),
        Some(DocumentNodeKind::Sound(_))
    )
}

pub fn validate(document: &MusaicDocument) -> Result<(), String> {
    let count = document
        .graph
        .nodes()
        .filter(|n| matches!(n.kind, DocumentNodeKind::Output(_)))
        .count();
    if document.channels == 0 || count > document.channels as usize {
        return Err(format!(
            "This project allows {} output tiles; it contains {count}",
            document.channels
        ));
    }
    Ok(())
}

pub fn timeline_name(project: &MusaicProject, output: &NodeId) -> String {
    if let Some(DocumentNodeKind::Output(value)) =
        project.document.graph.node(output).map(|n| &n.kind)
    {
        if !value.name.trim().is_empty() {
            return value.name.clone();
        }
    }
    let edges = crate::domain::document::export_document_program(&project.document)
        .map(|program| crate::domain::document::connection_policy::endpoint_connections(&program))
        .unwrap_or_default();
    let mut pending = vec![(
        output.clone(),
        std::collections::BTreeMap::<NodeId, NodeId>::new(),
    )];
    let mut visited = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    while let Some((node, scope)) = pending.pop() {
        if !visited.insert((node.clone(), scope.clone())) {
            continue;
        }
        if let Some(argument) = scope.get(&node).filter(|argument| **argument != node) {
            pending.push((argument.clone(), scope.clone()));
            continue;
        }
        if is_sound(&project.document, &node) {
            let instrument = project
                .document
                .graph
                .sound_definition(&node)
                .expect("sound node should own a definition");
            use crate::domain::instrument::InstrumentSource;
            names.insert(match &instrument.source {
                InstrumentSource::Sample(id) => project
                    .samples
                    .manifest()
                    .samples
                    .get(id)
                    .map(|s| s.source_name.clone())
                    .unwrap_or_else(|| "Sample".into()),
                InstrumentSource::Synth(w) => format!("{w:?}"),
                InstrumentSource::Kit => "Kit".into(),
                InstrumentSource::Drum(d) => match d.as_str() {
                    "bd" => "Kick",
                    "sd" => "Snare",
                    "hh" => "Hi-hat",
                    "oh" => "Open hi-hat",
                    _ => "Drum",
                }
                .into(),
                InstrumentSource::Preset(p) => format!("{p:?}"),
            });
        } else {
            if let Some(DocumentNodeKind::TrickInstance(t)) =
                project.document.graph.node(&node).map(|n| &n.kind)
            {
                if let Some(definition) = project.document.tricks.get(&t.prototype.0) {
                    let mut inner = scope.clone();
                    if let Some(input) = &definition.input {
                        if let Some(edge) = edges.iter().find(|edge| {
                            edge.to == node
                                && edge.input
                                    == tessera::prelude::InputEndpoint::Socket(
                                        tessera::prelude::InputPort::new("main"),
                                    )
                        }) {
                            inner.insert(input.clone(), edge.from.clone());
                        }
                    }
                    pending.push((definition.source.clone(), inner));
                    continue;
                }
            }
            pending.extend(
                edges
                    .iter()
                    .filter(|edge| edge.to == node)
                    .map(|edge| (edge.from.clone(), scope.clone())),
            );
        }
    }
    if names.is_empty() {
        "Output".into()
    } else {
        names.into_iter().collect::<Vec<_>>().join(" + ")
    }
}
