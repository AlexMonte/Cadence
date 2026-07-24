use std::collections::BTreeSet;

use bevy_ecs::prelude::*;

use crate::infrastructure::TesseraCompiler;

use super::components::{NeedsCompile, TesseraTile, TileNodeKind, TilePlacement};
use super::events::{CompileFinished, CompileRequested, ValidateFinished};
use super::resources::{
    AuthoredProgram, CompiledIr, ResolvedProgram, TesseraBoard, TesseraCompilerSettings,
    TesseraDiagnostics, TileEntityMap,
};

pub fn sync_authored_from_board_system(
    board: Res<TesseraBoard>,
    mut authored: ResMut<AuthoredProgram>,
) {
    if !board.is_changed() {
        return;
    }
    authored.0 = board.authored_program();
}

pub fn resolve_authored_system(
    authored: Res<AuthoredProgram>,
    mut resolved: ResMut<ResolvedProgram>,
    mut diagnostics: ResMut<TesseraDiagnostics>,
) {
    let compiler = TesseraCompiler::new();
    match compiler.resolve(&authored.0) {
        Ok(program) => {
            resolved.0 = Some(program);
            diagnostics.clear();
        }
        Err(err) => {
            resolved.0 = None;
            diagnostics.set(err);
        }
    }
}

pub fn validate_authored_system(
    authored: Res<AuthoredProgram>,
    mut diagnostics: ResMut<TesseraDiagnostics>,
    mut finished: MessageWriter<ValidateFinished>,
) {
    let compiler = TesseraCompiler::new();
    let report = compiler.validate_authored(&authored.0);
    if report.is_valid() {
        diagnostics.clear();
        finished.write(ValidateFinished::Ok(report));
    } else {
        diagnostics.set(report.diagnostics.clone());
        finished.write(ValidateFinished::Err(report.diagnostics));
    }
}

pub fn compile_authored_system(
    authored: Res<AuthoredProgram>,
    settings: Res<TesseraCompilerSettings>,
    mut resolved: ResMut<ResolvedProgram>,
    mut compiled: ResMut<CompiledIr>,
    mut diagnostics: ResMut<TesseraDiagnostics>,
    mut finished: MessageWriter<CompileFinished>,
) {
    let compiler = TesseraCompiler::with_options(settings.options.clone());
    match compiler.compile_authored(&authored.0) {
        Ok(report) => {
            if let Ok(program) = compiler.resolve(&authored.0) {
                resolved.0 = Some(program);
            }
            compiled.0 = Some(report.ir.clone());
            diagnostics.clear();
            finished.write(CompileFinished::Ok(report.ir));
        }
        Err(err) => {
            compiled.0 = None;
            diagnostics.set(err.clone());
            finished.write(CompileFinished::Err(err));
        }
    }
}

pub fn compile_on_request_system(
    mut requests: MessageReader<CompileRequested>,
    mut board: ResMut<TesseraBoard>,
    mut authored: ResMut<AuthoredProgram>,
    settings: Res<TesseraCompilerSettings>,
    mut resolved: ResMut<ResolvedProgram>,
    mut compiled: ResMut<CompiledIr>,
    mut diagnostics: ResMut<TesseraDiagnostics>,
    mut compile_finished: MessageWriter<CompileFinished>,
    mut validate_finished: MessageWriter<ValidateFinished>,
) {
    if requests.read().next().is_none() {
        return;
    }

    authored.0 = board.authored_program();

    let compiler = TesseraCompiler::with_options(settings.options.clone());

    match compiler.compile_authored_pipeline(&authored.0) {
        Ok((resolved_program, validation, report)) => {
            resolved.0 = Some(resolved_program);
            compiled.0 = Some(report.ir.clone());
            diagnostics.clear();
            board.clear_dirty();
            validate_finished.write(ValidateFinished::Ok(validation));
            compile_finished.write(CompileFinished::Ok(report.ir));
        }
        Err(err) => {
            resolved.0 = None;
            compiled.0 = None;
            diagnostics.set(err.clone());
            validate_finished.write(ValidateFinished::Err(err.clone()));
            compile_finished.write(CompileFinished::Err(err));
        }
    }
}

pub fn sync_tiles_from_program_system(
    authored: Res<AuthoredProgram>,
    mut commands: Commands,
    mut entity_map: ResMut<TileEntityMap>,
    mut tiles: Query<(Entity, &TesseraTile, &mut TilePlacement, &mut TileNodeKind)>,
) {
    if !authored.is_changed() {
        return;
    }

    let current_ids: BTreeSet<_> = authored.0.root_surface.nodes.keys().cloned().collect();

    entity_map.0.retain(|node_id, entity| {
        if current_ids.contains(node_id) {
            return true;
        }
        commands.entity(*entity).despawn();
        false
    });

    for (node_id, node) in &authored.0.root_surface.nodes {
        let Some(placement) = authored.0.root_surface.placements.get(node_id) else {
            continue;
        };

        if let Some(entity) = entity_map.0.get(node_id).copied() {
            match tiles.get_mut(entity) {
                Ok((_, _, mut tile_placement, mut kind)) => {
                    tile_placement.slot = placement.slot;
                    tile_placement.footprint = placement.footprint;
                    kind.kind = node.clone();
                }
                Err(_) => {
                    entity_map.0.remove(node_id);
                }
            }
            if entity_map.0.contains_key(node_id) {
                continue;
            }
        }

        let entity = commands
            .spawn((
                TesseraTile {
                    node_id: node_id.clone(),
                },
                TilePlacement {
                    slot: placement.slot,
                    footprint: placement.footprint,
                },
                TileNodeKind { kind: node.clone() },
                NeedsCompile,
            ))
            .id();
        entity_map.0.insert(node_id.clone(), entity);
    }
}
