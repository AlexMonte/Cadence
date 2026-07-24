//! Tile-first board authoring API.
//!
//! ## Placement
//! - [`Board::at`] replaces any occupant at the target slot. Default id is `n_{x}_{y}` unless
//!   [`TileSlot::named`] is used.
//! - [`Board::replace_at`] always replaces the occupant. Default id is a fresh UUID unless named.
//! - [`Flow::source`] uses `.named(format!("source_{x}"))` for stable, unique ids per column.

use std::collections::BTreeMap;

use crate::domain::{
    AuthoredTesseraProgram, BoardSlot, Container, ContainerId, ContainerKind, ContainerSurfaceTile,
    FlowControlKind, FlowControlNode, InputEndpoint, NodeId, OutputEndpoint, OutputNode,
    RootPlacement, RootRelation, RootSurfaceNodeKind, SpatialSide, TileFootprint, TransformKind,
    TransformNode, default_spatial_bindings,
};

use super::placement::{container_id_for_node, insert_placed_node, remove_placed_node};
use super::sequence_stack::SequenceStack;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileRef {
    pub id: NodeId,
    pub slot: BoardSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
    SlotOccupied,
    UnknownTile,
    DuplicateId { existing_slot: BoardSlot },
}

#[derive(Debug, Clone, Default)]
pub struct Board {
    program: AuthoredTesseraProgram,
    slot_to_node: BTreeMap<BoardSlot, NodeId>,
}

impl Board {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_program(program: AuthoredTesseraProgram) -> Self {
        let mut slot_to_node = BTreeMap::new();
        for (node_id, placement) in &program.root_surface.placements {
            index_placement_cells(&mut slot_to_node, node_id, placement);
        }
        Self {
            program,
            slot_to_node,
        }
    }

    pub fn authored(&self) -> &AuthoredTesseraProgram {
        &self.program
    }

    pub fn at(&mut self, x: i32, y: i32) -> TileSlot<'_> {
        TileSlot {
            board: self,
            slot: BoardSlot::new(x, y),
            name: None,
            replace: false,
            footprint: TileFootprint::unit(),
        }
    }

    pub fn replace_at(&mut self, x: i32, y: i32) -> TileSlot<'_> {
        TileSlot {
            board: self,
            slot: BoardSlot::new(x, y),
            name: None,
            replace: true,
            footprint: TileFootprint::unit(),
        }
    }

    pub fn start(&mut self, x: i32, y: i32) -> FlowCursor<'_> {
        FlowCursor {
            board: self,
            slot: BoardSlot::new(x, y),
        }
    }

    pub fn tile_at(&self, slot: BoardSlot) -> Option<TileRef> {
        self.slot_to_node.get(&slot).map(|id| TileRef {
            id: id.clone(),
            slot,
        })
    }

    pub fn tile(&self, id: &NodeId) -> Option<TileRef> {
        let placement = self.program.root_surface.placements.get(id)?;
        Some(TileRef {
            id: id.clone(),
            slot: placement.slot,
        })
    }

    pub fn handle(&mut self, id: &NodeId) -> Option<TileHandle<'_>> {
        let slot = self.program.root_surface.placements.get(id)?.slot;
        Some(TileHandle {
            board: self,
            id: id.clone(),
            slot,
        })
    }

    pub fn remove_at(&mut self, slot: BoardSlot) -> Option<TileRef> {
        let id = self.slot_to_node.get(&slot)?.clone();
        let placement = self.program.root_surface.placements.get(&id)?.clone();
        unindex_placement_cells(&mut self.slot_to_node, &id, &placement);
        remove_placed_node(&mut self.program, &id);
        Some(TileRef {
            id,
            slot: placement.slot,
        })
    }

    pub fn attach_neighbor<F>(
        &mut self,
        from: &TileRef,
        side: SpatialSide,
        build: F,
    ) -> Result<TileRef, BoardError>
    where
        F: FnOnce(TileSlot<'_>) -> Result<TileRef, BoardError>,
    {
        self.ensure_live_tile(from)?;
        let from_placement = self
            .program
            .root_surface
            .placements
            .get(&from.id)
            .expect("live tile has placement")
            .clone();
        let neighbor_footprint = TileFootprint::unit();
        let anchor = from_placement.footprint.anchor_for_neighbor(
            from_placement.slot,
            side,
            neighbor_footprint,
        );
        let placed = build(TileSlot {
            board: self,
            slot: anchor,
            name: None,
            replace: true,
            footprint: neighbor_footprint,
        })?;
        let output = default_output_on_side(&self.program, &from.id, side);
        let input = default_input_on_side(&self.program, &placed.id, side.opposite());
        if let Some(output) = output {
            self.bind_output_side(from, output, side)?;
        }
        if let Some(input) = input {
            self.bind_input_side(&placed, input, side.opposite())?;
        }
        Ok(placed)
    }

    pub fn move_tile(&mut self, id: &NodeId, slot: BoardSlot) -> Result<TileRef, BoardError> {
        let Some(placement) = self.program.root_surface.placements.get(id).cloned() else {
            return Err(BoardError::UnknownTile);
        };
        let new_placement = RootPlacement {
            slot,
            footprint: placement.footprint,
        };
        for cell in new_placement.footprint.occupied_cells(new_placement.slot) {
            if let Some(occupant) = self.slot_to_node.get(&cell) {
                if occupant != id {
                    return Err(BoardError::SlotOccupied);
                }
            }
        }
        unindex_placement_cells(&mut self.slot_to_node, id, &placement);
        self.program
            .root_surface
            .placements
            .get_mut(id)
            .unwrap()
            .slot = slot;
        index_placement_cells(&mut self.slot_to_node, id, &new_placement);
        Ok(TileRef {
            id: id.clone(),
            slot,
        })
    }

    pub fn bind_input_side(
        &mut self,
        tile: &TileRef,
        endpoint: InputEndpoint,
        side: SpatialSide,
    ) -> Result<&mut Self, BoardError> {
        self.ensure_live_tile(tile)?;
        self.program
            .root_surface
            .bindings
            .entry(tile.id.clone())
            .or_default()
            .inputs
            .insert(endpoint, side);
        Ok(self)
    }

    pub fn bind_output_side(
        &mut self,
        tile: &TileRef,
        endpoint: OutputEndpoint,
        side: SpatialSide,
    ) -> Result<&mut Self, BoardError> {
        self.ensure_live_tile(tile)?;
        self.program
            .root_surface
            .bindings
            .entry(tile.id.clone())
            .or_default()
            .outputs
            .insert(endpoint, side);
        Ok(self)
    }

    pub fn add_explicit_relation(&mut self, relation: RootRelation) -> &mut Self {
        self.program.root_surface.explicit_relations.push(relation);
        self
    }

    pub fn rotate_node_cw(&mut self, tile: &TileRef) -> Result<&mut Self, BoardError> {
        self.ensure_live_tile(tile)?;
        if let Some(bindings) = self.program.root_surface.bindings.get_mut(&tile.id) {
            bindings.rotate_cw();
        }
        Ok(self)
    }

    pub fn rotate_node_ccw(&mut self, tile: &TileRef) -> Result<&mut Self, BoardError> {
        self.ensure_live_tile(tile)?;
        if let Some(bindings) = self.program.root_surface.bindings.get_mut(&tile.id) {
            bindings.rotate_ccw();
        }
        Ok(self)
    }

    pub fn finish(self) -> AuthoredTesseraProgram {
        self.program
    }

    fn ensure_live_tile(&self, tile: &TileRef) -> Result<(), BoardError> {
        let Some(placement) = self.program.root_surface.placements.get(&tile.id) else {
            return Err(BoardError::UnknownTile);
        };
        if placement.slot != tile.slot {
            return Err(BoardError::UnknownTile);
        }
        Ok(())
    }

    fn ensure_unique_id(&self, id: &NodeId, slot: BoardSlot) -> Result<(), BoardError> {
        if let Some(placement) = self.program.root_surface.placements.get(id) {
            if placement.slot != slot {
                return Err(BoardError::DuplicateId {
                    existing_slot: placement.slot,
                });
            }
        }
        Ok(())
    }

    fn prepare_slot(&mut self, slot: BoardSlot, footprint: TileFootprint, replace: bool) {
        if replace {
            let occupants: BTreeMap<_, _> = footprint
                .occupied_cells(slot)
                .filter_map(|cell| self.slot_to_node.get(&cell).map(|id| (cell, id.clone())))
                .collect();
            for (_, id) in occupants {
                if let Some(placement) = self.program.root_surface.placements.get(&id).cloned() {
                    unindex_placement_cells(&mut self.slot_to_node, &id, &placement);
                    remove_placed_node(&mut self.program, &id);
                }
            }
            return;
        }
        for cell in footprint.occupied_cells(slot) {
            if self.slot_to_node.contains_key(&cell) {
                if let Some(id) = self.slot_to_node.get(&cell).cloned() {
                    if let Some(placement) = self.program.root_surface.placements.get(&id).cloned()
                    {
                        unindex_placement_cells(&mut self.slot_to_node, &id, &placement);
                        remove_placed_node(&mut self.program, &id);
                    }
                }
            }
        }
    }

    fn place_node(
        &mut self,
        slot: BoardSlot,
        footprint: TileFootprint,
        name: Option<String>,
        node: RootSurfaceNodeKind,
        replace: bool,
    ) -> Result<TileRef, BoardError> {
        self.prepare_slot(slot, footprint, replace);
        let id = NodeId::new(name.unwrap_or_else(|| id_for_placement(slot, replace)));
        self.ensure_unique_id(&id, slot)?;
        insert_placed_node(&mut self.program, id.clone(), slot, node, footprint);
        let placement = self.program.root_surface.placements.get(&id).unwrap();
        index_placement_cells(&mut self.slot_to_node, &id, placement);
        Ok(TileRef { id, slot })
    }

    fn place_container(
        &mut self,
        slot: BoardSlot,
        footprint: TileFootprint,
        name: Option<String>,
        kind: ContainerKind,
        stack: Vec<ContainerSurfaceTile>,
        replace: bool,
    ) -> Result<TileRef, BoardError> {
        self.prepare_slot(slot, footprint, replace);
        let id_str = name.unwrap_or_else(|| id_for_placement(slot, replace));
        let node_id = NodeId::new(id_str.clone());
        self.ensure_unique_id(&node_id, slot)?;
        let container_id = ContainerId::new(id_str);
        self.program.containers.insert(
            container_id.clone(),
            Container {
                kind,
                axis: crate::domain::ContainerAxis::Time,
                stack,
            },
        );
        let node = RootSurfaceNodeKind::Container {
            container: container_id,
        };
        insert_placed_node(&mut self.program, node_id.clone(), slot, node, footprint);
        let placement = self.program.root_surface.placements.get(&node_id).unwrap();
        index_placement_cells(&mut self.slot_to_node, &node_id, placement);
        Ok(TileRef { id: node_id, slot })
    }

    fn set_container_stack(
        &mut self,
        tile: &TileRef,
        kind: ContainerKind,
        stack: Vec<ContainerSurfaceTile>,
    ) -> Result<(), BoardError> {
        self.ensure_live_tile(tile)?;
        let Some(container_id) = container_id_for_node(&self.program, &tile.id) else {
            return Err(BoardError::UnknownTile);
        };
        self.program.containers.insert(
            container_id,
            Container {
                kind,
                axis: crate::domain::ContainerAxis::Time,
                stack,
            },
        );
        Ok(())
    }
}

pub struct TileHandle<'a> {
    board: &'a mut Board,
    id: NodeId,
    slot: BoardSlot,
}

impl TileHandle<'_> {
    pub fn id(&self) -> &NodeId {
        &self.id
    }

    pub fn slot(&self) -> BoardSlot {
        self.slot
    }

    pub fn as_ref(&self) -> TileRef {
        TileRef {
            id: self.id.clone(),
            slot: self.slot,
        }
    }

    pub fn bind_input_side(
        &mut self,
        endpoint: InputEndpoint,
        side: SpatialSide,
    ) -> Result<&mut Self, BoardError> {
        let tile = self.as_ref();
        self.board.bind_input_side(&tile, endpoint, side)?;
        Ok(self)
    }

    pub fn bind_output_side(
        &mut self,
        endpoint: OutputEndpoint,
        side: SpatialSide,
    ) -> Result<&mut Self, BoardError> {
        let tile = self.as_ref();
        self.board.bind_output_side(&tile, endpoint, side)?;
        Ok(self)
    }

    pub fn rotate_cw(&mut self) -> Result<&mut Self, BoardError> {
        let tile = self.as_ref();
        self.board.rotate_node_cw(&tile)?;
        Ok(self)
    }

    pub fn rotate_ccw(&mut self) -> Result<&mut Self, BoardError> {
        let tile = self.as_ref();
        self.board.rotate_node_ccw(&tile)?;
        Ok(self)
    }

    pub fn set_sequence(
        &mut self,
        stack: impl Into<Vec<ContainerSurfaceTile>>,
    ) -> Result<&mut Self, BoardError> {
        let tile = self.as_ref();
        self.board
            .set_container_stack(&tile, ContainerKind::Sequence, stack.into())?;
        Ok(self)
    }

    pub fn move_to(&mut self, slot: BoardSlot) -> Result<&mut Self, BoardError> {
        let tile = self.board.move_tile(&self.id, slot)?;
        self.slot = tile.slot;
        Ok(self)
    }

    pub fn remove(self) {
        self.board.remove_at(self.slot);
    }
}

pub struct TileSlot<'a> {
    board: &'a mut Board,
    slot: BoardSlot,
    name: Option<String>,
    replace: bool,
    footprint: TileFootprint,
}

impl TileSlot<'_> {
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn footprint(mut self, footprint: TileFootprint) -> Self {
        self.footprint = footprint;
        self
    }

    pub fn sequence(
        self,
        stack: impl Into<Vec<ContainerSurfaceTile>>,
    ) -> Result<TileRef, BoardError> {
        self.board.place_container(
            self.slot,
            self.footprint,
            self.name,
            ContainerKind::Sequence,
            stack.into(),
            self.replace,
        )
    }

    pub fn alternate(
        self,
        stack: impl Into<Vec<ContainerSurfaceTile>>,
    ) -> Result<TileRef, BoardError> {
        self.board.place_container(
            self.slot,
            self.footprint,
            self.name,
            ContainerKind::Alternate,
            stack.into(),
            self.replace,
        )
    }

    pub fn layer_container(
        self,
        stack: impl Into<Vec<ContainerSurfaceTile>>,
    ) -> Result<TileRef, BoardError> {
        self.board.place_container(
            self.slot,
            self.footprint,
            self.name,
            ContainerKind::Layer,
            stack.into(),
            self.replace,
        )
    }

    pub fn slow(self) -> Result<TileRef, BoardError> {
        self.transform(TransformKind::Slow)
    }

    pub fn fast(self) -> Result<TileRef, BoardError> {
        self.transform(TransformKind::Fast)
    }

    pub fn rev(self) -> Result<TileRef, BoardError> {
        self.transform(TransformKind::Rev)
    }

    pub fn gain(self) -> Result<TileRef, BoardError> {
        self.transform(TransformKind::Gain)
    }

    pub fn attack(self) -> Result<TileRef, BoardError> {
        self.transform(TransformKind::Attack)
    }

    pub fn transpose(self) -> Result<TileRef, BoardError> {
        self.transform(TransformKind::Transpose)
    }

    pub fn transform(self, kind: TransformKind) -> Result<TileRef, BoardError> {
        let node = RootSurfaceNodeKind::Transform(TransformNode::new(kind));
        self.board
            .place_node(self.slot, self.footprint, self.name, node, self.replace)
    }

    pub fn output(self) -> Result<TileRef, BoardError> {
        let node = RootSurfaceNodeKind::Output(OutputNode::default());
        self.board
            .place_node(self.slot, self.footprint, self.name, node, self.replace)
    }

    pub fn flow_control(self, kind: FlowControlKind) -> Result<TileRef, BoardError> {
        let node = RootSurfaceNodeKind::FlowControl(FlowControlNode::new(kind));
        self.board
            .place_node(self.slot, self.footprint, self.name, node, self.replace)
    }

    pub fn layer(
        self,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<TileRef, BoardError> {
        let mut node = FlowControlNode::new(FlowControlKind::Layer);
        node.members.inputs.insert(
            crate::domain::PortGroupId::new("streams"),
            members
                .into_iter()
                .map(crate::domain::PortMemberId::new)
                .collect(),
        );
        let node = RootSurfaceNodeKind::FlowControl(node);
        self.board
            .place_node(self.slot, self.footprint, self.name, node, self.replace)
    }

    pub fn split(
        self,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<TileRef, BoardError> {
        let mut node = FlowControlNode::new(FlowControlKind::Split);
        node.members.outputs.insert(
            crate::domain::PortGroupId::new("branches"),
            members
                .into_iter()
                .map(crate::domain::PortMemberId::new)
                .collect(),
        );
        let node = RootSurfaceNodeKind::FlowControl(node);
        self.board
            .place_node(self.slot, self.footprint, self.name, node, self.replace)
    }

    pub fn route(
        self,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<TileRef, BoardError> {
        let mut node = FlowControlNode::new(FlowControlKind::Route);
        node.members.outputs.insert(
            crate::domain::PortGroupId::new("routes"),
            members
                .into_iter()
                .map(crate::domain::PortMemberId::new)
                .collect(),
        );
        let node = RootSurfaceNodeKind::FlowControl(node);
        self.board
            .place_node(self.slot, self.footprint, self.name, node, self.replace)
    }
}

pub struct FlowCursor<'a> {
    board: &'a mut Board,
    slot: BoardSlot,
}

impl FlowCursor<'_> {
    pub fn sequence(self, stack: impl Into<Vec<ContainerSurfaceTile>>) -> Result<Self, BoardError> {
        self.board
            .replace_at(self.slot.x, self.slot.y)
            .sequence(stack)?;
        Ok(self.east())
    }

    pub fn slow(self) -> Result<Self, BoardError> {
        self.board.replace_at(self.slot.x, self.slot.y).slow()?;
        Ok(self.east())
    }

    pub fn fast(self) -> Result<Self, BoardError> {
        self.board.replace_at(self.slot.x, self.slot.y).fast()?;
        Ok(self.east())
    }

    pub fn transform(self, kind: TransformKind) -> Result<Self, BoardError> {
        self.board
            .replace_at(self.slot.x, self.slot.y)
            .transform(kind)?;
        Ok(self.east())
    }

    pub fn output(self) -> Result<(), BoardError> {
        self.board.replace_at(self.slot.x, self.slot.y).output()?;
        Ok(())
    }

    pub fn east(self) -> Self {
        Self {
            board: self.board,
            slot: self.slot.offset(1, 0),
        }
    }

    pub fn north(self) -> Self {
        Self {
            board: self.board,
            slot: self.slot.offset(0, -1),
        }
    }

    pub fn south(self) -> Self {
        Self {
            board: self.board,
            slot: self.slot.offset(0, 1),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Flow {
    board: Board,
    next_x: i32,
    y: i32,
}

impl Flow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source(mut self, container: BuiltContainer) -> Result<Self, BoardError> {
        let BuiltContainer { kind, stack } = container;
        let slot = self
            .board
            .at(self.next_x, self.y)
            .named(format!("source_{}", self.next_x));
        match kind {
            ContainerKind::Sequence => {
                slot.sequence(stack)?;
            }
            ContainerKind::Alternate => {
                slot.alternate(stack)?;
            }
            ContainerKind::Layer => {
                slot.layer_container(stack)?;
            }
        }
        self.next_x += 1;
        Ok(self)
    }

    pub fn then(mut self, kind: TransformKind) -> Result<Self, BoardError> {
        self.board.replace_at(self.next_x, self.y).transform(kind)?;
        self.next_x += 1;
        Ok(self)
    }

    pub fn output(mut self) -> Result<AuthoredTesseraProgram, BoardError> {
        self.board.replace_at(self.next_x, self.y).output()?;
        Ok(self.board.finish())
    }

    pub fn into_authored_program(self) -> AuthoredTesseraProgram {
        self.board.finish()
    }
}

#[derive(Debug, Clone)]
pub struct BuiltContainer {
    kind: ContainerKind,
    stack: Vec<ContainerSurfaceTile>,
}

#[derive(Debug, Clone, Default)]
pub struct Sequence {
    inner: SequenceStack,
}

impl Sequence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notes(mut self, values: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.inner = self.inner.notes(values);
        self
    }

    pub fn note(self, value: impl AsRef<str>) -> Self {
        Self {
            inner: self.inner.note(value),
        }
    }

    pub fn scalar(self, value: i64) -> Self {
        Self {
            inner: self.inner.scalar(value),
        }
    }

    pub fn rest(self) -> Self {
        Self {
            inner: self.inner.rest(),
        }
    }

    pub fn elongate(self, value: i64) -> Self {
        Self {
            inner: self.inner.elongate(value),
        }
    }

    pub fn chain(self, other: Sequence) -> Self {
        Self {
            inner: self.inner.extend(other.inner),
        }
    }

    pub fn build(self) -> BuiltContainer {
        BuiltContainer {
            kind: ContainerKind::Sequence,
            stack: self.inner.build(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Alternate {
    inner: SequenceStack,
}

impl Alternate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notes(mut self, values: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.inner = self.inner.notes(values);
        self
    }

    pub fn build(self) -> BuiltContainer {
        BuiltContainer {
            kind: ContainerKind::Alternate,
            stack: self.inner.build(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Layer {
    inner: SequenceStack,
}

impl Layer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notes(mut self, values: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.inner = self.inner.notes(values);
        self
    }

    pub fn build(self) -> BuiltContainer {
        BuiltContainer {
            kind: ContainerKind::Layer,
            stack: self.inner.build(),
        }
    }
}

fn id_for_placement(slot: BoardSlot, replace: bool) -> String {
    if replace {
        fresh_id()
    } else {
        default_id_for_slot(slot)
    }
}

fn fresh_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_id_for_slot(slot: BoardSlot) -> String {
    format!("n_{}_{}", slot.x, slot.y)
}

fn index_placement_cells(
    slot_to_node: &mut BTreeMap<BoardSlot, NodeId>,
    node_id: &NodeId,
    placement: &RootPlacement,
) {
    for cell in placement.footprint.occupied_cells(placement.slot) {
        slot_to_node.insert(cell, node_id.clone());
    }
}

fn unindex_placement_cells(
    slot_to_node: &mut BTreeMap<BoardSlot, NodeId>,
    node_id: &NodeId,
    placement: &RootPlacement,
) {
    for cell in placement.footprint.occupied_cells(placement.slot) {
        if slot_to_node.get(&cell) == Some(node_id) {
            slot_to_node.remove(&cell);
        }
    }
}

fn default_output_on_side(
    program: &AuthoredTesseraProgram,
    node_id: &NodeId,
    side: SpatialSide,
) -> Option<OutputEndpoint> {
    if let Some(bindings) = program.root_surface.bindings.get(node_id) {
        for (endpoint, bound_side) in &bindings.outputs {
            if *bound_side == side {
                return Some(endpoint.clone());
            }
        }
    }
    let node = program.root_surface.nodes.get(node_id)?;
    default_spatial_bindings(node)
        .outputs
        .into_iter()
        .find_map(|(endpoint, bound_side)| (bound_side == side).then_some(endpoint))
}

fn default_input_on_side(
    program: &AuthoredTesseraProgram,
    node_id: &NodeId,
    side: SpatialSide,
) -> Option<InputEndpoint> {
    if let Some(bindings) = program.root_surface.bindings.get(node_id) {
        for (endpoint, bound_side) in &bindings.inputs {
            if *bound_side == side {
                return Some(endpoint.clone());
            }
        }
    }
    let node = program.root_surface.nodes.get(node_id)?;
    default_spatial_bindings(node)
        .inputs
        .into_iter()
        .find_map(|(endpoint, bound_side)| (bound_side == side).then_some(endpoint))
}
