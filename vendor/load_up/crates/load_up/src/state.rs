use std::{collections::HashMap, hash::Hash};

use bevy_app::App;
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;
use bevy_state::{prelude::*, state::FreelyMutableState};

use crate::{
    dependency::{DependencyId, DependencyRegistry},
    error::{LoadUpSetupError, RegistrationRole},
    prepared::{
        prepare_resource_world, release_prepared_resource, release_prepared_resource_by_id,
        remove_prepared_resource_by_id, LoadAssetResource, PreparedResourceId, PreparedResources,
    },
};

pub trait LoadingStateFor: States + FreelyMutableState + Clone + Eq + Hash {
    fn loading_state() -> Self;
}

#[derive(Resource, Clone)]
pub struct LoadingState<S: States> {
    pub state: S,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RetentionPolicy {
    #[default]
    Keep,
    RemoveResource,
    Unload,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CancellationPolicy {
    #[default]
    KeepPrepared,
    ReleaseExclusive,
}

#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadUpConfig {
    pub cancellation_policy: CancellationPolicy,
    pub journal_capacity: usize,
}

impl Default for LoadUpConfig {
    fn default() -> Self {
        Self {
            cancellation_policy: CancellationPolicy::KeepPrepared,
            journal_capacity: 128,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RegisteredPreparedResource {
    pub id: PreparedResourceId,
    pub ordinal: u64,
    pub retention: RetentionPolicy,
    pub prepare: fn(&mut World),
    pub release: fn(&mut World) -> bool,
}

impl RegisteredPreparedResource {
    pub fn of<T: LoadAssetResource>(ordinal: u64, retention: RetentionPolicy) -> Self {
        Self {
            id: T::id(),
            ordinal,
            retention,
            prepare: prepare_resource_world::<T>,
            release: release_prepared_resource::<T>,
        }
    }
}

#[derive(Resource)]
pub struct PackRegistry<S: States> {
    blocking: HashMap<S, Vec<RegisteredPreparedResource>>,
    streaming: HashMap<S, Vec<RegisteredPreparedResource>>,
}

impl<S: States> Default for PackRegistry<S> {
    fn default() -> Self {
        Self {
            blocking: HashMap::new(),
            streaming: HashMap::new(),
        }
    }
}

impl<S: States + Clone + Eq + Hash> PackRegistry<S> {
    pub fn blocking_for(&self, state: &S) -> Vec<RegisteredPreparedResource> {
        self.blocking.get(state).cloned().unwrap_or_default()
    }

    pub fn streaming_for(&self, state: &S) -> Vec<RegisteredPreparedResource> {
        self.streaming.get(state).cloned().unwrap_or_default()
    }

    pub fn all_for(&self, state: &S) -> Vec<RegisteredPreparedResource> {
        let mut all = self.blocking_for(state);
        all.extend(self.streaming_for(state));
        all.sort_by_key(|resource| resource.ordinal);
        all
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveStateLoadReport {
    Waiting,
    Blocked {
        resource: PreparedResourceId,
        dependency: DependencyId,
    },
}

#[derive(Resource)]
pub struct ActiveStateLoad<S: States> {
    pub target: S,
    pub generation: u64,
    pub blocking_resources: Vec<PreparedResourceId>,
    pub streaming_resources: Vec<PreparedResourceId>,
    pub owned_resources: Vec<PreparedResourceId>,
    pub last_report: Option<ActiveStateLoadReport>,
}

#[derive(Resource, Default)]
#[doc(hidden)]
pub struct LoadRequestCounter(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateGateResult {
    CanEnter,
    Waiting,
    Blocked {
        resource: PreparedResourceId,
        dependency: DependencyId,
    },
}

#[derive(Event, Clone, Debug, Eq, PartialEq)]
pub struct StateLoadReady<S: States> {
    pub target: S,
    pub generation: u64,
}

#[derive(Event, Clone, Debug, Eq, PartialEq)]
pub struct StateLoadBlocked<S: States> {
    pub target: S,
    pub generation: u64,
    pub resource: PreparedResourceId,
    pub dependency: DependencyId,
}

#[derive(Event, Clone, Debug, Eq, PartialEq)]
pub struct StateLoadCancelled<S: States> {
    pub previous_target: S,
    pub replacement_target: S,
    pub generation: u64,
}

pub trait LoadUpStateAppExt<S: States> {
    fn require_resource_for_state<T>(&mut self, state: S, retention: RetentionPolicy) -> &mut Self
    where
        T: LoadAssetResource;

    fn try_require_resource_for_state<T>(
        &mut self,
        state: S,
        retention: RetentionPolicy,
    ) -> Result<&mut Self, LoadUpSetupError>
    where
        T: LoadAssetResource;

    fn stream_resource_for_state<T>(&mut self, state: S, retention: RetentionPolicy) -> &mut Self
    where
        T: LoadAssetResource;

    fn try_stream_resource_for_state<T>(
        &mut self,
        state: S,
        retention: RetentionPolicy,
    ) -> Result<&mut Self, LoadUpSetupError>
    where
        T: LoadAssetResource;
}

impl<S> LoadUpStateAppExt<S> for App
where
    S: States + FreelyMutableState + Clone + Eq + Hash,
{
    fn require_resource_for_state<T>(&mut self, state: S, retention: RetentionPolicy) -> &mut Self
    where
        T: LoadAssetResource,
    {
        self.try_require_resource_for_state::<T>(state, retention)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn try_require_resource_for_state<T>(
        &mut self,
        state: S,
        retention: RetentionPolicy,
    ) -> Result<&mut Self, LoadUpSetupError>
    where
        T: LoadAssetResource,
    {
        register_for_state::<S, T>(self, state, retention, RegistrationRole::Blocking)?;
        Ok(self)
    }

    fn stream_resource_for_state<T>(&mut self, state: S, retention: RetentionPolicy) -> &mut Self
    where
        T: LoadAssetResource,
    {
        self.try_stream_resource_for_state::<T>(state, retention)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn try_stream_resource_for_state<T>(
        &mut self,
        state: S,
        retention: RetentionPolicy,
    ) -> Result<&mut Self, LoadUpSetupError>
    where
        T: LoadAssetResource,
    {
        register_for_state::<S, T>(self, state, retention, RegistrationRole::Streaming)?;
        Ok(self)
    }
}

fn register_for_state<S, T>(
    app: &mut App,
    state: S,
    retention: RetentionPolicy,
    role: RegistrationRole,
) -> Result<(), LoadUpSetupError>
where
    S: States + Clone + Eq + Hash,
    T: LoadAssetResource,
{
    app.init_asset::<T>();
    T::register_dependency_assets(app);
    app.init_resource::<PreparedResources>();
    app.init_resource::<DependencyRegistry>();
    app.init_resource::<PackRegistry<S>>();
    let ordinal = app
        .world_mut()
        .resource_mut::<PreparedResources>()
        .register(T::id());
    let registered = RegisteredPreparedResource::of::<T>(ordinal, retention);
    let mut registry = app.world_mut().resource_mut::<PackRegistry<S>>();
    let conflicts = match role {
        RegistrationRole::Blocking => registry.streaming.get(&state),
        RegistrationRole::Streaming => registry.blocking.get(&state),
    }
    .is_some_and(|resources| resources.iter().any(|resource| resource.id == T::id()));
    if conflicts {
        return Err(LoadUpSetupError::ConflictingRegistration { resource: T::NAME });
    }
    let same = match role {
        RegistrationRole::Blocking => &mut registry.blocking,
        RegistrationRole::Streaming => &mut registry.streaming,
    };
    let resources = same.entry(state).or_default();
    if resources.iter().any(|resource| resource.id == T::id()) {
        return Err(LoadUpSetupError::DuplicateRegistration {
            resource: T::NAME,
            role,
        });
    }
    resources.push(registered);
    resources.sort_by_key(|resource| resource.ordinal);
    Ok(())
}

pub fn prepare_state_world<S>(world: &mut World, state: S)
where
    S: States + Clone + Eq + Hash,
{
    if !world.contains_resource::<PackRegistry<S>>() {
        world.init_resource::<PackRegistry<S>>();
    }
    let resources = world.resource::<PackRegistry<S>>().all_for(&state);
    for resource in resources {
        (resource.prepare)(world);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn intercept_state_assets<S>(
    mut commands: Commands,
    loading: Res<LoadingState<S>>,
    config: Res<LoadUpConfig>,
    registry: Res<PackRegistry<S>>,
    prepared: Res<PreparedResources>,
    dependencies: Res<DependencyRegistry>,
    current: Res<State<S>>,
    active: Option<Res<ActiveStateLoad<S>>>,
    mut counter: ResMut<LoadRequestCounter>,
    mut next_state: ResMut<NextState<S>>,
) where
    S: States + FreelyMutableState + Clone + Eq + Hash,
{
    let requested = match &*next_state {
        NextState::Pending(state) | NextState::PendingIfNeq(state) => state.clone(),
        NextState::Unchanged => return,
    };

    if let Some(active) = active.as_ref() {
        if active.target != requested {
            let previous_target = active.target.clone();
            let replacement_target = requested.clone();
            let generation = active.generation;
            commands.queue(move |world: &mut World| {
                world.trigger(StateLoadCancelled::<S> {
                    previous_target,
                    replacement_target,
                    generation,
                });
            });
            if config.cancellation_policy == CancellationPolicy::ReleaseExclusive {
                let replacement_ids = registry
                    .all_for(&requested)
                    .into_iter()
                    .map(|resource| resource.id)
                    .collect::<Vec<_>>();
                let exclusive = active
                    .owned_resources
                    .iter()
                    .filter(|id| !replacement_ids.contains(id))
                    .copied()
                    .collect::<Vec<_>>();
                commands.queue(move |world: &mut World| {
                    for id in exclusive {
                        release_prepared_resource_by_id(world, id);
                    }
                });
            }
            commands.remove_resource::<ActiveStateLoad<S>>();
        }
    }

    if current.get() != &loading.state && current.get() != &requested {
        let exiting = current.get().clone();
        let destination = requested.clone();
        commands.queue(move |world: &mut World| {
            apply_state_exit_policies(world, &exiting, &destination);
        });
    }

    if requested == loading.state {
        return;
    }

    let blocking = registry.blocking_for(&requested);
    let streaming = registry.streaming_for(&requested);
    if blocking.is_empty() && streaming.is_empty() {
        return;
    }
    let blocking_ids = blocking
        .iter()
        .map(|resource| resource.id)
        .collect::<Vec<_>>();
    let streaming_ids = streaming
        .iter()
        .map(|resource| resource.id)
        .collect::<Vec<_>>();
    let owned_resources = blocking
        .iter()
        .chain(&streaming)
        .filter(|resource| !prepared.is_started(resource.id))
        .map(|resource| resource.id)
        .collect::<Vec<_>>();
    for resource in blocking.iter().chain(&streaming).copied() {
        commands.queue(move |world: &mut World| (resource.prepare)(world));
    }

    if matches!(
        active.as_ref().map(|active| &active.target),
        Some(target) if target == &requested
    ) {
        next_state.set(loading.state.clone());
        return;
    }

    counter.0 = counter.0.wrapping_add(1);
    let generation = counter.0;
    let gate = state_gate_result(&requested, &registry, &prepared, &dependencies);
    match gate {
        StateGateResult::CanEnter => {}
        StateGateResult::Waiting => {
            commands.insert_resource(ActiveStateLoad {
                target: requested,
                generation,
                blocking_resources: blocking_ids,
                streaming_resources: streaming_ids,
                owned_resources,
                last_report: Some(ActiveStateLoadReport::Waiting),
            });
            next_state.set(loading.state.clone());
        }
        StateGateResult::Blocked {
            resource,
            dependency,
        } => {
            let target = requested.clone();
            commands.queue(move |world: &mut World| {
                world.trigger(StateLoadBlocked {
                    target,
                    generation,
                    resource,
                    dependency,
                });
            });
            commands.insert_resource(ActiveStateLoad {
                target: requested,
                generation,
                blocking_resources: blocking_ids,
                streaming_resources: streaming_ids,
                owned_resources,
                last_report: Some(ActiveStateLoadReport::Blocked {
                    resource,
                    dependency,
                }),
            });
            next_state.set(loading.state.clone());
        }
    }
}

fn apply_state_exit_policies<S>(world: &mut World, exiting: &S, destination: &S)
where
    S: States + Clone + Eq + Hash,
{
    let (exiting_resources, destination_ids) = {
        let registry = world.resource::<PackRegistry<S>>();
        (
            registry.all_for(exiting),
            registry
                .all_for(destination)
                .into_iter()
                .map(|resource| resource.id)
                .collect::<Vec<_>>(),
        )
    };
    for resource in exiting_resources {
        if destination_ids.contains(&resource.id) {
            continue;
        }
        match resource.retention {
            RetentionPolicy::Keep => {}
            RetentionPolicy::RemoveResource => {
                remove_prepared_resource_by_id(world, resource.id);
            }
            RetentionPolicy::Unload => {
                (resource.release)(world);
            }
        }
    }
}

pub fn finish_active_state_load<S>(
    active: Option<Res<ActiveStateLoad<S>>>,
    prepared: Res<PreparedResources>,
    registry: Res<PackRegistry<S>>,
    dependencies: Res<DependencyRegistry>,
    mut next_state: ResMut<NextState<S>>,
    mut commands: Commands,
) where
    S: States + FreelyMutableState + Clone + Eq + Hash,
{
    let Some(active) = active else { return };
    match state_gate_result(&active.target, &registry, &prepared, &dependencies) {
        StateGateResult::CanEnter => {
            let target = active.target.clone();
            let event_target = target.clone();
            let generation = active.generation;
            commands.queue(move |world: &mut World| {
                world.trigger(StateLoadReady {
                    target: event_target,
                    generation,
                });
            });
            commands.remove_resource::<ActiveStateLoad<S>>();
            next_state.set(target);
        }
        StateGateResult::Waiting => {
            if active.last_report != Some(ActiveStateLoadReport::Waiting) {
                commands.insert_resource(ActiveStateLoad {
                    target: active.target.clone(),
                    generation: active.generation,
                    blocking_resources: active.blocking_resources.clone(),
                    streaming_resources: active.streaming_resources.clone(),
                    owned_resources: active.owned_resources.clone(),
                    last_report: Some(ActiveStateLoadReport::Waiting),
                });
            }
        }
        StateGateResult::Blocked {
            resource,
            dependency,
        } => {
            let report = ActiveStateLoadReport::Blocked {
                resource,
                dependency,
            };
            if active.last_report != Some(report) {
                let target = active.target.clone();
                let generation = active.generation;
                commands.queue(move |world: &mut World| {
                    world.trigger(StateLoadBlocked {
                        target,
                        generation,
                        resource,
                        dependency,
                    });
                });
                commands.insert_resource(ActiveStateLoad {
                    target: active.target.clone(),
                    generation,
                    blocking_resources: active.blocking_resources.clone(),
                    streaming_resources: active.streaming_resources.clone(),
                    owned_resources: active.owned_resources.clone(),
                    last_report: Some(report),
                });
            }
        }
    }
}

pub fn state_gate_result<S>(
    state: &S,
    registry: &PackRegistry<S>,
    resources: &PreparedResources,
    dependencies: &DependencyRegistry,
) -> StateGateResult
where
    S: States + Clone + Eq + Hash,
{
    for registered in registry.blocking_for(state) {
        let Some(resource) = resources.get(registered.id) else {
            return StateGateResult::Waiting;
        };
        if let Some(dependency) = resource.blocked_by_failure(dependencies) {
            return StateGateResult::Blocked {
                resource: registered.id,
                dependency,
            };
        }
        if !resource.is_state_safe(dependencies) {
            return StateGateResult::Waiting;
        }
    }
    StateGateResult::CanEnter
}
