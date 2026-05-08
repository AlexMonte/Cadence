#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadinessState {
    Unavailable,
    Loading,
    Ready,
    Degraded,
    Failed,
    Recovering,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubsystemReadiness {
    pub state: ReadinessState,
    pub detail: Option<String>,
}

impl SubsystemReadiness {
    pub(crate) fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            state: ReadinessState::Unavailable,
            detail: Some(detail.into()),
        }
    }

    pub(crate) fn loading(detail: impl Into<String>) -> Self {
        Self {
            state: ReadinessState::Loading,
            detail: Some(detail.into()),
        }
    }

    pub(crate) fn ready(detail: impl Into<String>) -> Self {
        Self {
            state: ReadinessState::Ready,
            detail: Some(detail.into()),
        }
    }

    pub(crate) fn failed(detail: impl Into<String>) -> Self {
        Self {
            state: ReadinessState::Failed,
            detail: Some(detail.into()),
        }
    }

    pub(crate) fn recovering(detail: impl Into<String>) -> Self {
        Self {
            state: ReadinessState::Recovering,
            detail: Some(detail.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorLifecycleState {
    pub backend: SubsystemReadiness,
    pub hydration: SubsystemReadiness,
    pub runtime: SubsystemReadiness,
    pub recovery: SubsystemReadiness,
}

impl Default for EditorLifecycleState {
    fn default() -> Self {
        Self {
            backend: SubsystemReadiness::loading("Connecting to live backend..."),
            hydration: SubsystemReadiness::loading("Waiting for initial editor snapshot..."),
            runtime: SubsystemReadiness::loading("Runtime bridge has not been refreshed yet."),
            recovery: SubsystemReadiness::ready("Recovery idle."),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorCommandTrace {
    pub next_operation_id: u64,
    pub active_operation_id: Option<u64>,
    pub active_operation_label: Option<String>,
    pub last_completed_operation_id: Option<u64>,
}

impl Default for EditorCommandTrace {
    fn default() -> Self {
        Self {
            next_operation_id: 1,
            active_operation_id: None,
            active_operation_label: None,
            last_completed_operation_id: None,
        }
    }
}
