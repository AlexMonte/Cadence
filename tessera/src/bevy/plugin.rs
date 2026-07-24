use bevy_app::{App, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;

use super::components::{NeedsCompile, TesseraTile, TileNodeKind, TilePlacement};
use super::events::{CompileFinished, CompileRequested, ValidateFinished};
use super::reflect::register_tessera_types;
use super::resources::{
    AuthoredProgram, CompiledIr, ResolvedProgram, TesseraBoard, TesseraCompilerSettings,
    TesseraDiagnostics, TileEntityMap,
};
use super::systems::{
    compile_on_request_system, sync_authored_from_board_system, sync_tiles_from_program_system,
};

pub struct TesseraPlugin;

impl Plugin for TesseraPlugin {
    fn build(&self, app: &mut App) {
        register_tessera_types(app);
        app.init_resource::<TesseraBoard>()
            .init_resource::<AuthoredProgram>()
            .init_resource::<ResolvedProgram>()
            .init_resource::<CompiledIr>()
            .init_resource::<TesseraDiagnostics>()
            .init_resource::<TesseraCompilerSettings>()
            .init_resource::<TileEntityMap>()
            .register_type::<AuthoredProgram>()
            .register_type::<ResolvedProgram>()
            .register_type::<TesseraDiagnostics>()
            .register_type::<TesseraCompilerSettings>()
            .register_type::<TesseraTile>()
            .register_type::<TilePlacement>()
            .register_type::<TileNodeKind>()
            .register_type::<NeedsCompile>()
            .register_type::<CompileRequested>()
            .add_message::<CompileRequested>()
            .add_message::<CompileFinished>()
            .add_message::<ValidateFinished>()
            .add_systems(
                Update,
                (
                    sync_authored_from_board_system,
                    compile_on_request_system,
                    sync_tiles_from_program_system,
                )
                    .chain(),
            );
    }
}
