use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};

use bevy_asset::{AssetLoadError, AssetPath, UntypedAssetId, UntypedHandle};
use bevy_ecs::prelude::*;

use crate::prepared::PreparedResourceId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DependencyId {
    pub resource: PreparedResourceId,
    pub index: usize,
    pub field: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DependencyRole {
    Blocking,
    Streaming,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyFailure {
    LoadFailed,
    TimedOut,
    RetryExhausted,
    FallbackFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyStatus {
    NotStarted,
    Loading,
    StateSafe,
    Ready,
    Failed(DependencyFailure),
}

impl DependencyStatus {
    pub fn is_state_safe(&self) -> bool {
        matches!(self, Self::StateSafe | Self::Ready)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FailurePolicy {
    Fatal,
    Ignore,
    Retry { attempts: u8, cooldown: Duration },
    Fallback { path: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateSafeThreshold {
    Ready,
    Loaded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcquisitionState {
    Primary,
    Retrying { attempt: u8 },
    WaitingRetry { next_attempt: u8 },
    Fallback,
    Exhausted,
}

#[derive(Clone)]
pub struct LoadUpDependency {
    pub field: &'static str,
    pub handle: UntypedHandle,
    pub role: DependencyRole,
    pub policy: FailurePolicy,
    pub state_safe_threshold: StateSafeThreshold,
    pub acquire: fn(&mut World) -> UntypedHandle,
    pub acquire_fallback: fn(&mut World, &'static str) -> UntypedHandle,
    pub replace: fn(&mut World, &UntypedHandle, UntypedHandle),
    pub primary_path: Option<&'static str>,
    pub timeout: Option<Duration>,
    /// True for handles inserted directly into `Assets<T>` rather than loaded by `AssetServer`.
    pub immediate: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyState {
    pub status: DependencyStatus,
    pub acquisition: AcquisitionState,
    pub retry_after: Option<Duration>,
    pub started_at: Duration,
    pub last_failure: Option<DependencyFailure>,
}

#[allow(
    dead_code,
    reason = "commands are part of the complete reducer lifecycle"
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DependencyCommand {
    LoadObserved(DependencyStatus),
    LoadFailed {
        failure: DependencyFailure,
        now: Duration,
    },
    RetryDeadlineReached {
        now: Duration,
    },
    RetryRequested {
        now: Duration,
    },
    FallbackStarted {
        now: Duration,
    },
    Released,
    Reloaded {
        now: Duration,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DependencyEffect {
    AcquirePrimary {
        attempt: u8,
    },
    AcquireFallback {
        path: &'static str,
    },
    ScheduleRetry {
        deadline: Duration,
    },
    Poll,
    StopPolling,
    ReportFailure {
        failure: DependencyFailure,
        terminal: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DependencyReduction {
    pub state: DependencyState,
    pub effects: Vec<DependencyEffect>,
}

/// Pure authoritative state transition for a dependency.
pub(crate) fn reduce_dependency(
    current: &DependencyState,
    policy: &FailurePolicy,
    command: DependencyCommand,
) -> DependencyReduction {
    let mut state = current.clone();
    let mut effects = Vec::new();

    match command {
        DependencyCommand::LoadObserved(status) => {
            state.status = status.clone();
            if status.is_ready() {
                effects.push(DependencyEffect::StopPolling);
            } else {
                effects.push(DependencyEffect::Poll);
            }
        }
        DependencyCommand::LoadFailed { failure, now } => {
            state.last_failure = Some(failure.clone());
            match policy {
                FailurePolicy::Fatal | FailurePolicy::Ignore => {
                    state.status = DependencyStatus::Failed(failure.clone());
                    state.acquisition = AcquisitionState::Exhausted;
                    state.retry_after = None;
                    effects.push(DependencyEffect::ReportFailure {
                        failure,
                        terminal: true,
                    });
                    effects.push(DependencyEffect::StopPolling);
                }
                FailurePolicy::Retry { attempts, cooldown } => {
                    let completed = match state.acquisition {
                        AcquisitionState::Retrying { attempt } => attempt,
                        AcquisitionState::WaitingRetry { next_attempt } => {
                            next_attempt.saturating_sub(1)
                        }
                        _ => 0,
                    };
                    effects.push(DependencyEffect::ReportFailure {
                        failure,
                        terminal: completed >= *attempts,
                    });
                    if completed < *attempts {
                        let next_attempt = completed + 1;
                        state.status = DependencyStatus::Loading;
                        if cooldown.is_zero() {
                            state.acquisition = AcquisitionState::Retrying {
                                attempt: next_attempt,
                            };
                            state.retry_after = None;
                            state.started_at = now;
                            effects.push(DependencyEffect::AcquirePrimary {
                                attempt: next_attempt,
                            });
                            effects.push(DependencyEffect::Poll);
                        } else {
                            let deadline = now.saturating_add(*cooldown);
                            state.acquisition = AcquisitionState::WaitingRetry { next_attempt };
                            state.retry_after = Some(deadline);
                            effects.push(DependencyEffect::ScheduleRetry { deadline });
                        }
                    } else {
                        state.status = DependencyStatus::Failed(DependencyFailure::RetryExhausted);
                        state.acquisition = AcquisitionState::Exhausted;
                        state.retry_after = None;
                        state.last_failure = Some(DependencyFailure::RetryExhausted);
                        effects.push(DependencyEffect::StopPolling);
                    }
                }
                FailurePolicy::Fallback { path } => {
                    if state.acquisition == AcquisitionState::Fallback {
                        state.status = DependencyStatus::Failed(DependencyFailure::FallbackFailed);
                        state.acquisition = AcquisitionState::Exhausted;
                        state.retry_after = None;
                        state.last_failure = Some(DependencyFailure::FallbackFailed);
                        effects.push(DependencyEffect::ReportFailure {
                            failure: DependencyFailure::FallbackFailed,
                            terminal: true,
                        });
                        effects.push(DependencyEffect::StopPolling);
                    } else {
                        state.status = DependencyStatus::Loading;
                        state.acquisition = AcquisitionState::Fallback;
                        state.retry_after = None;
                        state.started_at = now;
                        effects.push(DependencyEffect::ReportFailure {
                            failure,
                            terminal: false,
                        });
                        effects.push(DependencyEffect::AcquireFallback { path });
                        effects.push(DependencyEffect::Poll);
                    }
                }
            }
        }
        DependencyCommand::RetryDeadlineReached { now } => {
            if let AcquisitionState::WaitingRetry { next_attempt } = state.acquisition {
                if state.retry_after.is_some_and(|deadline| now >= deadline) {
                    state.status = DependencyStatus::Loading;
                    state.acquisition = AcquisitionState::Retrying {
                        attempt: next_attempt,
                    };
                    state.retry_after = None;
                    state.started_at = now;
                    effects.push(DependencyEffect::AcquirePrimary {
                        attempt: next_attempt,
                    });
                    effects.push(DependencyEffect::Poll);
                }
            }
        }
        DependencyCommand::RetryRequested { now } | DependencyCommand::Reloaded { now } => {
            if state.status.is_failed() || matches!(command, DependencyCommand::Reloaded { .. }) {
                state.status = DependencyStatus::Loading;
                state.acquisition = AcquisitionState::Primary;
                state.retry_after = None;
                state.started_at = now;
                state.last_failure = None;
                effects.push(DependencyEffect::AcquirePrimary { attempt: 0 });
                effects.push(DependencyEffect::Poll);
            }
        }
        DependencyCommand::FallbackStarted { now } => {
            state.status = DependencyStatus::Loading;
            state.acquisition = AcquisitionState::Fallback;
            state.retry_after = None;
            state.started_at = now;
            effects.push(DependencyEffect::Poll);
        }
        DependencyCommand::Released => {
            state.status = DependencyStatus::NotStarted;
            state.acquisition = AcquisitionState::Exhausted;
            state.retry_after = None;
            effects.push(DependencyEffect::StopPolling);
        }
    }

    DependencyReduction { state, effects }
}

#[derive(Clone)]
pub struct RuntimeDependency {
    pub id: DependencyId,
    pub resource_ordinal: u64,
    pub handle: UntypedHandle,
    pub role: DependencyRole,
    pub state: DependencyState,
    pub policy: FailurePolicy,
    pub state_safe_threshold: StateSafeThreshold,
    pub acquire: fn(&mut World) -> UntypedHandle,
    pub acquire_fallback: fn(&mut World, &'static str) -> UntypedHandle,
    pub replace: fn(&mut World, &UntypedHandle, UntypedHandle),
    pub prepared_handle: UntypedHandle,
    pub primary_path: Option<&'static str>,
    pub resolved_path: Option<String>,
    pub timeout: Option<Duration>,
    pub load_error: Option<Arc<AssetLoadError>>,
    pub immediate: bool,
}

impl RuntimeDependency {
    pub fn status(&self) -> &DependencyStatus {
        &self.state.status
    }

    pub fn gates_state_entry(&self) -> bool {
        self.role == DependencyRole::Blocking
    }

    pub fn is_settled(&self) -> bool {
        self.state.status.is_ready()
            || matches!(self.state.acquisition, AcquisitionState::Exhausted)
    }

    pub fn blocks_state_entry(&self) -> bool {
        self.state.status.is_failed()
            && matches!(self.state.acquisition, AcquisitionState::Exhausted)
    }

    pub fn is_gate_blocking_failure(&self) -> bool {
        self.gates_state_entry() && self.blocks_state_entry()
    }

    pub fn counts_toward_progress(&self) -> bool {
        self.state.status.is_state_safe() || self.is_settled()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencySnapshot {
    pub resource: &'static str,
    pub resource_ordinal: u64,
    pub field: &'static str,
    pub index: usize,
    pub role: DependencyRole,
    pub status: DependencyStatus,
    pub attempt: u8,
    pub path: Option<String>,
    pub failure: Option<DependencyFailure>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct DependencyOrderKey {
    resource_ordinal: u64,
    index: usize,
    field: &'static str,
}

#[derive(Resource, Default)]
pub struct DependencyRegistry {
    dependencies: HashMap<DependencyId, RuntimeDependency>,
    by_handle: HashMap<UntypedAssetId, Vec<DependencyId>>,
    ordered: BTreeMap<DependencyOrderKey, DependencyId>,
    pending: BTreeSet<DependencyOrderKey>,
    retry_deadlines: BTreeMap<Duration, BTreeSet<DependencyOrderKey>>,
}

impl DependencyRegistry {
    fn key(dependency: &RuntimeDependency) -> DependencyOrderKey {
        DependencyOrderKey {
            resource_ordinal: dependency.resource_ordinal,
            index: dependency.id.index,
            field: dependency.id.field,
        }
    }

    pub fn insert(&mut self, dependency: RuntimeDependency) {
        self.remove(dependency.id);
        let key = Self::key(&dependency);
        self.by_handle
            .entry(dependency.handle.id())
            .or_default()
            .push(dependency.id);
        self.ordered.insert(key, dependency.id);
        if !dependency.immediate && !dependency.state.status.is_ready() {
            self.pending.insert(key);
        }
        self.dependencies.insert(dependency.id, dependency);
    }

    pub fn get(&self, id: DependencyId) -> Option<&RuntimeDependency> {
        self.dependencies.get(&id)
    }

    pub fn get_mut(&mut self, id: DependencyId) -> Option<&mut RuntimeDependency> {
        self.dependencies.get_mut(&id)
    }

    pub fn ids(&self) -> Vec<DependencyId> {
        self.ordered.values().copied().collect()
    }

    pub fn pending_ids(&self) -> Vec<DependencyId> {
        self.pending
            .iter()
            .filter_map(|key| self.ordered.get(key).copied())
            .collect()
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn ids_for_handle(&self, handle_id: UntypedAssetId) -> &[DependencyId] {
        static EMPTY: [DependencyId; 0] = [];
        self.by_handle
            .get(&handle_id)
            .map(Vec::as_slice)
            .unwrap_or(&EMPTY)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RuntimeDependency> {
        self.ids().into_iter().filter_map(|id| self.get(id))
    }

    pub fn snapshots(&self) -> Vec<DependencySnapshot> {
        self.iter()
            .map(|dependency| DependencySnapshot {
                resource: dependency.id.resource.name,
                resource_ordinal: dependency.resource_ordinal,
                field: dependency.id.field,
                index: dependency.id.index,
                role: dependency.role,
                status: dependency.state.status.clone(),
                attempt: match dependency.state.acquisition {
                    AcquisitionState::Retrying { attempt } => attempt,
                    AcquisitionState::WaitingRetry { next_attempt } => next_attempt,
                    _ => 0,
                },
                path: dependency.resolved_path.clone(),
                failure: dependency.state.last_failure.clone(),
                error: dependency.load_error.as_ref().map(ToString::to_string),
            })
            .collect()
    }

    pub(crate) fn mark_pending(&mut self, id: DependencyId) {
        if let Some(dependency) = self.get(id) {
            self.pending.insert(Self::key(dependency));
        }
    }

    pub(crate) fn stop_polling(&mut self, id: DependencyId) {
        if let Some(dependency) = self.get(id) {
            self.pending.remove(&Self::key(dependency));
        }
    }

    pub(crate) fn schedule_retry(&mut self, id: DependencyId, deadline: Duration) {
        let Some(dependency) = self.get(id) else {
            return;
        };
        let key = Self::key(dependency);
        self.pending.remove(&key);
        self.retry_deadlines
            .entry(deadline)
            .or_default()
            .insert(key);
    }

    pub(crate) fn due_retry_ids(&mut self, now: Duration) -> Vec<DependencyId> {
        let due = self
            .retry_deadlines
            .range(..=now)
            .map(|(deadline, _)| *deadline)
            .collect::<Vec<_>>();
        let mut ids = Vec::new();
        for deadline in due {
            if let Some(keys) = self.retry_deadlines.remove(&deadline) {
                ids.extend(
                    keys.into_iter()
                        .filter_map(|key| self.ordered.get(&key).copied()),
                );
            }
        }
        ids
    }

    pub fn update_handle(&mut self, id: DependencyId, new_handle: UntypedHandle) {
        let Some(old_id) = self
            .dependencies
            .get(&id)
            .map(|dependency| dependency.handle.id())
        else {
            return;
        };
        let new_id = new_handle.id();
        if old_id != new_id {
            if let Some(ids) = self.by_handle.get_mut(&old_id) {
                ids.retain(|existing| *existing != id);
                if ids.is_empty() {
                    self.by_handle.remove(&old_id);
                }
            }
            self.by_handle.entry(new_id).or_default().push(id);
        }
        if let Some(dependency) = self.dependencies.get_mut(&id) {
            dependency.handle = new_handle;
            dependency.load_error = None;
        }
    }

    pub fn remove(&mut self, id: DependencyId) -> Option<RuntimeDependency> {
        let removed = self.dependencies.remove(&id)?;
        let key = Self::key(&removed);
        self.ordered.remove(&key);
        self.pending.remove(&key);
        for ids in self.retry_deadlines.values_mut() {
            ids.remove(&key);
        }
        self.retry_deadlines.retain(|_, ids| !ids.is_empty());
        let handle = removed.handle.id();
        if let Some(ids) = self.by_handle.get_mut(&handle) {
            ids.retain(|existing| *existing != id);
            if ids.is_empty() {
                self.by_handle.remove(&handle);
            }
        }
        Some(removed)
    }

    pub fn set_failure_details(
        &mut self,
        id: DependencyId,
        path: AssetPath<'static>,
        error: AssetLoadError,
    ) {
        if let Some(dependency) = self.dependencies.get_mut(&id) {
            dependency.resolved_path = Some(path.to_string());
            dependency.load_error = Some(Arc::new(error));
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyTransition {
    pub sequence: u64,
    pub at: Duration,
    pub dependency: DependencyId,
    pub resource_ordinal: u64,
    pub from: DependencyStatus,
    pub to: DependencyStatus,
    pub failure: Option<DependencyFailure>,
    pub terminal: bool,
}

#[derive(Resource)]
pub struct TransitionJournal {
    capacity: usize,
    next_sequence: u64,
    entries: VecDeque<DependencyTransition>,
}

impl Default for TransitionJournal {
    fn default() -> Self {
        Self::new(128)
    }
}

impl TransitionJournal {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_sequence: 0,
            entries: VecDeque::with_capacity(capacity.min(128)),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = &DependencyTransition> {
        self.entries.iter()
    }

    pub(crate) fn push(&mut self, mut transition: DependencyTransition) {
        if self.capacity == 0 {
            return;
        }
        transition.sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(transition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(acquisition: AcquisitionState) -> DependencyState {
        DependencyState {
            status: DependencyStatus::Loading,
            acquisition,
            retry_after: None,
            started_at: Duration::ZERO,
            last_failure: None,
        }
    }

    #[test]
    fn reducer_schedules_then_starts_retry() {
        let policy = FailurePolicy::Retry {
            attempts: 2,
            cooldown: Duration::from_secs(1),
        };
        let first = reduce_dependency(
            &state(AcquisitionState::Primary),
            &policy,
            DependencyCommand::LoadFailed {
                failure: DependencyFailure::LoadFailed,
                now: Duration::from_secs(2),
            },
        );
        assert_eq!(first.state.retry_after, Some(Duration::from_secs(3)));
        assert!(matches!(
            first.state.acquisition,
            AcquisitionState::WaitingRetry { next_attempt: 1 }
        ));

        let second = reduce_dependency(
            &first.state,
            &policy,
            DependencyCommand::RetryDeadlineReached {
                now: Duration::from_secs(3),
            },
        );
        assert!(second
            .effects
            .contains(&DependencyEffect::AcquirePrimary { attempt: 1 }));
    }

    #[test]
    fn fallback_failure_is_terminal() {
        let result = reduce_dependency(
            &state(AcquisitionState::Fallback),
            &FailurePolicy::Fallback {
                path: "fallback.png",
            },
            DependencyCommand::LoadFailed {
                failure: DependencyFailure::TimedOut,
                now: Duration::from_secs(1),
            },
        );
        assert_eq!(
            result.state.status,
            DependencyStatus::Failed(DependencyFailure::FallbackFailed)
        );
        assert!(result.effects.contains(&DependencyEffect::StopPolling));
    }

    #[test]
    fn journal_is_bounded() {
        let id = DependencyId {
            resource: PreparedResourceId::of::<u32>(),
            index: 0,
            field: "field",
        };
        let mut journal = TransitionJournal::new(2);
        for _ in 0..3 {
            journal.push(DependencyTransition {
                sequence: 0,
                at: Duration::ZERO,
                dependency: id,
                resource_ordinal: 0,
                from: DependencyStatus::Loading,
                to: DependencyStatus::Ready,
                failure: None,
                terminal: false,
            });
        }
        assert_eq!(journal.entries().len(), 2);
        assert_eq!(journal.entries().next().unwrap().sequence, 1);
    }

    #[test]
    fn reducer_handles_observation_retry_reload_fallback_and_release_commands() {
        let ready = reduce_dependency(
            &state(AcquisitionState::Primary),
            &FailurePolicy::Fatal,
            DependencyCommand::LoadObserved(DependencyStatus::Ready),
        );
        assert_eq!(ready.state.status, DependencyStatus::Ready);
        assert!(ready.effects.contains(&DependencyEffect::StopPolling));

        let mut failed = state(AcquisitionState::Exhausted);
        failed.status = DependencyStatus::Failed(DependencyFailure::LoadFailed);
        let retry = reduce_dependency(
            &failed,
            &FailurePolicy::Fatal,
            DependencyCommand::RetryRequested {
                now: Duration::from_secs(4),
            },
        );
        assert_eq!(retry.state.status, DependencyStatus::Loading);
        assert!(retry
            .effects
            .contains(&DependencyEffect::AcquirePrimary { attempt: 0 }));

        let fallback = reduce_dependency(
            &failed,
            &FailurePolicy::Fallback { path: "safe.data" },
            DependencyCommand::FallbackStarted {
                now: Duration::from_secs(5),
            },
        );
        assert_eq!(fallback.state.acquisition, AcquisitionState::Fallback);

        let reloaded = reduce_dependency(
            &ready.state,
            &FailurePolicy::Fatal,
            DependencyCommand::Reloaded {
                now: Duration::from_secs(6),
            },
        );
        assert_eq!(reloaded.state.status, DependencyStatus::Loading);

        let released = reduce_dependency(
            &reloaded.state,
            &FailurePolicy::Fatal,
            DependencyCommand::Released,
        );
        assert_eq!(released.state.status, DependencyStatus::NotStarted);
        assert!(released.effects.contains(&DependencyEffect::StopPolling));
    }
}
