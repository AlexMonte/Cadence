use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tessera::prelude::{Arrangement, ContainerId, NodeId};

use tessera::prelude::TileFootprint;

use crate::domain::board::{
    BoardSlot, BoardSurface, BoardSurfaceId, BoardSurfaceKind, BoardSurfaces,
};

use super::footprint::root_board_tile_footprint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TilePrototypeId(pub u64);

/// Authored document structure for the board language.
///
/// Important domain split:
/// - `placements` store each tile's anchor slot; root-board containers/outputs also
///   occupy a [`root_board_tile_footprint`] rectangle that must stay free.
/// - Empty board slots are not document nodes. They are editor/render affordances.
/// - Container nodes own child board surfaces through `container_surfaces`.
///
/// In other words, a board surface is an authoring space, a slot is an address,
/// and a document node placed at that address is the AST/program structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentGraph {
    nodes: BTreeMap<NodeId, DocumentNode>,
    #[serde(with = "placement_entries")]
    board_placements: BTreeMap<(BoardSurfaceId, BoardSlot), NodeId>,
    #[serde(with = "placement_entries")]
    stack_placements: BTreeMap<(BoardSurfaceId, StackIndex), NodeId>,
    node_locations: BTreeMap<NodeId, NodeLocation>,
    container_surfaces: BTreeMap<NodeId, BoardSurfaceId>,
    next_node_id: u64,
    next_surface_id: u64,
}

/// JSON object keys cannot hold typed (surface, position) pairs, so placements
/// use explicit key/value entries.
mod placement_entries {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S, K, V>(map: &BTreeMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        K: Serialize,
        V: Serialize,
    {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
    {
        let entries = Vec::<(K, V)>::deserialize(deserializer)?;
        let mut map = BTreeMap::new();
        for (key, value) in entries {
            if map.insert(key, value).is_some() {
                return Err(serde::de::Error::custom("duplicate authored placement"));
            }
        }
        Ok(map)
    }
}

impl Default for DocumentGraph {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            board_placements: BTreeMap::new(),
            stack_placements: BTreeMap::new(),
            node_locations: BTreeMap::new(),
            container_surfaces: BTreeMap::new(),
            next_node_id: 1,
            next_surface_id: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentNode {
    pub id: NodeId,
    pub kind: DocumentNodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentNodeKind {
    Tile(TileNode),
    Atom(AtomNode),
    Container(ContainerNode),
    Output(OutputNode),
    Sound(Box<SoundNode>),
    TrickInstance(TrickInstanceNode),
    FlowControl(tessera::prelude::FlowControlNode),
    Arrangement(ArrangementNode),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrangementNode {
    pub arrangement: Arrangement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileNode {
    pub prototype: TilePrototypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtomNode {
    pub atom: AtomValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerNode {
    pub kind: ContainerKind,
    pub local_surface: BoardSurfaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputNode {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundNode {
    pub definition: crate::domain::instrument::InstrumentDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrickInstanceNode {
    pub prototype: TilePrototypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerKind {
    Sequence,
    Arrangement,
    Subdivision,
    Alternating,
    Parallel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomValue {
    NoteName(NoteName),
    DrumHit(DrumHit),
    Octave(i8),
    Accidental(Accidental),
    Operator(OperatorValue),
    Number(i32),
    Ratio(tessera::prelude::Rational),
    /// One modifier tile with a typed, role-owned operand.
    Modifier(tessera::prelude::AtomModifier),
    Rest,
}

/// Stable names provided by Musaic's built-in drum kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DrumHit {
    Bd,
    Sd,
    Hh,
    Oh,
}

impl DrumHit {
    pub const ALL: [Self; 4] = [Self::Bd, Self::Sd, Self::Hh, Self::Oh];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Bd => "bd",
            Self::Sd => "sd",
            Self::Hh => "hh",
            Self::Oh => "oh",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Bd => "Kick",
            Self::Sd => "Snare",
            Self::Hh => "Closed hat",
            Self::Oh => "Open hat",
        }
    }
}

impl AtomValue {
    /// A free scalar source; owned modifier operands are deliberately excluded.
    pub fn numeric_rational(&self) -> Option<tessera::prelude::Rational> {
        match self {
            Self::Number(value) => {
                Some(tessera::prelude::Rational::from_integer(i64::from(*value)))
            }
            Self::Ratio(value) => Some(*value),
            _ => None,
        }
    }

    pub fn owned_parameter_value(&self) -> Option<tessera::prelude::FieldValue> {
        match self {
            Self::Modifier(modifier) => modifier.parameter_value(),
            _ => None,
        }
    }

    pub fn owned_numeric_rational(&self) -> Option<tessera::prelude::Rational> {
        match self.owned_parameter_value()? {
            tessera::prelude::FieldValue::Rational { value } => Some(value),
            _ => None,
        }
    }

    pub fn with_owned_parameter_value(
        &self,
        value: tessera::prelude::FieldValue,
    ) -> Result<Self, &'static str> {
        match self {
            Self::Modifier(modifier) => modifier.with_parameter_value(value).map(Self::Modifier),
            _ => Err("This tile does not own a modifier value."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteName {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accidental {
    Sharp,
    Flat,
    Natural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorValue {
    Power,
    At,
    Multiply,
    Divide,
    Choice,
    Parallel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StackIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementAddress {
    BoardSlot(BoardSlot),
    StackIndex(StackIndex),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeLocation {
    pub surface: BoardSurfaceId,
    pub address: PlacementAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileSpawnKind {
    Tile {
        prototype: TilePrototypeId,
    },
    Atom {
        atom: AtomValue,
    },
    Container {
        kind: ContainerKind,
    },
    Output {
        name: String,
    },
    Sound {
        definition: Box<crate::domain::instrument::InstrumentDefinition>,
    },
    TrickInstance {
        prototype: TilePrototypeId,
    },
    FlowControl {
        control: tessera::prelude::FlowControlNode,
    },
}

impl TileSpawnKind {
    pub fn sound(definition: crate::domain::instrument::InstrumentDefinition) -> Self {
        Self::Sound {
            definition: Box::new(definition),
        }
    }
}

impl DocumentGraph {
    pub fn validate(
        &self,
        surfaces: &BoardSurfaces,
        root_surface: BoardSurfaceId,
    ) -> Result<(), String> {
        if surfaces.root() != Some(root_surface)
            || !matches!(
                surfaces.kind(root_surface),
                Some(BoardSurfaceKind::RootBoard)
            )
        {
            return Err("The document must have exactly one matching root surface".into());
        }

        for (surface_id, surface) in surfaces.iter() {
            if surface.id != *surface_id {
                return Err("A surface is stored under the wrong identifier".into());
            }
        }

        if self.nodes.len() != self.node_locations.len() {
            return Err("Every tile must have exactly one placement".into());
        }
        for (node_id, node) in &self.nodes {
            if &node.id != node_id {
                return Err("A tile is stored under the wrong identifier".into());
            }
            let Some(location) = self.node_locations.get(node_id) else {
                return Err(format!("Tile {} has no placement", node_id.0));
            };
            let Some(surface) = surfaces.get(location.surface) else {
                return Err(format!("Tile {} uses a missing surface", node_id.0));
            };
            let placed = match (surface.kind.layout_kind(), location.address) {
                (
                    crate::domain::board::SurfaceLayoutKind::Board,
                    PlacementAddress::BoardSlot(slot),
                ) => self.board_placements.get(&(location.surface, slot)),
                (
                    crate::domain::board::SurfaceLayoutKind::Stack,
                    PlacementAddress::StackIndex(index),
                ) => self.stack_placements.get(&(location.surface, index)),
                _ => return Err(format!("Tile {} uses the wrong placement kind", node_id.0)),
            };
            if placed != Some(node_id) {
                return Err(format!("Tile {} placement indexes disagree", node_id.0));
            }

            match &node.kind {
                DocumentNodeKind::Sound(sound) => sound
                    .definition
                    .validate()
                    .map_err(|error| format!("Sound {} is invalid: {error}", node_id.0))?,
                DocumentNodeKind::Container(container) => {
                    if self.container_surfaces.get(node_id) != Some(&container.local_surface) {
                        return Err(format!(
                            "Container {} does not own its local surface",
                            node_id.0
                        ));
                    }
                    match surfaces.kind(container.local_surface) {
                        Some(BoardSurfaceKind::ContainerStack { container: owner })
                            if owner == ContainerId::new(node_id.0.clone()) => {}
                        _ => {
                            return Err(format!(
                                "Container {} has an invalid local surface",
                                node_id.0
                            ));
                        }
                    }
                }
                _ if self.container_surfaces.contains_key(node_id) => {
                    return Err(format!("Non-container tile {} owns a surface", node_id.0));
                }
                _ => {}
            }
        }

        for ((surface, slot), node) in &self.board_placements {
            if !matches!(surfaces.kind(*surface), Some(BoardSurfaceKind::RootBoard))
                || self.node_locations.get(node)
                    != Some(&NodeLocation {
                        surface: *surface,
                        address: PlacementAddress::BoardSlot(*slot),
                    })
            {
                return Err("A board placement is invalid".into());
            }
        }
        for ((surface, index), node) in &self.stack_placements {
            if !matches!(
                surfaces.kind(*surface),
                Some(BoardSurfaceKind::ContainerStack { .. })
            ) || self.node_locations.get(node)
                != Some(&NodeLocation {
                    surface: *surface,
                    address: PlacementAddress::StackIndex(*index),
                })
            {
                return Err("A container placement is invalid".into());
            }
        }

        let mut occupied = BTreeSet::new();
        for ((surface, anchor), node_id) in &self.board_placements {
            let node = self
                .nodes
                .get(node_id)
                .ok_or("A placement refers to a missing tile")?;
            for cell in root_board_tile_footprint(&node.kind).occupied_cells(*anchor) {
                if !occupied.insert((*surface, cell)) {
                    return Err("Root-board tile footprints overlap".into());
                }
            }
        }

        let mut owned_surfaces = BTreeSet::new();
        for (owner, surface) in &self.container_surfaces {
            if !owned_surfaces.insert(*surface) {
                return Err("A container surface has more than one owner".into());
            }
            if !self.nodes.contains_key(owner) {
                return Err("A missing container owns a surface".into());
            }
        }
        for (surface_id, surface) in surfaces.iter() {
            if matches!(surface.kind, BoardSurfaceKind::ContainerStack { .. })
                && !owned_surfaces.contains(surface_id)
            {
                return Err("A container surface has no owner".into());
            }
        }

        let max_surface = surfaces.iter().map(|(id, _)| id.0).max().unwrap_or(0);
        if self.next_surface_id <= max_surface {
            return Err("The next surface identifier is not available".into());
        }
        let max_generated_node = self
            .nodes
            .keys()
            .filter_map(|id| id.0.strip_prefix("doc_")?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        if self.next_node_id <= max_generated_node {
            return Err("The next tile identifier is not available".into());
        }
        Ok(())
    }

    pub fn nodes(&self) -> impl Iterator<Item = &DocumentNode> {
        self.nodes.values()
    }

    /// Returns authored nodes placed on a surface.
    ///
    /// This intentionally does not synthesize empty slots. Empty slots belong to
    /// the editor/render layer as possible placement targets, not to the document
    /// graph as program nodes.
    pub fn nodes_on_surface(&self, surface: BoardSurfaceId) -> Vec<(NodeLocation, &DocumentNode)> {
        self.node_locations
            .iter()
            .filter_map(|(node_id, location)| {
                if location.surface == surface {
                    self.nodes.get(node_id).map(|node| (*location, node))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn contains_node(&self, node: &NodeId) -> bool {
        self.nodes.contains_key(node)
    }

    pub fn set_trick_prototype(&mut self, node: &NodeId, prototype: TilePrototypeId) {
        if let Some(DocumentNode {
            kind: DocumentNodeKind::TrickInstance(t),
            ..
        }) = self.nodes.get_mut(node)
        {
            t.prototype = prototype;
        }
    }
    pub fn rename_output(&mut self, node: &NodeId, name: &str) -> Result<(), String> {
        if name.len() > 128 || name.chars().any(char::is_control) {
            return Err("Use a name of at most 128 characters".into());
        }
        let Some(DocumentNode {
            kind: DocumentNodeKind::Output(output),
            ..
        }) = self.nodes.get_mut(node)
        else {
            return Err("Select an output".into());
        };
        output.name = name.trim().to_owned();
        Ok(())
    }

    pub fn sound_definition(
        &self,
        node: &NodeId,
    ) -> Option<&crate::domain::instrument::InstrumentDefinition> {
        match self.nodes.get(node).map(|node| &node.kind) {
            Some(DocumentNodeKind::Sound(sound)) => Some(&sound.definition),
            _ => None,
        }
    }

    pub fn set_sound_definition(
        &mut self,
        node: &NodeId,
        definition: crate::domain::instrument::InstrumentDefinition,
    ) -> Result<crate::domain::instrument::InstrumentDefinition, String> {
        definition.validate()?;
        let Some(DocumentNode {
            kind: DocumentNodeKind::Sound(sound),
            ..
        }) = self.nodes.get_mut(node)
        else {
            return Err("Choose a Sound tile".into());
        };
        Ok(std::mem::replace(&mut sound.definition, definition))
    }

    pub fn node(&self, node: &NodeId) -> Option<&DocumentNode> {
        self.nodes.get(node)
    }

    pub fn set_flow_control(
        &mut self,
        node: &NodeId,
        control: tessera::prelude::FlowControlNode,
    ) -> Result<(), String> {
        let Some(DocumentNode {
            kind: DocumentNodeKind::FlowControl(current),
            ..
        }) = self.nodes.get_mut(node)
        else {
            return Err("This flow tile no longer exists".into());
        };
        if current.kind != control.kind {
            return Err("Editing cannot change the flow tile's kind".into());
        }
        *current = control;
        Ok(())
    }

    /// Bind a numeric pitch component after contextual note validation.
    pub(crate) fn bind_number_as_octave(
        &mut self,
        node: &NodeId,
        octave: i8,
    ) -> Result<(), String> {
        if !(-1..=9).contains(&octave) {
            return Err("Octave must be between -1 and 9.".into());
        }
        let Some(DocumentNode {
            kind: DocumentNodeKind::Atom(atom),
            ..
        }) = self.nodes.get_mut(node)
        else {
            return Err("Missing numeric tile.".into());
        };
        if atom.atom.numeric_rational().is_none() {
            return Err("Only a free number can become a note's octave.".into());
        }
        atom.atom = AtomValue::Octave(octave);
        Ok(())
    }

    /// Changes an atom's value without changing its identity, placement, or typed role.
    pub fn set_atom_value(&mut self, node: &NodeId, value: AtomValue) -> Result<AtomValue, String> {
        let Some(DocumentNode {
            kind: DocumentNodeKind::Atom(atom),
            ..
        }) = self.nodes.get_mut(node)
        else {
            return Err("Select an existing atom to edit its value.".into());
        };
        if std::mem::discriminant(&atom.atom) != std::mem::discriminant(&value)
            && !(atom.atom.numeric_rational().is_some() && value.numeric_rational().is_some())
        {
            return Err("Editing a value cannot change the tile's role.".into());
        }
        if matches!(&value, AtomValue::Ratio(value) if value.denominator <= 0) {
            return Err("A ratio must have a positive denominator.".into());
        }
        if matches!(value, AtomValue::Octave(octave) if !(-1..=9).contains(&octave)) {
            return Err("Octave must be between -1 and 9.".into());
        }
        if let (AtomValue::Modifier(previous), AtomValue::Modifier(next)) = (&atom.atom, &value) {
            if std::mem::discriminant(previous) != std::mem::discriminant(next) {
                return Err("Editing a modifier value cannot change its role.".into());
            }
            tessera::domain::stack::validate_modifier(next)
                .map_err(|error| format!("Invalid modifier value: {error:?}"))?;
        }
        Ok(std::mem::replace(&mut atom.atom, value))
    }

    pub fn location_of(&self, node: &NodeId) -> Option<NodeLocation> {
        self.node_locations.get(node).copied()
    }

    /// Move existing nodes as one atomic operation, preserving stable identities.
    /// Callers validate surface roles; occupancy and container cycles live here.
    pub fn relocate_nodes(
        &mut self,
        surfaces: &BoardSurfaces,
        destinations: &BTreeMap<NodeId, NodeLocation>,
    ) -> Result<(), String> {
        let mut next = self.clone();
        for (id, target) in destinations {
            let old = self.location_of(id).ok_or("The tile no longer exists")?;
            if !surfaces.contains(target.surface) {
                return Err("The destination container no longer exists".into());
            }
            let mut surface = target.surface;
            let mut seen = BTreeSet::new();
            while let Some(parent) = self.container_node_for_surface(surface) {
                if &parent == id || !seen.insert(parent.clone()) {
                    return Err("A container cannot be moved inside itself".into());
                }
                surface = destinations
                    .get(&parent)
                    .copied()
                    .or_else(|| self.location_of(&parent))
                    .ok_or("Missing parent container")?
                    .surface;
            }
            match old.address {
                PlacementAddress::BoardSlot(slot) => {
                    next.board_placements.remove(&(old.surface, slot));
                }
                PlacementAddress::StackIndex(index) => {
                    next.stack_placements.remove(&(old.surface, index));
                }
            }
        }
        for (id, target) in destinations {
            match target.address {
                PlacementAddress::BoardSlot(slot) => {
                    let footprint = root_board_tile_footprint(&next.nodes[id].kind);
                    if next.board_footprint_conflicts(target.surface, slot, footprint) {
                        return Err("That destination overlaps an existing tile".into());
                    }
                    next.board_placements
                        .insert((target.surface, slot), id.clone());
                }
                PlacementAddress::StackIndex(index) => {
                    if !next.is_stack_index_empty(target.surface, index) {
                        return Err("That destination is already occupied".into());
                    }
                    next.stack_placements
                        .insert((target.surface, index), id.clone());
                }
            }
            next.node_locations.insert(id.clone(), *target);
        }
        *self = next;
        Ok(())
    }

    /// Reassigns an existing set of stack positions atomically. No node or
    /// child surface is recreated, and all positions remain on the same surface.
    pub fn reorder_stack_nodes(
        &mut self,
        surface: BoardSurfaceId,
        order: &[NodeId],
    ) -> Result<Vec<NodeId>, String> {
        let mut positions = Vec::with_capacity(order.len());
        let mut seen = BTreeSet::new();
        for node in order {
            if !seen.insert(node.clone()) {
                return Err("A stack tile cannot appear twice in a move.".into());
            }
            let Some(NodeLocation {
                surface: found,
                address: PlacementAddress::StackIndex(index),
            }) = self.location_of(node)
            else {
                return Err("Only tiles inside the same container can be reordered here.".into());
            };
            if found != surface {
                return Err("A modifier group cannot move to another container.".into());
            }
            positions.push(index);
        }
        positions.sort();
        if positions
            .windows(2)
            .any(|p| p[0].0.checked_add(1) != Some(p[1].0))
        {
            return Err("A group move cannot cross an empty slot.".into());
        }
        let previous = positions
            .iter()
            .map(|index| self.stack_placements[&(surface, *index)].clone())
            .collect();
        for (index, node) in positions.into_iter().zip(order) {
            self.stack_placements.insert((surface, index), node.clone());
            self.node_locations.insert(
                node.clone(),
                NodeLocation {
                    surface,
                    address: PlacementAddress::StackIndex(index),
                },
            );
        }
        Ok(previous)
    }

    /// Returns the authored node occupying this address, if any.
    ///
    /// `None` means the slot is empty and may be used as a placement target; it
    /// does not mean an "empty tile" exists in the document.
    pub fn node_at_board_slot(&self, surface: BoardSurfaceId, slot: BoardSlot) -> Option<NodeId> {
        self.board_placements.get(&(surface, slot)).cloned()
    }

    pub fn node_at_stack_index(
        &self,
        surface: BoardSurfaceId,
        index: StackIndex,
    ) -> Option<NodeId> {
        self.stack_placements.get(&(surface, index)).cloned()
    }

    pub fn is_board_slot_empty(&self, surface: BoardSurfaceId, slot: BoardSlot) -> bool {
        self.node_at_board_slot(surface, slot).is_none()
    }

    pub fn is_stack_index_empty(&self, surface: BoardSurfaceId, index: StackIndex) -> bool {
        self.node_at_stack_index(surface, index).is_none()
    }

    /// Returns the child authoring surface owned by a container node.
    pub fn container_surface(&self, container: &NodeId) -> Option<BoardSurfaceId> {
        self.container_surfaces.get(container).copied()
    }

    pub fn container_node_for_surface(&self, surface: BoardSurfaceId) -> Option<NodeId> {
        self.container_surfaces
            .iter()
            .find(|(_, local)| **local == surface)
            .map(|(node, _)| node.clone())
    }

    /// Inserts a new authored tile/node into a surface slot.
    ///
    /// This is the document-side authorship operation: the slot is merely an
    /// address until this method succeeds. Container insertion also creates the
    /// container-local child surface that nested authoring will happen inside.
    ///
    /// Root-board placements reject footprint overlap (not only exact-anchor
    /// collision), matching Tessera export occupancy.
    pub fn insert_tile(
        &mut self,
        surfaces: &mut BoardSurfaces,
        surface: BoardSurfaceId,
        address: PlacementAddress,
        spawn: TileSpawnKind,
    ) -> Result<NodeId, DocumentGraphError> {
        if !surfaces.contains(surface) {
            return Err(DocumentGraphError::MissingSurface(surface));
        }
        if let TileSpawnKind::Sound { definition } = &spawn {
            definition
                .validate()
                .map_err(DocumentGraphError::InvalidNode)?;
        }
        self.ensure_placement_free(surface, address, &spawn)?;

        let node_id = self.alloc_node_id();
        let kind = match spawn {
            TileSpawnKind::Tile { prototype } => DocumentNodeKind::Tile(TileNode { prototype }),
            TileSpawnKind::Atom { atom } => DocumentNodeKind::Atom(AtomNode { atom }),
            TileSpawnKind::Container { kind } => {
                let local_surface = self.alloc_surface_id();
                let container_id = ContainerId::new(node_id.0.clone());
                surfaces
                    .insert(BoardSurface {
                        id: local_surface,
                        kind: BoardSurfaceKind::ContainerStack {
                            container: container_id,
                        },
                    })
                    .map_err(DocumentGraphError::Surface)?;
                self.container_surfaces
                    .insert(node_id.clone(), local_surface);
                DocumentNodeKind::Container(ContainerNode {
                    kind,
                    local_surface,
                })
            }
            TileSpawnKind::Output { name } => DocumentNodeKind::Output(OutputNode { name }),
            TileSpawnKind::Sound { definition } => DocumentNodeKind::Sound(Box::new(SoundNode {
                definition: *definition,
            })),
            TileSpawnKind::FlowControl { control } => DocumentNodeKind::FlowControl(control),
            TileSpawnKind::TrickInstance { prototype } => {
                DocumentNodeKind::TrickInstance(TrickInstanceNode { prototype })
            }
        };

        let node = DocumentNode {
            id: node_id.clone(),
            kind,
        };
        self.nodes.insert(node_id.clone(), node);
        match address {
            PlacementAddress::BoardSlot(slot) => {
                self.board_placements
                    .insert((surface, slot), node_id.clone());
            }
            PlacementAddress::StackIndex(index) => {
                self.stack_placements
                    .insert((surface, index), node_id.clone());
            }
        }
        self.node_locations
            .insert(node_id.clone(), NodeLocation { surface, address });
        Ok(node_id)
    }

    pub fn delete_subtree(
        &mut self,
        root: &NodeId,
        _surfaces: &mut BoardSurfaces,
    ) -> Result<DeletedSubtree, DocumentGraphError> {
        if !self.contains_node(root) {
            return Err(DocumentGraphError::MissingNode(root.clone()));
        }

        let mut deleted = DeletedSubtree::default();
        self.collect_subtree(root, &mut deleted.nodes);

        for node in deleted.nodes.iter() {
            if let Some(location) = self.node_locations.remove(node) {
                match location.address {
                    PlacementAddress::BoardSlot(slot) => {
                        self.board_placements.remove(&(location.surface, slot));
                    }
                    PlacementAddress::StackIndex(index) => {
                        self.stack_placements.remove(&(location.surface, index));
                    }
                }
            }
            if let Some(local_surface) = self.container_surfaces.remove(node) {
                deleted.surfaces.insert(local_surface);
            }
            self.nodes.remove(node);
        }

        Ok(deleted)
    }

    fn collect_subtree(&self, root: &NodeId, out: &mut BTreeSet<NodeId>) {
        if !out.insert(root.clone()) {
            return;
        }
        let Some(local_surface) = self.container_surfaces.get(root).copied() else {
            return;
        };

        let children = self
            .node_locations
            .iter()
            .filter_map(|(node, location)| {
                if location.surface == local_surface {
                    Some(node.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for child in children {
            self.collect_subtree(&child, out);
        }
    }

    pub fn allocate_node_id(&mut self) -> NodeId {
        self.alloc_node_id()
    }

    pub fn reserve_node_id(&mut self, id: &NodeId) {
        if let Some(stem) = id.0.strip_prefix("doc_") {
            if let Ok(value) = stem.parse::<u64>() {
                if value >= self.next_node_id {
                    self.next_node_id = value + 1;
                }
            }
        }
    }

    pub fn reserve_surface_id(&mut self, id: BoardSurfaceId) {
        if id.0 >= self.next_surface_id {
            self.next_surface_id = id.0 + 1;
        }
    }

    /// Re-inserts a node exactly as captured (preserves ids and container surfaces).
    pub fn restore_node_unchanged(
        &mut self,
        _surfaces: &mut BoardSurfaces,
        node: DocumentNode,
        location: NodeLocation,
    ) -> Result<(), DocumentGraphError> {
        let node_id = node.id.clone();
        if self.contains_node(&node_id) {
            return Err(DocumentGraphError::MissingNode(node_id));
        }
        if let DocumentNodeKind::Container(container) = &node.kind {
            self.container_surfaces
                .insert(node_id.clone(), container.local_surface);
        }
        self.nodes.insert(node_id.clone(), node);
        match location.address {
            PlacementAddress::BoardSlot(slot) => {
                self.board_placements
                    .insert((location.surface, slot), node_id.clone());
            }
            PlacementAddress::StackIndex(index) => {
                self.stack_placements
                    .insert((location.surface, index), node_id.clone());
            }
        }
        self.node_locations.insert(node_id, location);
        Ok(())
    }

    pub fn link_container_surface(&mut self, container: NodeId, local_surface: BoardSurfaceId) {
        self.container_surfaces.insert(container, local_surface);
    }

    pub fn insert_tile_at_id(
        &mut self,
        surfaces: &mut BoardSurfaces,
        surface: BoardSurfaceId,
        address: PlacementAddress,
        node_id: NodeId,
        spawn: TileSpawnKind,
    ) -> Result<(), DocumentGraphError> {
        if !surfaces.contains(surface) {
            return Err(DocumentGraphError::MissingSurface(surface));
        }
        if let TileSpawnKind::Sound { definition } = &spawn {
            definition
                .validate()
                .map_err(DocumentGraphError::InvalidNode)?;
        }
        self.ensure_placement_free(surface, address, &spawn)?;

        let kind = match spawn {
            TileSpawnKind::Tile { prototype } => DocumentNodeKind::Tile(TileNode { prototype }),
            TileSpawnKind::Atom { atom } => DocumentNodeKind::Atom(AtomNode { atom }),
            TileSpawnKind::Container { kind } => {
                let local_surface = self.alloc_surface_id();
                let container_id = ContainerId::new(node_id.0.clone());
                surfaces
                    .insert(BoardSurface {
                        id: local_surface,
                        kind: BoardSurfaceKind::ContainerStack {
                            container: container_id,
                        },
                    })
                    .map_err(DocumentGraphError::Surface)?;
                self.container_surfaces
                    .insert(node_id.clone(), local_surface);
                DocumentNodeKind::Container(ContainerNode {
                    kind,
                    local_surface,
                })
            }
            TileSpawnKind::Output { name } => DocumentNodeKind::Output(OutputNode { name }),
            TileSpawnKind::Sound { definition } => DocumentNodeKind::Sound(Box::new(SoundNode {
                definition: *definition,
            })),
            TileSpawnKind::FlowControl { control } => DocumentNodeKind::FlowControl(control),
            TileSpawnKind::TrickInstance { prototype } => {
                DocumentNodeKind::TrickInstance(TrickInstanceNode { prototype })
            }
        };

        let node = DocumentNode {
            id: node_id.clone(),
            kind,
        };
        self.nodes.insert(node_id.clone(), node);
        match address {
            PlacementAddress::BoardSlot(slot) => {
                self.board_placements
                    .insert((surface, slot), node_id.clone());
            }
            PlacementAddress::StackIndex(index) => {
                self.stack_placements
                    .insert((surface, index), node_id.clone());
            }
        }
        self.node_locations
            .insert(node_id, NodeLocation { surface, address });
        Ok(())
    }

    fn alloc_node_id(&mut self) -> NodeId {
        let id = NodeId::new(format!("doc_{}", self.next_node_id));
        self.next_node_id += 1;
        id
    }

    fn is_address_empty(&self, surface: BoardSurfaceId, address: PlacementAddress) -> bool {
        match address {
            PlacementAddress::BoardSlot(slot) => self.is_board_slot_empty(surface, slot),
            PlacementAddress::StackIndex(index) => self.is_stack_index_empty(surface, index),
        }
    }

    fn ensure_placement_free(
        &self,
        surface: BoardSurfaceId,
        address: PlacementAddress,
        spawn: &TileSpawnKind,
    ) -> Result<(), DocumentGraphError> {
        match address {
            PlacementAddress::StackIndex(_) => {
                if !self.is_address_empty(surface, address) {
                    return Err(DocumentGraphError::OccupiedAddress { surface, address });
                }
            }
            PlacementAddress::BoardSlot(slot) => {
                let footprint = root_board_tile_footprint(spawn);
                if self.board_footprint_conflicts(surface, slot, footprint) {
                    return Err(DocumentGraphError::OccupiedAddress { surface, address });
                }
            }
        }
        Ok(())
    }

    fn board_footprint_conflicts(
        &self,
        surface: BoardSurfaceId,
        slot: BoardSlot,
        footprint: TileFootprint,
    ) -> bool {
        for cell in footprint.occupied_cells(slot) {
            for ((placed_surface, anchor), node_id) in &self.board_placements {
                if *placed_surface != surface {
                    continue;
                }
                let Some(node) = self.nodes.get(node_id) else {
                    continue;
                };
                let existing = root_board_tile_footprint(&node.kind);
                if existing.occupies(*anchor, cell) {
                    return true;
                }
            }
        }
        false
    }

    fn alloc_surface_id(&mut self) -> BoardSurfaceId {
        let id = BoardSurfaceId(self.next_surface_id);
        self.next_surface_id += 1;
        id
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedSubtree {
    pub nodes: BTreeSet<NodeId>,
    pub surfaces: BTreeSet<BoardSurfaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentGraphError {
    MissingSurface(BoardSurfaceId),
    MissingNode(NodeId),
    OccupiedAddress {
        surface: BoardSurfaceId,
        address: PlacementAddress,
    },
    InvalidNode(String),
    Surface(crate::domain::board::BoardSurfaceError),
}
