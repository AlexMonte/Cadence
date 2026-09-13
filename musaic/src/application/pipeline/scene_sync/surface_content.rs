//! Project document nodes into dynamic tile surface labels.

use std::collections::{BTreeMap, BTreeSet};

use tessera::prelude::NodeId;

use super::types::{
    StackDisplayMap, TilePreviewCell, TileSurfaceContent, VisibleAtomCompound, VisibleNodeKind,
};
use crate::{
    application::{
        board_view_settings::AtomDisplayMode,
        editor::{
            Accidental, AtomCompoundSemantic, AtomCompoundView, AtomKind, AtomView, NoteName,
            OperatorKind, compound_atoms,
        },
    },
    domain::{
        board::{BoardSlot, BoardSurfaceId, SurfaceLayoutKind},
        document::{
            AtomValue, DocumentNodeKind, GraphTilePrototypeId, PlacementAddress, StackIndex,
            queries::DocumentQueries,
        },
    },
};

/// One selectable/reorderable authoring group. A modifier and its value are
/// inseparable members; typed octave and accidental tiles remain independent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedTileGroup {
    pub owner: NodeId,
    pub members: Vec<NodeId>,
    pub role: OwnedTileGroupRole,
    pub label: String,
    pub owned_value: Option<OwnedTileValue>,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedTileValue {
    pub node: NodeId,
    pub atom: AtomValue,
    pub display: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedTileGroupRole {
    Pitch,
    Octave,
    Accidental,
    Modifier(crate::domain::document::OperatorValue),
    SoundModifier(tessera::prelude::ParameterKey),
    RhythmModifier,
    Value,
}

/// The local expression surrounding a selected constituent, in authored order.
/// This read model carries stable document IDs, never renderer entity IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedCompoundView {
    pub surface: crate::domain::board::BoardSurfaceId,
    pub groups: Vec<OwnedTileGroup>,
    pub selected_group: usize,
}

/// Looks up an owner or any owned value and returns the same selectable group.
pub fn owned_tile_group_for_node(
    queries: &DocumentQueries<'_>,
    node: &NodeId,
) -> Option<OwnedTileGroup> {
    let view = owned_compound_for_node(queries, node)?;
    view.groups.get(view.selected_group).cloned()
}

/// Adapts the existing visual compound resolver without inventing musical
/// argument binding. Only an adjacent numeric tile already owned by an operator is
/// grouped with it. An octave remains independently movable even when the
/// pitch's face summarizes it as C4.
pub fn owned_compound_for_node(
    queries: &DocumentQueries<'_>,
    node: &NodeId,
) -> Option<OwnedCompoundView> {
    let surface = queries.location_of(node)?.surface;
    let compounds = atom_compounds_for_surface(queries, surface, AtomDisplayMode::CompoundTile);
    let groups = owned_groups_from_compounds(queries, &compounds);
    let selected = groups
        .iter()
        .position(|group| group.members.contains(node))?;
    let (first, end) = expression_bounds(queries, &groups, selected);
    Some(OwnedCompoundView {
        surface,
        groups: groups[first..end].to_vec(),
        selected_group: selected - first,
    })
}

/// Ownership is resolved once from the uncompressed compounds. Rendering may hide
/// pieces, but the inspector keeps exactly these original owner/value groups.
fn owned_groups_from_compounds(
    queries: &DocumentQueries<'_>,
    compounds: &[VisibleAtomCompound],
) -> Vec<OwnedTileGroup> {
    let mut groups = Vec::new();
    for compound in compounds {
        let Some(owner) = compound.compound.members.first() else {
            continue;
        };
        if matches!(queries.node_kind(owner), Some(DocumentNodeKind::Atom(atom))
            if matches!(atom.atom, AtomValue::Operator(_)))
        {
            if let Some(group) = owned_group(queries, compound.compound.members.clone()) {
                groups.push(group);
            }
        } else {
            // Pitch display is a summary, not an ownership relation: explicit
            // octave/accidental groups can move around bound modifier groups.
            groups.extend(
                compound
                    .compound
                    .members
                    .iter()
                    .cloned()
                    .filter_map(|member| owned_group(queries, vec![member])),
            );
        }
    }
    // Complete effects can also lead typed control expressions. Keep adjacent
    // effect values separate when no note/scalar expression owns them.
    let mut has_owner = false;
    for index in 0..groups.len() {
        if index == 0
            || !groups[index - 1]
                .members
                .last()
                .and_then(|id| queries.location_of(id))
                .zip(queries.location_of(&groups[index].owner))
                .is_some_and(|(left, right)| adjacent_addresses(left.address, right.address))
        {
            has_owner = false;
        }
        let effect = matches!(queries.node_kind(&groups[index].owner), Some(DocumentNodeKind::Atom(atom))
            if matches!(&atom.atom, AtomValue::Modifier(modifier) if modifier.effect_value().is_some()));
        if effect && !has_owner {
            groups[index].role = OwnedTileGroupRole::Value;
        } else if matches!(
            groups[index].role,
            OwnedTileGroupRole::Pitch | OwnedTileGroupRole::Value
        ) {
            has_owner = true;
        }
    }
    groups
}

/// Finds visual atom compounds directly from one authored surface. This is a
/// bounded projection step, not a second retained representation of the board.
pub(super) fn atom_compounds_for_surface(
    queries: &DocumentQueries<'_>,
    surface: BoardSurfaceId,
    display_mode: AtomDisplayMode,
) -> Vec<VisibleAtomCompound> {
    if display_mode != AtomDisplayMode::CompoundTile {
        return Vec::new();
    }

    let mut atom_rows: BTreeMap<i32, Vec<(BoardSlot, AtomView)>> = BTreeMap::new();
    for (placement, node) in queries.active_surface_tiles(surface) {
        let DocumentNodeKind::Atom(atom) = &node.kind else {
            continue;
        };
        let slot = match placement.address {
            PlacementAddress::BoardSlot(slot) => slot,
            PlacementAddress::StackIndex(index) => BoardSlot::new(index.0 as i32, 0),
        };
        atom_rows.entry(slot.y).or_default().push((
            slot,
            AtomView {
                node: node.id.clone(),
                kind: atom_value_to_atom_kind(&atom.atom),
            },
        ));
    }

    let mut compounds = Vec::new();
    for (_row, mut row_atoms) in atom_rows {
        row_atoms.sort_by_key(|(slot, _)| slot.x);
        for contiguous in
            row_atoms.chunk_by(|left, right| left.0.x.checked_add(1) == Some(right.0.x))
        {
            let atoms = contiguous
                .iter()
                .map(|(_, atom)| atom.clone())
                .collect::<Vec<_>>();
            let slots = contiguous
                .iter()
                .map(|(slot, atom)| (atom.node.clone(), *slot))
                .collect::<BTreeMap<_, _>>();
            for compound in compound_atoms(&atoms) {
                let Some(anchor) = compound.members.first() else {
                    continue;
                };
                let Some(slot) = slots.get(anchor).copied() else {
                    continue;
                };
                compounds.push(visible_compound(
                    queries,
                    slot,
                    compound.members,
                    compound.display,
                ));
            }
        }
    }
    compounds
}

fn expression_bounds(
    queries: &DocumentQueries<'_>,
    groups: &[OwnedTileGroup],
    selected: usize,
) -> (usize, usize) {
    let adjacent = |left: &OwnedTileGroup, right: &OwnedTileGroup| {
        let Some(left) = left.members.last().and_then(|id| queries.location_of(id)) else {
            return false;
        };
        let Some(right) = queries.location_of(&right.owner) else {
            return false;
        };
        adjacent_addresses(left.address, right.address)
    };
    let is_root = |group: &OwnedTileGroup| {
        matches!(
            group.role,
            OwnedTileGroupRole::Pitch | OwnedTileGroupRole::Value
        )
    };
    let mut first = selected;
    while first > 0 && !is_root(&groups[first]) && adjacent(&groups[first - 1], &groups[first]) {
        first -= 1;
    }
    let mut end = selected + 1;
    while end < groups.len() && !is_root(&groups[end]) && adjacent(&groups[end - 1], &groups[end]) {
        end += 1;
    }
    (first, end)
}

/// One face per complete note expression on a container surface. This remains
/// separate from the low-level compounds used to bind modifier values.
pub(super) fn display_compounds_for_surface(
    queries: &DocumentQueries<'_>,
    layout: SurfaceLayoutKind,
    compounds: &[VisibleAtomCompound],
    wide_containers: impl IntoIterator<Item = StackIndex>,
) -> (Vec<VisibleAtomCompound>, StackDisplayMap) {
    if layout != SurfaceLayoutKind::Stack {
        return (compounds.to_vec(), Default::default());
    }
    let groups = owned_groups_from_compounds(queries, compounds);
    let mut displayed = Vec::new();
    let mut hidden = BTreeMap::new();
    let mut index = 0;
    while index < groups.len() {
        let (_, end) = expression_bounds(queries, &groups, index);
        let expression = &groups[index..end];
        let members: Vec<_> = expression
            .iter()
            .flat_map(|group| group.members.iter().cloned())
            .collect();
        let Some(PlacementAddress::StackIndex(anchor)) = members
            .first()
            .and_then(|node| queries.location_of(node))
            .map(|location| location.address)
        else {
            index = end;
            continue;
        };
        let first = &expression[0];
        let mut display = first.label.clone();
        if first.role == OwnedTileGroupRole::Pitch {
            // Octave and accidental may move around complete modifier groups.
            // Only typed pitch pieces contribute to the face's pitch summary.
            if let Some(accidental) = expression
                .iter()
                .find(|group| group.role == OwnedTileGroupRole::Accidental)
            {
                display.push_str(match accidental.label.as_str() {
                    "#" => "♯",
                    "b" => "♭",
                    other => other,
                });
            }
            if let Some(octave) = expression
                .iter()
                .find(|group| group.role == OwnedTileGroupRole::Octave)
            {
                display.push_str(&octave.label);
            }
        }
        for member in members.iter().skip(1) {
            if let Some(PlacementAddress::StackIndex(address)) =
                queries.location_of(member).map(|location| location.address)
            {
                hidden.insert(address, anchor);
            }
        }
        displayed.push(visible_compound(
            queries,
            BoardSlot::new(anchor.0 as i32, 0),
            members,
            display,
        ));
        index = end;
    }
    let widths = wide_containers
        .into_iter()
        .map(|index| (index, 2))
        .collect();
    let mapping = StackDisplayMap::new(hidden, widths);
    for compound in &mut displayed {
        compound.slot.x = mapping
            .display_index(StackIndex(compound.slot.x as usize))
            .0 as i32;
    }
    (displayed, mapping)
}

fn visible_compound(
    queries: &DocumentQueries<'_>,
    slot: BoardSlot,
    members: Vec<NodeId>,
    display: String,
) -> VisibleAtomCompound {
    let primary_atom = members
        .first()
        .and_then(|member| match queries.node_kind(member) {
            Some(DocumentNodeKind::Atom(atom)) => Some(atom.atom.clone()),
            _ => None,
        });
    VisibleAtomCompound {
        slot,
        compound: AtomCompoundView {
            members,
            display,
            semantic: AtomCompoundSemantic::Single,
        },
        primary_atom,
    }
}

fn atom_value_to_atom_kind(atom: &AtomValue) -> AtomKind {
    match atom {
        AtomValue::NoteName(note) => AtomKind::NoteName(match note {
            crate::domain::document::NoteName::A => NoteName::A,
            crate::domain::document::NoteName::B => NoteName::B,
            crate::domain::document::NoteName::C => NoteName::C,
            crate::domain::document::NoteName::D => NoteName::D,
            crate::domain::document::NoteName::E => NoteName::E,
            crate::domain::document::NoteName::F => NoteName::F,
            crate::domain::document::NoteName::G => NoteName::G,
        }),
        AtomValue::DrumHit(hit) => AtomKind::DrumHit(*hit),
        AtomValue::Octave(value) => AtomKind::Octave(*value),
        AtomValue::Accidental(accidental) => AtomKind::Accidental(match accidental {
            crate::domain::document::Accidental::Sharp => Accidental::Sharp,
            crate::domain::document::Accidental::Flat => Accidental::Flat,
            crate::domain::document::Accidental::Natural => Accidental::Natural,
        }),
        AtomValue::Operator(operator) => AtomKind::Operator(match operator {
            crate::domain::document::OperatorValue::Power => OperatorKind::Power,
            crate::domain::document::OperatorValue::Choice => OperatorKind::Choice,
            crate::domain::document::OperatorValue::Parallel => OperatorKind::Parallel,
            crate::domain::document::OperatorValue::At => OperatorKind::At,
            crate::domain::document::OperatorValue::Multiply => OperatorKind::Multiply,
            crate::domain::document::OperatorValue::Divide => OperatorKind::Divide,
        }),
        AtomValue::Number(value) => AtomKind::Number(*value),
        AtomValue::Ratio(value) => AtomKind::Ratio(*value),
        AtomValue::Modifier(value) => AtomKind::Modifier(value.clone()),
        AtomValue::Rest => AtomKind::Rest,
    }
}

fn adjacent_addresses(
    left: crate::domain::document::PlacementAddress,
    right: crate::domain::document::PlacementAddress,
) -> bool {
    use crate::domain::document::PlacementAddress;
    match (left, right) {
        (PlacementAddress::BoardSlot(a), PlacementAddress::BoardSlot(b)) => {
            a.y == b.y && a.x.checked_add(1) == Some(b.x)
        }
        (PlacementAddress::StackIndex(a), PlacementAddress::StackIndex(b)) => {
            a.0.checked_add(1) == Some(b.0)
        }
        _ => false,
    }
}

fn owned_group(queries: &DocumentQueries<'_>, members: Vec<NodeId>) -> Option<OwnedTileGroup> {
    use crate::domain::document::OperatorValue;
    use tessera::prelude::AtomModifier;
    let owner = members.first()?.clone();
    let DocumentNodeKind::Atom(atom) = queries.node_kind(&owner)? else {
        return None;
    };
    let role = match &atom.atom {
        AtomValue::NoteName(_) => OwnedTileGroupRole::Pitch,
        AtomValue::DrumHit(_) => OwnedTileGroupRole::Value,
        AtomValue::Octave(_) => OwnedTileGroupRole::Octave,
        AtomValue::Accidental(_) => OwnedTileGroupRole::Accidental,
        AtomValue::Operator(
            crate::domain::document::OperatorValue::Choice
            | crate::domain::document::OperatorValue::Parallel,
        ) => OwnedTileGroupRole::Value,
        AtomValue::Operator(operator) => OwnedTileGroupRole::Modifier(*operator),
        AtomValue::Modifier(AtomModifier::Elongate(_)) => {
            OwnedTileGroupRole::Modifier(OperatorValue::At)
        }
        AtomValue::Modifier(AtomModifier::Replicate(_)) => {
            OwnedTileGroupRole::Modifier(OperatorValue::Power)
        }
        AtomValue::Modifier(AtomModifier::Fast(_)) => {
            OwnedTileGroupRole::Modifier(OperatorValue::Multiply)
        }
        AtomValue::Modifier(AtomModifier::Slow(_)) => {
            OwnedTileGroupRole::Modifier(OperatorValue::Divide)
        }
        AtomValue::Modifier(modifier) => modifier
            .parameter_key()
            .map(OwnedTileGroupRole::SoundModifier)
            .unwrap_or(OwnedTileGroupRole::RhythmModifier),
        _ => OwnedTileGroupRole::Value,
    };
    let numeric_rhythm = match &atom.atom {
        AtomValue::Modifier(AtomModifier::Elongate(value)) => Some(*value),
        AtomValue::Modifier(AtomModifier::Replicate(value)) => {
            Some(tessera::prelude::Rational::from_integer(i64::from(*value)))
        }
        _ => None,
    };
    let owned_value = if let Some(value) = numeric_rhythm
        .map(tessera::prelude::FieldValue::rational)
        .or_else(|| atom.atom.owned_parameter_value())
    {
        Some(OwnedTileValue {
            node: owner.clone(),
            atom: atom.atom.clone(),
            display: crate::application::editor::atoms::parameter_display(&value),
        })
    } else if let AtomValue::Modifier(_) = &atom.atom {
        Some(OwnedTileValue {
            node: owner.clone(),
            atom: atom.atom.clone(),
            display: format_atom_display(&atom.atom),
        })
    } else {
        members.get(1).and_then(|id| {
            let DocumentNodeKind::Atom(value) = queries.node_kind(id)? else {
                return None;
            };
            Some(OwnedTileValue {
                node: id.clone(),
                atom: value.atom.clone(),
                display: format_atom_display(&value.atom),
            })
        })
    };
    let complete = !matches!(
        role,
        OwnedTileGroupRole::Modifier(_)
            | OwnedTileGroupRole::SoundModifier(_)
            | OwnedTileGroupRole::RhythmModifier
    ) || owned_value.is_some();
    let mut label = format_atom_display(&atom.atom);
    // A typed modifier's label already includes its value. Only a separate
    // operator tile needs the adjacent operand appended to its board label.
    if matches!(atom.atom, AtomValue::Operator(_)) {
        if let Some(value) = &owned_value {
            label.push_str(&value.display);
        }
    }
    Some(OwnedTileGroup {
        owner,
        members,
        role,
        label,
        owned_value,
        complete,
    })
}

pub fn surface_content_for_node(
    queries: &DocumentQueries<'_>,
    node_id: &NodeId,
    compound_anchor: &std::collections::BTreeMap<NodeId, (String, Vec<NodeId>)>,
    compound_members: &std::collections::BTreeSet<NodeId>,
) -> TileSurfaceContent {
    if compound_members.contains(node_id) && !compound_anchor.contains_key(node_id) {
        return TileSurfaceContent::Empty;
    }
    if let Some((display, members)) = compound_anchor.get(node_id) {
        return TileSurfaceContent::Compound {
            display: display.clone(),
            layers: members.len(),
            parts: members
                .iter()
                .filter_map(|id| match queries.node_kind(id) {
                    Some(DocumentNodeKind::Atom(atom)) => Some(format_atom_display(&atom.atom)),
                    _ => None,
                })
                .collect(),
        };
    }

    match queries.node_kind(node_id) {
        Some(DocumentNodeKind::Atom(atom)) => TileSurfaceContent::Scalar {
            display: format_atom_display(&atom.atom),
        },
        Some(DocumentNodeKind::TrickInstance(trick)) if trick.prototype.0 == 32 => {
            let ports = queries
                .document
                .connections
                .bindings
                .get(node_id)
                .map(crate::domain::flow::port_config)
                .unwrap_or_else(|| {
                    crate::domain::flow::port_config(&tessera::prelude::default_spatial_bindings(
                        &tessera::prelude::RootSurfaceNodeKind::Transform(
                            tessera::prelude::TransformNode::new(
                                tessera::prelude::TransformKind::Wire,
                            ),
                        ),
                    ))
                });
            TileSurfaceContent::Wire { ports }
        }
        Some(DocumentNodeKind::TrickInstance(trick)) => {
            let label = queries
                .document
                .tricks
                .get(&trick.prototype.0)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| trick_label(trick.prototype));
            let aux = aux_scalar_for_transform(queries, node_id);
            TileSurfaceContent::Transform { label, aux }
        }
        Some(DocumentNodeKind::Sound(_)) => TileSurfaceContent::Transform {
            label: "Sound".into(),
            aux: None,
        },
        Some(DocumentNodeKind::FlowControl(control)) => TileSurfaceContent::Transform {
            label: crate::domain::flow::label(control.kind).into(),
            aux: None,
        },
        Some(DocumentNodeKind::Container(_)) => container_preview(queries, node_id, 0),
        _ => TileSurfaceContent::Empty,
    }
}

pub(crate) fn format_atom_display(atom: &AtomValue) -> String {
    match atom {
        AtomValue::NoteName(note) => format!("{note:?}"),
        AtomValue::DrumHit(hit) => hit.code().into(),
        AtomValue::Octave(octave) => octave.to_string(),
        AtomValue::Accidental(acc) => match acc {
            crate::domain::document::Accidental::Sharp => "#",
            crate::domain::document::Accidental::Flat => "b",
            crate::domain::document::Accidental::Natural => "♮",
        }
        .into(),
        AtomValue::Operator(op) => match op {
            crate::domain::document::OperatorValue::Power => "^",
            crate::domain::document::OperatorValue::Choice => "|",
            crate::domain::document::OperatorValue::Parallel => ",",
            crate::domain::document::OperatorValue::At => "@",
            crate::domain::document::OperatorValue::Multiply => "×",
            crate::domain::document::OperatorValue::Divide => "/",
        }
        .into(),
        AtomValue::Number(n) => format_scalar_number(*n),
        AtomValue::Ratio(value) => format!("{}/{}", value.numerator, value.denominator),
        AtomValue::Modifier(value) => crate::application::editor::atoms::modifier_display(value),
        AtomValue::Rest => "~".into(),
    }
}

fn format_scalar_number(value: i32) -> String {
    value.to_string()
}

fn trick_label(prototype: GraphTilePrototypeId) -> String {
    crate::domain::transform::transform_kind_from_prototype(prototype)
        .map(|kind| format!("{kind:?}"))
        .unwrap_or_else(|| format!("Trick {}", prototype.0))
}

/// Reuse the opened board's compound resolver; thumbnails never parse their
/// own musical language. Depth and breadth are bounded for malformed/large
/// documents, while child_count still reports the complete authored surface.
fn container_preview(
    queries: &DocumentQueries<'_>,
    node_id: &NodeId,
    depth: usize,
) -> TileSurfaceContent {
    let Some(DocumentNodeKind::Container(container)) = queries.node_kind(node_id) else {
        return TileSurfaceContent::Empty;
    };
    let surface = container.local_surface;
    let layout = queries
        .surface_layout(surface)
        .unwrap_or(SurfaceLayoutKind::Board);
    let compounds = atom_compounds_for_surface(queries, surface, AtomDisplayMode::CompoundTile);
    let mut tiles = queries.active_surface_tiles(surface).collect::<Vec<_>>();
    let child_count = tiles.len();
    let wide_containers = tiles.iter().filter_map(|(location, node)| {
        matches!(node.kind, DocumentNodeKind::Container(_))
            .then_some(location.address)
            .and_then(|address| match address {
                PlacementAddress::StackIndex(index) => Some(index),
                PlacementAddress::BoardSlot(_) => None,
            })
    });
    let (compounds, mapping) =
        display_compounds_for_surface(queries, layout, &compounds, wide_containers);
    let mut anchors = BTreeMap::new();
    let mut members = BTreeSet::new();
    for compound in compounds {
        if let Some(anchor) = compound.compound.members.first() {
            anchors.insert(
                anchor.clone(),
                (compound.compound.display, compound.compound.members.clone()),
            );
        }
        members.extend(compound.compound.members);
    }
    tiles.sort_by_key(|(location, _)| match location.address {
        PlacementAddress::BoardSlot(slot) => (slot.y, slot.x),
        PlacementAddress::StackIndex(index) => (0, index.0 as i32),
    });
    let children = if depth >= 2 {
        Vec::new()
    } else {
        tiles
            .into_iter()
            .filter(|(_, node)| !members.contains(&node.id) || anchors.contains_key(&node.id))
            .take(24)
            .map(|(location, node)| {
                let kind = visible_node_kind(&node.kind);
                let content = if kind == VisibleNodeKind::Container {
                    container_preview(queries, &node.id, depth + 1)
                } else {
                    surface_content_for_node(queries, &node.id, &anchors, &members)
                };
                TilePreviewCell {
                    slot: match mapping.display_address(location.address) {
                        PlacementAddress::BoardSlot(slot) => slot,
                        PlacementAddress::StackIndex(index) => BoardSlot::new(index.0 as i32, 0),
                    },
                    kind,
                    content,
                }
            })
            .collect()
    };
    TileSurfaceContent::Container {
        kind: container.kind,
        children,
        child_count,
    }
}

pub(super) fn visible_node_kind(kind: &DocumentNodeKind) -> VisibleNodeKind {
    match kind {
        DocumentNodeKind::Atom(_) => VisibleNodeKind::Atom,
        DocumentNodeKind::Container(_) => VisibleNodeKind::Container,
        DocumentNodeKind::Output(_) => VisibleNodeKind::Output,
        DocumentNodeKind::Sound(_)
        | DocumentNodeKind::TrickInstance(_)
        | DocumentNodeKind::FlowControl(_) => VisibleNodeKind::TrickInstance,
        DocumentNodeKind::Tile(_) | DocumentNodeKind::Arrangement(_) => VisibleNodeKind::Tile,
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
            if matches!(
                atom.atom,
                AtomValue::Number(_) | AtomValue::Ratio(_) | AtomValue::Octave(_)
            ) {
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

    fn grouped_document(order: &[(&str, AtomValue, usize)]) -> MusaicDocument {
        use crate::domain::{
            board::BoardSlot,
            document::{PlacementAddress, TileSpawnKind},
        };
        let mut document = MusaicDocument::new_empty();
        let root = document.root_surface;
        for (name, atom, position) in order {
            document
                .graph
                .insert_tile_at_id(
                    &mut document.surfaces,
                    root,
                    PlacementAddress::BoardSlot(BoardSlot::new(*position as i32, 0)),
                    NodeId::new(*name),
                    TileSpawnKind::Atom { atom: atom.clone() },
                )
                .unwrap();
        }
        document
    }

    #[test]
    fn owned_modifier_values_survive_group_reordering_and_octave_is_independent() {
        use crate::domain::document::OperatorValue;
        let pitch = ("pitch", AtomValue::NoteName(NoteName::C));
        let octave = ("octave", AtomValue::Octave(4));
        let elongate = ("elongate", AtomValue::Operator(OperatorValue::At));
        let two = ("two", AtomValue::Number(2));
        let fast = ("fast", AtomValue::Operator(OperatorValue::Multiply));
        let three = ("three", AtomValue::Number(3));
        for order in [
            vec![
                pitch.clone(),
                octave.clone(),
                elongate.clone(),
                two.clone(),
                fast.clone(),
                three.clone(),
            ],
            vec![
                pitch.clone(),
                fast.clone(),
                three.clone(),
                octave.clone(),
                elongate.clone(),
                two.clone(),
            ],
        ] {
            let positioned = order
                .into_iter()
                .enumerate()
                .map(|(i, (id, atom))| (id, atom, i))
                .collect::<Vec<_>>();
            let document = grouped_document(&positioned);
            let queries = DocumentQueries::new(&document);
            let view = owned_compound_for_node(&queries, &NodeId::new("two")).unwrap();
            assert_eq!(view.groups.len(), 4);
            let modifier = &view.groups[view.selected_group];
            assert_eq!(modifier.owner, NodeId::new("elongate"));
            assert_eq!(
                modifier.members,
                vec![NodeId::new("elongate"), NodeId::new("two")]
            );
            assert_eq!(
                modifier.owned_value.as_ref().unwrap().atom,
                AtomValue::Number(2)
            );
            let octave = owned_tile_group_for_node(&queries, &NodeId::new("octave")).unwrap();
            assert_eq!(octave.members, vec![NodeId::new("octave")]);
            assert_eq!(octave.role, OwnedTileGroupRole::Octave);
            let fast = owned_tile_group_for_node(&queries, &NodeId::new("three")).unwrap();
            assert_eq!(fast.owner, NodeId::new("fast"));
            assert_eq!(fast.owned_value.unwrap().atom, AtomValue::Number(3));
        }
    }

    #[test]
    fn standalone_effect_values_keep_independent_time_slots_and_note_ownership() {
        use tessera::prelude::{AtomModifier, DelayParameters};
        let effect = AtomValue::Modifier(AtomModifier::Delay(DelayParameters::default()));
        let values =
            grouped_document(&[("first", effect.clone(), 0), ("second", effect.clone(), 1)]);
        for name in ["first", "second"] {
            let view = owned_compound_for_node(&DocumentQueries::new(&values), &NodeId::new(name))
                .unwrap();
            assert_eq!(view.groups.len(), 1);
            assert_eq!(view.groups[0].role, OwnedTileGroupRole::Value);
        }
        let note = grouped_document(&[
            ("note", AtomValue::NoteName(NoteName::C), 0),
            ("effect", effect, 1),
        ]);
        let view =
            owned_compound_for_node(&DocumentQueries::new(&note), &NodeId::new("effect")).unwrap();
        assert_eq!(view.groups.len(), 2);
        assert!(matches!(
            view.groups[1].role,
            OwnedTileGroupRole::SoundModifier(tessera::prelude::ParameterKey::Delay)
        ));
    }

    #[test]
    fn sound_modifier_is_its_own_owned_group() {
        use tessera::prelude::{AtomModifier as M, ParameterKey, Rational};
        for (legato_position, gate_position, octave_position) in [(1, 2, 3), (3, 1, 2)] {
            let document = grouped_document(&[
                ("pitch", AtomValue::NoteName(NoteName::C), 0),
                (
                    "length",
                    AtomValue::Modifier(M::Legato(Rational::new(3, 2))),
                    legato_position,
                ),
                ("gate", AtomValue::Modifier(M::Gate(false)), gate_position),
                ("octave", AtomValue::Octave(4), octave_position),
            ]);
            let queries = DocumentQueries::new(&document);
            let view = owned_compound_for_node(&queries, &NodeId::new("length")).unwrap();
            assert_eq!(view.groups.len(), 4);
            let group = &view.groups[view.selected_group];
            assert_eq!(
                group.role,
                OwnedTileGroupRole::SoundModifier(ParameterKey::Legato)
            );
            assert_eq!(group.label, "L3/2");
            assert_eq!(group.members, [NodeId::new("length")]);
            assert!(group.complete);
            let value = group.owned_value.as_ref().unwrap();
            assert_eq!(value.node, group.owner);
            assert_eq!(
                value.atom,
                AtomValue::Modifier(M::Legato(Rational::new(3, 2)))
            );
            assert_eq!(value.display, "3/2");
            let gate = owned_tile_group_for_node(&queries, &NodeId::new("gate")).unwrap();
            assert_eq!(gate.label, "Gate0");
            assert_eq!(
                gate.owned_value.unwrap().atom,
                AtomValue::Modifier(M::Gate(false))
            );
        }
    }

    #[test]
    fn rhythm_groups_stay_with_notes_and_binary_tokens_remain_separate() {
        use crate::domain::document::OperatorValue;
        use tessera::prelude::AtomModifier;
        for operator in [OperatorValue::Choice, OperatorValue::Parallel] {
            let rhythm = AtomValue::Modifier(AtomModifier::Euclid {
                pulses: 3,
                steps: 8,
            });
            let document = grouped_document(&[
                ("pitch", AtomValue::NoteName(NoteName::C), 0),
                ("rhythm", rhythm.clone(), 1),
                ("separator", AtomValue::Operator(operator), 2),
                ("next", AtomValue::NoteName(NoteName::E), 3),
            ]);
            let queries = DocumentQueries::new(&document);
            let view = owned_compound_for_node(&queries, &NodeId::new("pitch")).unwrap();
            assert_eq!(view.groups.len(), 2);
            assert_eq!(view.groups[1].role, OwnedTileGroupRole::RhythmModifier);
            assert_eq!(view.groups[1].owned_value.as_ref().unwrap().atom, rhythm);
            assert!(view.groups[1].complete);
            let separator = owned_compound_for_node(&queries, &NodeId::new("separator")).unwrap();
            assert_eq!(
                separator.groups.len(),
                1,
                "a separator must be a visible face, not hidden inside a note stack"
            );
            assert_eq!(separator.groups[0].role, OwnedTileGroupRole::Value);
        }
    }

    #[test]
    fn owned_ratio_remains_exact_and_selectable_through_its_modifier() {
        use crate::domain::document::OperatorValue;
        let value = tessera::prelude::Rational::new(3, 2);
        let document = grouped_document(&[
            ("pitch", AtomValue::NoteName(NoteName::C), 0),
            ("operator", AtomValue::Operator(OperatorValue::Multiply), 1),
            ("ratio", AtomValue::Ratio(value), 2),
        ]);
        let group =
            owned_tile_group_for_node(&DocumentQueries::new(&document), &NodeId::new("ratio"))
                .unwrap();
        assert_eq!(
            group.members,
            vec![NodeId::new("operator"), NodeId::new("ratio")]
        );
        assert_eq!(
            group.owned_value.as_ref().unwrap().atom,
            AtomValue::Ratio(value)
        );
        assert_eq!(group.owned_value.as_ref().unwrap().display, "3/2");
        assert_eq!(group.label, "×3/2");
    }

    #[test]
    fn group_ownership_stops_at_empty_slots_and_missing_values() {
        use crate::domain::document::OperatorValue;
        let document = grouped_document(&[
            ("pitch", AtomValue::NoteName(NoteName::C), 0),
            ("operator", AtomValue::Operator(OperatorValue::At), 1),
            ("number", AtomValue::Number(2), 3),
        ]);
        let queries = DocumentQueries::new(&document);
        let group = owned_tile_group_for_node(&queries, &NodeId::new("operator")).unwrap();
        assert_eq!(group.members, vec![NodeId::new("operator")]);
        assert_eq!(group.owned_value, None);
        assert!(!group.complete);
        let separate = owned_compound_for_node(&queries, &NodeId::new("number")).unwrap();
        assert_eq!(separate.groups.len(), 1);
    }

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

    #[test]
    fn container_thumbnail_retains_nested_children_and_actual_layer_counts() {
        use crate::domain::{
            board::BoardSlot,
            document::{ContainerKind, PlacementAddress, StackIndex, TileSpawnKind},
        };
        let mut document = MusaicDocument::new_empty();
        let root = document.root_surface;
        let outer = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            )
            .unwrap();
        let surface = document.graph.container_surface(&outer).unwrap();
        let nested = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(0)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Alternating,
                },
            )
            .unwrap();
        let inner_surface = document.graph.container_surface(&nested).unwrap();
        for (index, atom) in [AtomValue::NoteName(NoteName::C), AtomValue::Octave(4)]
            .into_iter()
            .enumerate()
        {
            document
                .graph
                .insert_tile(
                    &mut document.surfaces,
                    inner_surface,
                    PlacementAddress::StackIndex(StackIndex(index)),
                    TileSpawnKind::Atom { atom },
                )
                .unwrap();
        }
        let preview = container_preview(&DocumentQueries::new(&document), &outer, 0);
        let TileSurfaceContent::Container {
            children,
            child_count,
            ..
        } = preview
        else {
            panic!("outer preview")
        };
        assert_eq!(child_count, 1);
        let TileSurfaceContent::Container {
            children,
            child_count,
            kind,
        } = &children[0].content
        else {
            panic!("nested preview")
        };
        assert_eq!(*kind, ContainerKind::Alternating);
        assert_eq!(*child_count, 2);
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0].content,
            TileSurfaceContent::Compound {
                display: "C4".into(),
                layers: 2,
                parts: vec!["C".into(), "4".into()],
            }
        );
    }
}
