use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use bevy_ecs::reflect::ReflectResource;
use bevy_reflect::Reflect;

use crate::domain::{
    AuthoredTesseraProgram, BoardSlot, Diagnostic, InputEndpoint, NodeId, OutputEndpoint,
    PatternIr, RootRelation, SpatialSide, TesseraProgram,
};
use crate::infrastructure::CompileOptions;
use crate::infrastructure::board::{Board, BoardError, FlowCursor, TileHandle, TileRef, TileSlot};

/// Live authoring board — primary edit surface for Cadence.
///
/// This resource is the authoritative program state. Mutations go through [`TesseraBoard`];
/// [`AuthoredProgram`] is a derived snapshot used for compile and tile-entity sync.
#[derive(Resource, Default)]
pub struct TesseraBoard {
    inner: Board,
    revision: u64,
    dirty: bool,
}

impl TesseraBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_program(program: AuthoredTesseraProgram) -> Self {
        Self {
            inner: Board::from_program(program),
            revision: 0,
            dirty: false,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.touch();
    }

    pub fn board(&self) -> &Board {
        &self.inner
    }

    pub fn authored_program(&self) -> AuthoredTesseraProgram {
        self.inner.authored().clone()
    }

    pub fn start(&mut self, x: i32, y: i32) -> FlowCursor<'_> {
        self.inner.start(x, y)
    }

    pub fn at(&mut self, x: i32, y: i32) -> TileSlot<'_> {
        self.touch();
        self.inner.at(x, y)
    }

    pub fn replace_at(&mut self, x: i32, y: i32) -> TileSlot<'_> {
        self.touch();
        self.inner.replace_at(x, y)
    }

    pub fn tile_at(&self, slot: BoardSlot) -> Option<TileRef> {
        self.inner.tile_at(slot)
    }

    pub fn tile(&self, id: &NodeId) -> Option<TileRef> {
        self.inner.tile(id)
    }

    pub fn handle(&mut self, id: &NodeId) -> Option<TileHandle<'_>> {
        self.touch();
        self.inner.handle(id)
    }

    pub fn remove_at(&mut self, slot: BoardSlot) -> Option<TileRef> {
        self.touch();
        self.inner.remove_at(slot)
    }

    pub fn move_tile(&mut self, id: &NodeId, slot: BoardSlot) -> Result<TileRef, BoardError> {
        self.touch();
        self.inner.move_tile(id, slot)
    }

    pub fn bind_input_side(
        &mut self,
        tile: &TileRef,
        endpoint: InputEndpoint,
        side: SpatialSide,
    ) -> Result<&mut Self, BoardError> {
        self.touch();
        self.inner.bind_input_side(tile, endpoint, side)?;
        Ok(self)
    }

    pub fn bind_output_side(
        &mut self,
        tile: &TileRef,
        endpoint: OutputEndpoint,
        side: SpatialSide,
    ) -> Result<&mut Self, BoardError> {
        self.touch();
        self.inner.bind_output_side(tile, endpoint, side)?;
        Ok(self)
    }

    pub fn add_explicit_relation(&mut self, relation: RootRelation) -> &mut Self {
        self.touch();
        self.inner.add_explicit_relation(relation);
        self
    }

    fn touch(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }
}

/// Derived snapshot of [`TesseraBoard`] for compile and tile-entity sync.
///
/// Do not mutate `0` directly in host code — edits should go through [`TesseraBoard`].
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct AuthoredProgram(pub AuthoredTesseraProgram);

impl AuthoredProgram {
    pub fn new(program: AuthoredTesseraProgram) -> Self {
        Self(program)
    }

    pub fn from_board(board: Board) -> Self {
        Self(board.finish())
    }
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct ResolvedProgram(pub Option<TesseraProgram>);

#[derive(Resource, Default)]
pub struct CompiledIr(pub Option<PatternIr>);

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct TesseraDiagnostics(pub Vec<Diagnostic>);

impl TesseraDiagnostics {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn set(&mut self, diagnostics: Vec<Diagnostic>) {
        self.0 = diagnostics;
    }
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct TesseraCompilerSettings {
    pub options: CompileOptions,
}

impl Default for TesseraCompilerSettings {
    fn default() -> Self {
        Self {
            options: CompileOptions::default(),
        }
    }
}

/// Stable mapping from compiler node ids to tile entities.
#[derive(Resource, Default)]
pub struct TileEntityMap(pub BTreeMap<NodeId, Entity>);
