use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Default)]
pub struct DiagnosticStore {
    pub items: Vec<AppDiagnostic>,
}

#[derive(Debug, Clone)]
pub enum AppDiagnostic {
    Tessera(tessera::prelude::Diagnostic),
    Lowering(LoweringDiagnostic),
    Runtime(RuntimeDiagnostic),
}

#[derive(Debug, Clone)]
pub enum LoweringDiagnostic {
    UnsupportedPatternNode { node: String },
    UnknownOutputRoute { output: String },
    UnknownSample { sample: String },
    InvalidControlMapping { key: String },
}

#[derive(Debug, Clone)]
pub enum RuntimeDiagnostic {
    ProjectionFailed { detail: String },
    AudioScheduleFailed { detail: String },
}

pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiagnosticStore>();
    }
}
