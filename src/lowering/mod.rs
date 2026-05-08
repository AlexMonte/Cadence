use std::collections::BTreeMap;

use bevy::prelude::*;
use cadence_core::domain::score::{ControlScore, Score};
use tessera::prelude::{NodeId, PatternIr};

use crate::{
    app::CadenceSet,
    diagnostics::{AppDiagnostic, DiagnosticStore},
    document::{AssetLibraryState, PlaybackTarget},
    runtime::CompiledProject,
};

#[derive(Debug, Clone)]
pub struct LoweredPatternSet {
    pub outputs: BTreeMap<NodeId, LoweredOutput>,
}

#[derive(Debug, Clone)]
pub struct LoweredOutput {
    pub score: Score,
    pub control: Option<ControlScore>,
    pub target: PlaybackTarget,
}

#[derive(Debug, Clone)]
pub struct LoweringPolicy {
    pub note_policy: NoteLoweringPolicy,
    pub control_policy: ControlLoweringPolicy,
    pub output_policy: OutputLoweringPolicy,
    pub random_seed_policy: RandomSeedPolicy,
}

impl Default for LoweringPolicy {
    fn default() -> Self {
        Self {
            note_policy: NoteLoweringPolicy::SampleId,
            control_policy: ControlLoweringPolicy::ControlsAsTracks,
            output_policy: OutputLoweringPolicy::OutputToMaster,
            random_seed_policy: RandomSeedPolicy::StablePerDocument,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NoteLoweringPolicy {
    SampleId,
    SynthPitch,
    Hybrid,
}

#[derive(Debug, Clone)]
pub enum ControlLoweringPolicy {
    ControlsAsTracks,
}

#[derive(Debug, Clone)]
pub enum OutputLoweringPolicy {
    OutputToTrack,
    OutputToBus,
    OutputToMaster,
}

#[derive(Debug, Clone)]
pub enum RandomSeedPolicy {
    StablePerDocument,
}

pub struct LoweringContext<'a> {
    pub policy: &'a LoweringPolicy,
    pub asset_library: &'a AssetLibraryState,
    pub diagnostics: Vec<AppDiagnostic>,
}

impl<'a> LoweringContext<'a> {
    pub fn new(policy: &'a LoweringPolicy, asset_library: &'a AssetLibraryState) -> Self {
        Self {
            policy,
            asset_library,
            diagnostics: Vec::new(),
        }
    }
}

pub fn lower_tessera_ir(ir: &PatternIr, context: &mut LoweringContext<'_>) -> LoweredPatternSet {
    let outputs = ir
        .outputs
        .iter()
        .map(|output| {
            (
                output.id.clone(),
                LoweredOutput {
                    score: Score::empty(),
                    control: None,
                    target: PlaybackTarget::Master,
                },
            )
        })
        .collect();

    let _ = context;
    LoweredPatternSet { outputs }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct LoweringConfig {
    pub policy: LoweringPolicy,
}

pub struct LoweringPlugin;

impl Plugin for LoweringPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoweringConfig>()
            .add_systems(Update, lower_compiled_project.in_set(CadenceSet::Lower));
    }
}

fn lower_compiled_project(
    mut project: ResMut<'_, crate::document::CadenceProject>,
    config: Res<'_, LoweringConfig>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
) {
    if !project.runtime.dirty.lowering {
        return;
    }

    let Some(ir) = project
        .runtime
        .compiled
        .as_ref()
        .map(|compiled| compiled.tessera_ir.clone())
    else {
        project.runtime.dirty.lowering = false;
        return;
    };

    let mut context = LoweringContext::new(&config.policy, &project.document.assets);
    let lowered = lower_tessera_ir(&ir, &mut context);
    diagnostics.items.extend(context.diagnostics);

    match &mut project.runtime.compiled {
        Some(compiled) => {
            compiled.lowered = Some(lowered);
        }
        None => {
            project.runtime.compiled = Some(CompiledProject {
                tessera_ir: ir,
                lowered: Some(lowered),
            });
        }
    }

    project.runtime.dirty.lowering = false;
    project.runtime.dirty.runtime = true;
}
