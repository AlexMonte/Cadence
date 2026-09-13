//! Readiness is projected from the actual audio queue, independently of which
//! cycle the user is browsing. This module does not accept or publish scores.
use super::{RuntimePreviewSnapshot, RuntimeState};
use crate::infrastructure::diagnostics::{
    AppDiagnostic, DiagnosticStore, HostDiagnostic, LoweringDiagnostic, TransactionDiagnostic,
};
use bevy::prelude::*;
use cadence::{
    infrastructure::playback::{PlaybackRuntime, PlaybackState},
    prelude::Time as CycleTime,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Readiness {
    #[default]
    Checking,
    Accepted,
    Rejected,
    PreviewProblem,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaybackFeedback {
    pub readiness: Readiness,
    pub playing: bool,
    pub has_valid_version: bool,
    pub pending_cycle: Option<CycleTime>,
    pub detail: String,
}
impl PlaybackFeedback {
    pub fn summary(&self) -> String {
        match self.readiness {
            Readiness::Checking => "Checking edits…".into(),
            Readiness::PreviewProblem => "Timing preview unavailable · check the error".into(),
            Readiness::Rejected => {
                if let Some(cycle) = self.pending_cycle {
                    format!(
                        "Edit rejected · earlier valid edit queued for cycle {:.0}",
                        cycle.value() + 1.0
                    )
                } else if self.playing && self.has_valid_version {
                    "Edit rejected · previous version is still playing".into()
                } else if self.has_valid_version {
                    "Edit rejected · previous valid version kept".into()
                } else {
                    "Edit rejected · nothing ready to play".into()
                }
            }
            Readiness::Accepted => {
                if let Some(cycle) = self.pending_cycle {
                    format!(
                        "Queued · {} cycle {:.0}",
                        if self.playing {
                            "plays at"
                        } else {
                            "applies on playback at"
                        },
                        cycle.value() + 1.0
                    )
                } else if self.playing {
                    "Playing current version".into()
                } else {
                    "Edits accepted · ready to play".into()
                }
            }
        }
    }
}

fn first_error(diagnostics: &DiagnosticStore) -> Option<(Readiness, String)> {
    diagnostics.items.iter().find_map(|item| {
        let (kind, message) = match &item.diagnostic {
            AppDiagnostic::Transaction(TransactionDiagnostic::Info { .. }) => return None,
            AppDiagnostic::Transaction(TransactionDiagnostic::Rejected { message })
            | AppDiagnostic::Host(HostDiagnostic::TransactionRejected { message }) => {
                (Readiness::Rejected, message.clone())
            }
            AppDiagnostic::Host(HostDiagnostic::BoardExportFailed { detail }) => {
                (Readiness::Rejected, detail.clone())
            }
            AppDiagnostic::Tessera(error) => (Readiness::Rejected, error.message.clone()),
            AppDiagnostic::Lowering(LoweringDiagnostic::UnsupportedPatternNode { node }) => {
                (Readiness::Rejected, node.clone())
            }
            AppDiagnostic::Lowering(LoweringDiagnostic::InvalidControlMapping { key }) => {
                (Readiness::Rejected, format!("Unsupported control: {key}"))
            }
            AppDiagnostic::Runtime(
                crate::infrastructure::diagnostics::RuntimeDiagnostic::ProjectionFailed { detail },
            ) => (Readiness::PreviewProblem, detail.clone()),
        };
        Some((kind, message))
    })
}

pub(super) fn sync_feedback(
    playback: NonSend<PlaybackRuntime>,
    runtime: Res<RuntimeState>,
    diagnostics: Res<DiagnosticStore>,
    mut error: Local<Option<(Readiness, String)>>,
    mut snapshot: ResMut<RuntimePreviewSnapshot>,
) {
    if diagnostics.is_changed() {
        *error = first_error(&diagnostics);
    }
    let pending_cycle = playback.pending_revision_cycle();
    // The note lanes may stay pinned to a remote cycle for minutes. Queue state
    // still follows the audio clock and must not wait for a lane rebuild.
    if snapshot.pending_cycle != pending_cycle {
        snapshot.pending_cycle = pending_cycle;
    }
    let checking =
        runtime.needs_compile() || runtime.pending_ir.is_some() || runtime.proposed.is_some();
    let (readiness, detail) = if checking {
        (Readiness::Checking, "")
    } else if let Some((kind, message)) = &*error {
        (*kind, message.as_str())
    } else if runtime.compiled.is_some() {
        (Readiness::Accepted, "")
    } else {
        (Readiness::Checking, "")
    };
    let playing = playback.status().state == PlaybackState::Playing;
    let has_valid_version = runtime.compiled.is_some();
    let old = &snapshot.feedback;
    if old.readiness != readiness
        || old.playing != playing
        || old.has_valid_version != has_valid_version
        || old.pending_cycle != pending_cycle
        || old.detail != detail
    {
        let feedback = PlaybackFeedback {
            readiness,
            playing,
            has_valid_version,
            pending_cycle,
            detail: detail.into(),
        };
        snapshot.feedback = feedback;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feedback_never_calls_a_queued_or_rejected_edit_current() {
        let mut feedback = PlaybackFeedback {
            readiness: Readiness::Accepted,
            playing: true,
            has_valid_version: true,
            pending_cycle: Some(CycleTime::ONE),
            detail: String::new(),
        };
        assert_eq!(feedback.summary(), "Queued · plays at cycle 2");
        feedback.readiness = Readiness::Rejected;
        assert_eq!(
            feedback.summary(),
            "Edit rejected · earlier valid edit queued for cycle 2"
        );
        feedback.pending_cycle = None;
        assert_eq!(
            feedback.summary(),
            "Edit rejected · previous version is still playing"
        );
        feedback.playing = false;
        assert_eq!(
            feedback.summary(),
            "Edit rejected · previous valid version kept"
        );
        feedback.has_valid_version = false;
        assert_eq!(feedback.summary(), "Edit rejected · nothing ready to play");
    }
}
