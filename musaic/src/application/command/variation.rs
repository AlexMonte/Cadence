//! Independent authored copies retain internal links while replacing every shared definition.
use super::{Fragment, paste_remapped};
use crate::{
    application::session::MusaicProject,
    domain::{
        board::BoardSlot,
        document::{self, DocumentNodeKind, NodeLocation, PlacementAddress},
    },
};
use std::collections::{BTreeMap, BTreeSet};
use tessera::prelude::NodeId;
struct Scope {
    fragment: Fragment,
    definitions: BTreeSet<u64>,
}
fn root_owner(project: &MusaicProject, node: &NodeId) -> Result<NodeId, String> {
    let mut root = node.clone();
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(root.clone()) {
            return Err("Invalid containing pattern cycle".into());
        }
        let location = project
            .document
            .graph
            .location_of(&root)
            .ok_or("The selected source no longer exists")?;
        if location.surface == project.document.root_surface {
            return Ok(root);
        }
        root = project
            .document
            .graph
            .container_node_for_surface(location.surface)
            .ok_or("Missing containing pattern")?;
    }
}
fn collect(project: &MusaicProject, selected: &[NodeId]) -> Result<Scope, String> {
    if selected.is_empty() {
        return Err("Select a pattern or linked tile first".into());
    }
    let mut incoming: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    let program = document::export_document_program(&project.document)
        .map_err(|error| format!("Cannot export this document: {error:?}"))?;
    for edge in document::connection_policy::endpoint_connections(&program) {
        incoming.entry(edge.to).or_default().push(edge.from);
    }
    let mut pending = selected.to_vec();
    let mut roots = BTreeSet::new();
    let mut definitions = BTreeSet::new();
    while let Some(node) = pending.pop() {
        let root = root_owner(project, &node)?;
        if !roots.insert(root.clone()) {
            continue;
        }
        let patch = document::capture_subtree_patch(&project.document, &[root]);
        for node in patch.nodes.values() {
            if let Some(inputs) = incoming.get(&node.id) {
                pending.extend(inputs.iter().cloned());
            }
            if let DocumentNodeKind::TrickInstance(trick) = &node.kind
                && trick.prototype.0 >= 1000
                && definitions.insert(trick.prototype.0)
            {
                let definition = project
                    .document
                    .tricks
                    .get(&trick.prototype.0)
                    .ok_or("Missing linked definition")?;
                pending.push(definition.source.clone());
                pending.extend(definition.input.iter().cloned());
            }
        }
    }
    let mut roots: Vec<_> = roots.into_iter().collect();
    roots.sort_by_key(|id| {
        project
            .document
            .graph
            .location_of(id)
            .map(|l| super::address_order(l.address))
    });
    Ok(Scope {
        fragment: Fragment::capture(project, roots),
        definitions,
    })
}
fn variation_name(project: &MusaicProject, original: &str) -> String {
    let mut prefix = String::new();
    for c in original.chars() {
        if prefix.len() + c.len_utf8() > 96 {
            break;
        }
        prefix.push(c);
    }
    (1u64..)
        .map(|n| {
            if n == 1 {
                format!("{prefix} variation")
            } else {
                format!("{prefix} variation {n}")
            }
        })
        .find(|name| {
            !project
                .document
                .tricks
                .values()
                .any(|d| d.name.eq_ignore_ascii_case(name))
        })
        .expect("finite document has an unused name")
}
pub(super) fn apply(
    project: &mut MusaicProject,
    selected: &[NodeId],
) -> Result<Vec<NodeId>, String> {
    let scope = collect(project, selected)?;
    let first = scope.fragment.patch.locations[&scope.fragment.roots[0]];
    let PlacementAddress::BoardSlot(first_slot) = first.address else {
        return Err("Missing root board position".into());
    };
    let mut bottom = i32::MIN;
    for (location, node) in project
        .document
        .graph
        .nodes_on_surface(project.document.root_surface)
    {
        let PlacementAddress::BoardSlot(slot) = location.address else {
            continue;
        };
        let height = document::root_board_tile_footprint(&node.kind).height as i32;
        bottom = bottom.max(
            slot.y
                .checked_add(height)
                .ok_or("Board position is too large")?,
        );
    }
    let minimum_y = scope
        .fragment
        .roots
        .iter()
        .filter_map(|id| match scope.fragment.patch.locations[id].address {
            PlacementAddress::BoardSlot(slot) => Some(slot.y),
            _ => None,
        })
        .min()
        .ok_or("Missing source position")?;
    let y = i64::from(bottom) + 3 + i64::from(first_slot.y) - i64::from(minimum_y);
    let target = NodeLocation {
        surface: project.document.root_surface,
        address: PlacementAddress::BoardSlot(BoardSlot::new(
            first_slot.x,
            i32::try_from(y).map_err(|_| "No space below the board for a variation")?,
        )),
    };
    let (roots, remap) = paste_remapped(project, &scope.fragment, target)?;
    let mut definitions = BTreeMap::new();
    for old in scope.definitions {
        let original = &scope.fragment.tricks[&old];
        let id = project
            .document
            .tricks
            .keys()
            .next_back()
            .copied()
            .unwrap_or(999)
            .checked_add(1)
            .ok_or("Too many reusable definitions")?;
        let name = variation_name(project, &original.name);
        let source = remap
            .get(&original.source)
            .ok_or("Missing copied source")?
            .clone();
        let input = original
            .input
            .as_ref()
            .map(|id| {
                remap
                    .get(id)
                    .cloned()
                    .ok_or("Missing copied function input")
            })
            .transpose()?;
        project.document.tricks.insert(
            id,
            crate::domain::tricks::TrickDefinition {
                name,
                source,
                input,
            },
        );
        definitions.insert(old, id);
    }
    for (old, new) in &remap {
        if let DocumentNodeKind::TrickInstance(trick) = &scope.fragment.patch.nodes[old].kind
            && let Some(prototype) = definitions.get(&trick.prototype.0)
        {
            project
                .document
                .graph
                .set_trick_prototype(new, document::GraphTilePrototypeId(*prototype));
        }
    }
    Ok(roots)
}
