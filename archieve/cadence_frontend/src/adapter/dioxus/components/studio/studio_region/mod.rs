use dioxus::prelude::*;

use crate::application::editor::{EditorShellState, WorkspaceMode};

use super::backend_offline_banner::BackendOfflineBanner;
use super::composer::ComposerStudio;
use super::score_setup::ScoreSetup;

#[component]
pub(crate) fn StudioRegion(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    rsx! {
        section {
            class: "workspace-main",
            "aria-label": "Grid workspace",
            "data-workspace-mode": snapshot.workspace_mode.workspace_mode_attr(),
            if !snapshot.backend_available {
                BackendOfflineBanner { state }
            }
            if matches!(snapshot.workspace_mode, WorkspaceMode::Init) {
                ScoreSetup {
                    state,
                    snapshot: snapshot.clone(),
                }
            } else {
                ComposerStudio {
                    state,
                    snapshot: snapshot.clone(),
                }
            }
        }
    }
}
