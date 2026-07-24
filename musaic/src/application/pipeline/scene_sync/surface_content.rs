//! Project document nodes into dynamic tile surface labels.

use tessera::prelude::NodeId;

use super::types::TileSurfaceContent;
use crate::domain::document::{
    AtomValue, DocumentNodeKind, GraphTilePrototypeId, queries::DocumentQueries,
};

pub fn surface_content_for_node(
    queries: &DocumentQueries<'_>,
    node_id: &NodeId,
    compound_anchor: &std::collections::BTreeMap<NodeId, String>,
    compound_members: &std::collections::BTreeSet<NodeId>,
) -> TileSurfaceContent {
    if compound_members.contains(node_id) && !compound_anchor.contains_key(node_id) {
        return TileSurfaceContent::Empty;
    }
    if let Some(display) = compound_anchor.get(node_id) {
        return TileSurfaceContent::Compound {
            display: display.clone(),
        };
    }

    match queries.node_kind(node_id) {
        Some(DocumentNodeKind::Atom(atom)) => TileSurfaceContent::Scalar {
            display: format_atom_display(&atom.atom),
        },
        Some(DocumentNodeKind::TrickInstance(trick)) => {
            let label = trick_label(trick.prototype);
            let aux = aux_scalar_for_transform(queries, node_id);
            TileSurfaceContent::Transform { label, aux }
        }
        _ => TileSurfaceContent::Empty,
    }
}

fn format_atom_display(atom: &AtomValue) -> String {
    match atom {
        AtomValue::NoteName(note) => format!("{note:?}"),
        AtomValue::Octave(octave) => octave.to_string(),
        AtomValue::Accidental(acc) => format!("{acc:?}"),
        AtomValue::Operator(op) => format!("{op:?}"),
        AtomValue::Number(n) => format_scalar_number(*n),
        AtomValue::Rest => "~".into(),
    }
}

fn format_scalar_number(value: i32) -> String {
    value.to_string()
}

fn trick_label(prototype: GraphTilePrototypeId) -> String {
    match prototype.0 {
        0 => "Fast".into(),
        1 => "Slow".into(),
        2 => "Legato".into(),
        3 => "Gain".into(),
        id => format!("Trick {id}"),
    }
}

fn aux_scalar_for_transform(queries: &DocumentQueries<'_>, transform: &NodeId) -> Option<String> {
    let surface = queries
        .location_of(transform)
        .map(|location| location.surface)?;
    for connection in queries.connections_on_surface(surface) {
        if connection.to != *transform {
            continue;
        }
        if let Some(DocumentNodeKind::Atom(atom)) = queries.node_kind(&connection.from) {
            if matches!(atom.atom, AtomValue::Number(_) | AtomValue::Octave(_)) {
                return Some(format_atom_display(&atom.atom));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{MusaicDocument, NoteName};

    #[test]
    fn scalar_atom_formats_number() {
        let document = MusaicDocument::new_empty();
        let queries = DocumentQueries::new(&document);
        let content = format_atom_display(&AtomValue::Number(25));
        assert_eq!(content, "25");
        let _ = queries;
    }

    #[test]
    fn note_atom_formats_name() {
        let content = format_atom_display(&AtomValue::NoteName(NoteName::C));
        assert!(content.contains('C'));
    }
}
