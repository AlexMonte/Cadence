use bevy::prelude::*;
use cadence_core::domain::{rational::Time as MusicalTime, span::Span};
use tessera::{TesseraCompiler, prelude::PatternIr};

use crate::{
    app::{CadenceSet, ProjectState, TransportMode},
    diagnostics::{AppDiagnostic, DiagnosticStore},
    lowering::LoweredPatternSet,
};

#[derive(Resource, Debug, Clone)]
pub struct RuntimeState {
    pub transport: crate::transport::TransportState,
    pub compiled: Option<CompiledProject>,
    pub diagnostics: Vec<AppDiagnostic>,
    pub dirty: ProjectDirty,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            transport: crate::transport::TransportState::default(),
            compiled: None,
            diagnostics: Vec::new(),
            dirty: ProjectDirty {
                tessera: true,
                lowering: false,
                scene: true,
                runtime: false,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledProject {
    pub tessera_ir: PatternIr,
    pub lowered: Option<LoweredPatternSet>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProjectDirty {
    pub tessera: bool,
    pub lowering: bool,
    pub scene: bool,
    pub runtime: bool,
}

pub struct RuntimePlugin;

impl Plugin for RuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (compile_tessera_document, advance_transport)
                .chain()
                .in_set(CadenceSet::Runtime),
        );
    }
}

fn compile_tessera_document(
    mut project: ResMut<'_, crate::document::CadenceProject>,
    mut project_state: ResMut<'_, NextState<ProjectState>>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
) {
    if !project.runtime.dirty.tessera {
        return;
    }

    let compiler = TesseraCompiler::new();
    match compiler.compile_authored_ir(&project.document.tessera.authored_program) {
        Ok(ir) => {
            project.runtime.compiled = Some(CompiledProject {
                tessera_ir: ir,
                lowered: None,
            });
            project.runtime.dirty.tessera = false;
            project.runtime.dirty.lowering = true;
            *project_state = NextState::Pending(ProjectState::ProjectOpen);
        }
        Err(items) => {
            diagnostics
                .items
                .extend(items.into_iter().map(AppDiagnostic::Tessera));
            project.runtime.compiled = None;
            project.runtime.dirty.tessera = false;
        }
    }
}

fn advance_transport(
    time: Res<'_, Time>,
    mut project: ResMut<'_, crate::document::CadenceProject>,
    transport_mode: Res<'_, State<TransportMode>>,
) {
    if *transport_mode.get() != TransportMode::Playing {
        return;
    }

    let delta_seconds = time.delta_secs_f64();
    if delta_seconds <= 0.0 {
        return;
    }

    let bpm = project.document.playback.bpm.max(120.0);
    let delta_cycles = MusicalTime::new((delta_seconds * bpm) as i64, 60);
    let old_position = project.runtime.transport.position;
    let new_position = old_position + delta_cycles;
    let _window: Option<Span> = Span::new(old_position, new_position);
    project.runtime.transport.position = new_position;
}
