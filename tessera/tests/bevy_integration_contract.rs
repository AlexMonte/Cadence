#![cfg(feature = "bevy")]

use bevy_app::App;
use bevy_ecs::prelude::*;
use tessera::bevy::{
    AuthoredProgram, CompileFinished, CompileRequested, CompiledIr, TesseraBoard, TesseraPlugin,
    TesseraTile, TileEntityMap, register_tessera_types, type_registry_contains,
};
use tessera::prelude::*;

#[test]
fn bevy_plugin_registers_core_types() {
    let mut app = App::new();
    app.add_plugins(TesseraPlugin);
    register_tessera_types(&mut app);
    assert!(type_registry_contains::<AuthoredTesseraProgram>(&app));
    assert!(type_registry_contains::<BoardSlot>(&app));
    assert!(type_registry_contains::<Diagnostic>(&app));
}

#[test]
fn tessera_board_compiles_via_bevy_systems() {
    let mut app = App::new();
    app.add_plugins(TesseraPlugin);
    app.update();

    {
        let mut board = app.world_mut().resource_mut::<TesseraBoard>();
        board
            .at(0, 0)
            .sequence(notes(["a", "b", "c"]))
            .expect("sequence");
        board.at(1, 0).output().expect("output");
    }

    app.world_mut().write_message(CompileRequested::forced());
    app.update();

    let compiled = app.world().resource::<CompiledIr>();
    assert!(compiled.0.is_some());
    assert_eq!(compiled.0.as_ref().unwrap().outputs.len(), 1);

    let authored = app.world().resource::<AuthoredProgram>();
    assert_eq!(authored.0.root_surface.nodes.len(), 2);

    let mut saw_ok = false;
    let mut messages = app.world_mut().resource_mut::<Messages<CompileFinished>>();
    for message in messages.drain() {
        if matches!(message, CompileFinished::Ok(_)) {
            saw_ok = true;
        }
    }
    assert!(saw_ok);
}

#[test]
fn tessera_board_move_tile_keeps_entity_stable() {
    let mut app = App::new();
    app.add_plugins(TesseraPlugin);
    app.update();

    let tile_id = {
        let mut board = app.world_mut().resource_mut::<TesseraBoard>();
        let tile = board.at(0, 0).sequence(notes(["a"])).expect("sequence");
        board.at(1, 0).output().expect("output");
        tile.id
    };
    app.world_mut().write_message(CompileRequested::forced());
    app.update();

    let entity_before = app
        .world()
        .resource::<TileEntityMap>()
        .0
        .get(&tile_id)
        .copied()
        .expect("tile entity should exist");

    {
        let mut board = app.world_mut().resource_mut::<TesseraBoard>();
        board
            .handle(&tile_id)
            .expect("tile handle")
            .move_to(slot(2, 0))
            .expect("move tile");
    }
    app.update();

    let entity_after = app
        .world()
        .resource::<TileEntityMap>()
        .0
        .get(&tile_id)
        .copied()
        .expect("moved tile entity should remain");
    assert_eq!(entity_before, entity_after);

    let placement = app
        .world()
        .resource::<AuthoredProgram>()
        .0
        .root_surface
        .placements
        .get(&tile_id)
        .expect("moved placement");
    assert_eq!(placement.slot, slot(2, 0));
}

#[test]
fn sync_tiles_respawns_after_external_despawn() {
    let mut app = App::new();
    app.add_plugins(TesseraPlugin);
    app.update();

    let tile_id = {
        let mut board = app.world_mut().resource_mut::<TesseraBoard>();
        let tile = board.at(0, 0).sequence(notes(["a"])).expect("sequence");
        board.at(1, 0).output().expect("output");
        tile.id
    };
    app.update();

    let entity_before = app
        .world()
        .resource::<TileEntityMap>()
        .0
        .get(&tile_id)
        .copied()
        .expect("tile entity should exist");

    app.world_mut().entity_mut(entity_before).despawn();

    {
        let mut board = app.world_mut().resource_mut::<TesseraBoard>();
        board.at(2, 0).output().expect("trigger board sync");
    }
    app.update();

    let entity_after = app
        .world()
        .resource::<TileEntityMap>()
        .0
        .get(&tile_id)
        .copied()
        .expect("tile entity should respawn after external despawn");
    assert_ne!(entity_before, entity_after);
    assert!(app.world().get::<TesseraTile>(entity_after).is_some());
}
