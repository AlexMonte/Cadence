use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Default)]
pub struct DiagnosticStore {
    pub items: Vec<LayeredDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticPhase {
    TesseraCompile,
    Lowering,
    Runtime,
    Transaction,
}

#[derive(Debug, Clone)]
pub struct LayeredDiagnostic {
    pub phase: DiagnosticPhase,
    pub diagnostic: AppDiagnostic,
}

#[derive(Debug, Clone)]
pub enum AppDiagnostic {
    Tessera(tessera::prelude::Diagnostic),
    Host(HostDiagnostic),
    Lowering(LoweringDiagnostic),
    Runtime(RuntimeDiagnostic),
    Transaction(TransactionDiagnostic),
}

#[derive(Debug, Clone)]
pub enum HostDiagnostic {
    BoardExportFailed { detail: String },
    TransactionRejected { message: String },
}

#[derive(Debug, Clone)]
pub enum TransactionDiagnostic {
    Rejected { message: String },
}

#[derive(Debug, Clone)]
pub enum LoweringDiagnostic {
    UnsupportedPatternNode { node: String },
    InvalidControlMapping { key: String },
}

#[derive(Debug, Clone)]
pub enum RuntimeDiagnostic {
    ProjectionFailed { detail: String },
}

impl DiagnosticStore {
    pub fn has_errors(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.items.is_empty() {
            "No diagnostics".into()
        } else {
            format!("{} diagnostic(s)", self.items.len())
        }
    }

    pub fn replace_phase(
        &mut self,
        phase: DiagnosticPhase,
        diagnostics: impl IntoIterator<Item = AppDiagnostic>,
    ) {
        self.items.retain(|item| item.phase != phase);
        self.items.extend(
            diagnostics
                .into_iter()
                .map(|diagnostic| LayeredDiagnostic { phase, diagnostic }),
        );
    }

    pub fn clear_phase(&mut self, phase: DiagnosticPhase) {
        self.items.retain(|item| item.phase != phase);
    }
}

mod hierarchy_audit;

pub use hierarchy_audit::HierarchyAuditPlugin;
