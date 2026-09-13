use std::{any::TypeId, collections::HashMap, marker::PhantomData, sync::Arc, time::Duration};

use bevy_app::App;
use bevy_asset::{
    Asset, AssetApp, AssetLoadError, AssetPath, AssetServer, Assets, LoadState,
    RecursiveDependencyLoadState, UntypedAssetLoadFailedEvent, UntypedHandle,
};
use bevy_ecs::{event::Event, prelude::*, system::Command};
use bevy_state::prelude::States;
use bevy_time::Time;

use crate::{
    dependency::{
        reduce_dependency, AcquisitionState, DependencyCommand, DependencyEffect,
        DependencyFailure, DependencyId, DependencyRegistry, DependencyRole, DependencySnapshot,
        DependencyState, DependencyStatus, DependencyTransition, FailurePolicy, LoadUpDependency,
        RuntimeDependency, StateSafeThreshold, TransitionJournal,
    },
    error::LoadUpOperationError,
    state::ActiveStateLoad,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PreparedResourceId {
    pub type_id: TypeId,
    pub name: &'static str,
}

impl PreparedResourceId {
    pub fn of<T: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            name: std::any::type_name::<T>(),
        }
    }
}

pub trait LoadAssetResource: Resource + Asset + Clone + FromWorld + 'static {
    const NAME: &'static str;

    fn id() -> PreparedResourceId {
        PreparedResourceId::of::<Self>()
    }

    fn visit_load_up_dependencies(&self, f: &mut dyn FnMut(LoadUpDependency));
    fn register_dependency_assets(_app: &mut App) {}
}

pub type InsertLoadedResource = fn(&mut World, &UntypedHandle);
pub type RemoveLoadedResource = fn(&mut World);
pub type ReleasePreparedAsset = fn(&mut World, &UntypedHandle);

#[derive(Event, Clone, Debug, Eq, PartialEq)]
pub struct DependencyStatusChanged {
    pub dependency: DependencyId,
    pub resource: PreparedResourceId,
    pub field: &'static str,
    pub from: DependencyStatus,
    pub to: DependencyStatus,
}

#[derive(Event, Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyStateSafe {
    pub dependency: DependencyId,
    pub resource: PreparedResourceId,
    pub field: &'static str,
}

#[derive(Event, Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyReady {
    pub dependency: DependencyId,
    pub resource: PreparedResourceId,
    pub field: &'static str,
}

#[derive(Event, Clone, Debug)]
pub struct DependencyFailed {
    pub dependency: DependencyId,
    pub resource: PreparedResourceId,
    pub field: &'static str,
    pub role: DependencyRole,
    pub failure: DependencyFailure,
    pub terminal: bool,
    pub path: Option<AssetPath<'static>>,
    pub load_error: Option<Arc<AssetLoadError>>,
}

#[derive(Event, Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencySettled {
    pub dependency: DependencyId,
    pub resource: PreparedResourceId,
    pub field: &'static str,
    pub role: DependencyRole,
}

#[derive(Event, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedResourceStateSafe {
    pub resource: PreparedResourceId,
}

#[derive(Event, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedResourceReady {
    pub resource: PreparedResourceId,
}

#[derive(Event, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedResourceBlocked {
    pub resource: PreparedResourceId,
    pub dependency: DependencyId,
}

#[derive(Event, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedResourceSettled {
    pub resource: PreparedResourceId,
}

#[derive(Clone)]
pub struct PreparedResourceEntry {
    pub id: PreparedResourceId,
    pub ordinal: u64,
    pub handle: UntypedHandle,
    pub dependencies: Vec<DependencyId>,
    pub inserted_as_resource: bool,
    pub insert_enabled: bool,
    pub insert_fn: InsertLoadedResource,
    pub remove_fn: RemoveLoadedResource,
    pub release_asset_fn: ReleasePreparedAsset,
    pub state_safe_reported: bool,
    pub ready_reported: bool,
    pub blocked_reported: Option<DependencyId>,
    pub settled_reported: bool,
}

impl PreparedResourceEntry {
    pub fn is_state_safe(&self, dependencies: &DependencyRegistry) -> bool {
        self.dependencies.iter().all(|id| {
            dependencies
                .get(*id)
                .is_some_and(|dependency| match dependency.role {
                    DependencyRole::Blocking => dependency.status().is_state_safe(),
                    DependencyRole::Streaming => true,
                })
        })
    }

    pub fn is_ready(&self, dependencies: &DependencyRegistry) -> bool {
        self.dependencies.iter().all(|id| {
            dependencies
                .get(*id)
                .is_some_and(|dependency| dependency.status().is_ready())
        })
    }

    pub fn is_settled(&self, dependencies: &DependencyRegistry) -> bool {
        self.dependencies.iter().all(|id| {
            dependencies
                .get(*id)
                .is_some_and(RuntimeDependency::is_settled)
        })
    }

    pub fn blocked_by_failure(&self, dependencies: &DependencyRegistry) -> Option<DependencyId> {
        self.dependencies.iter().copied().find(|id| {
            dependencies
                .get(*id)
                .is_some_and(RuntimeDependency::is_gate_blocking_failure)
        })
    }
}

#[derive(Resource, Default)]
pub struct PreparedResources {
    entries: HashMap<PreparedResourceId, PreparedResourceEntry>,
    ordinals: HashMap<PreparedResourceId, u64>,
    next_ordinal: u64,
}

impl PreparedResources {
    pub fn register(&mut self, id: PreparedResourceId) -> u64 {
        if let Some(ordinal) = self.ordinals.get(&id) {
            return *ordinal;
        }
        let ordinal = self.next_ordinal;
        self.next_ordinal = self.next_ordinal.wrapping_add(1);
        self.ordinals.insert(id, ordinal);
        ordinal
    }

    pub fn ordinal(&self, id: PreparedResourceId) -> Option<u64> {
        self.ordinals.get(&id).copied()
    }

    pub fn insert(&mut self, entry: PreparedResourceEntry) {
        self.ordinals.entry(entry.id).or_insert(entry.ordinal);
        self.entries.insert(entry.id, entry);
    }

    pub fn get(&self, id: PreparedResourceId) -> Option<&PreparedResourceEntry> {
        self.entries.get(&id)
    }

    pub fn get_mut(&mut self, id: PreparedResourceId) -> Option<&mut PreparedResourceEntry> {
        self.entries.get_mut(&id)
    }

    pub fn is_started(&self, id: PreparedResourceId) -> bool {
        self.entries.contains_key(&id)
    }

    pub fn ids(&self) -> Vec<PreparedResourceId> {
        let mut ids = self.entries.keys().copied().collect::<Vec<_>>();
        ids.sort_by_key(|id| (self.ordinal(*id).unwrap_or(u64::MAX), id.name));
        ids
    }

    pub fn iter(&self) -> impl Iterator<Item = &PreparedResourceEntry> {
        self.ids().into_iter().filter_map(|id| self.get(id))
    }

    pub fn remove(&mut self, id: PreparedResourceId) -> Option<PreparedResourceEntry> {
        self.entries.remove(&id)
    }
}

pub trait PrepareResourceAppExt {
    fn register_asset_resource<T>(&mut self) -> &mut Self
    where
        T: LoadAssetResource;

    fn prepare_resource_now<T>(&mut self) -> &mut Self
    where
        T: LoadAssetResource;
}

impl PrepareResourceAppExt for App {
    fn register_asset_resource<T>(&mut self) -> &mut Self
    where
        T: LoadAssetResource,
    {
        self.init_resource::<PreparedResources>();
        self.init_resource::<DependencyRegistry>();
        self.init_resource::<TransitionJournal>();
        self.init_asset::<T>();
        T::register_dependency_assets(self);
        self.world_mut()
            .resource_mut::<PreparedResources>()
            .register(T::id());
        self
    }

    fn prepare_resource_now<T>(&mut self) -> &mut Self
    where
        T: LoadAssetResource,
    {
        self.register_asset_resource::<T>();
        prepare_resource_world::<T>(self.world_mut());
        self
    }
}

pub fn prepare_resource_world<T>(world: &mut World)
where
    T: LoadAssetResource,
{
    ensure_runtime_resources(world);
    let resource_id = T::id();
    if world
        .resource::<PreparedResources>()
        .is_started(resource_id)
    {
        if let Some(entry) = world
            .resource_mut::<PreparedResources>()
            .get_mut(resource_id)
        {
            entry.insert_enabled = true;
        }
        return;
    }

    let ordinal = world
        .resource_mut::<PreparedResources>()
        .register(resource_id);
    let now = world.resource::<Time>().elapsed();
    let value = T::from_world(world);
    let resource_handle = world.resource_mut::<Assets<T>>().add(value.clone());
    let prepared_handle = resource_handle.clone().untyped();

    let mut metadata = Vec::new();
    value.visit_load_up_dependencies(&mut |dependency| metadata.push(dependency));
    for dependency in &metadata {
        validate_dependency::<T>(dependency);
    }

    let mut dependency_ids = Vec::with_capacity(metadata.len());
    for dependency in metadata {
        let dependency_id = DependencyId {
            resource: resource_id,
            index: dependency_ids.len(),
            field: dependency.field,
        };
        let status = if dependency.immediate {
            DependencyStatus::Ready
        } else {
            DependencyStatus::Loading
        };
        world
            .resource_mut::<DependencyRegistry>()
            .insert(RuntimeDependency {
                id: dependency_id,
                resource_ordinal: ordinal,
                handle: dependency.handle,
                role: dependency.role,
                state: DependencyState {
                    status,
                    acquisition: AcquisitionState::Primary,
                    retry_after: None,
                    started_at: now,
                    last_failure: None,
                },
                policy: dependency.policy,
                state_safe_threshold: dependency.state_safe_threshold,
                acquire: dependency.acquire,
                acquire_fallback: dependency.acquire_fallback,
                replace: dependency.replace,
                prepared_handle: prepared_handle.clone(),
                primary_path: dependency.primary_path,
                resolved_path: dependency.primary_path.map(str::to_owned),
                timeout: dependency.timeout,
                load_error: None,
                immediate: dependency.immediate,
            });
        dependency_ids.push(dependency_id);
    }

    world
        .resource_mut::<PreparedResources>()
        .insert(PreparedResourceEntry {
            id: resource_id,
            ordinal,
            handle: resource_handle.untyped(),
            dependencies: dependency_ids,
            inserted_as_resource: false,
            insert_enabled: true,
            insert_fn: insert_prepared_resource::<T>,
            remove_fn: remove_prepared_resource_value::<T>,
            release_asset_fn: release_prepared_asset::<T>,
            state_safe_reported: false,
            ready_reported: false,
            blocked_reported: None,
            settled_reported: false,
        });
}

fn validate_dependency<T: LoadAssetResource>(dependency: &LoadUpDependency) {
    match &dependency.policy {
        FailurePolicy::Retry { attempts, .. } if *attempts == 0 => {
            panic!("{}::{} has zero retry attempts", T::NAME, dependency.field)
        }
        FailurePolicy::Fallback { path: "" } => {
            panic!(
                "{}::{} has an empty fallback path",
                T::NAME,
                dependency.field
            )
        }
        _ => {}
    }
    if dependency.timeout.is_some_and(|timeout| timeout.is_zero()) {
        panic!("{}::{} has a zero timeout", T::NAME, dependency.field);
    }
}

fn insert_prepared_resource<T: LoadAssetResource>(world: &mut World, handle: &UntypedHandle) {
    let value = world
        .resource::<Assets<T>>()
        .get(handle.id().typed::<T>())
        .cloned();
    if let Some(value) = value {
        world.insert_resource(value);
    }
}

pub(crate) fn remove_prepared_resource_value<T: LoadAssetResource>(world: &mut World) {
    world.remove_resource::<T>();
}

fn release_prepared_asset<T: LoadAssetResource>(world: &mut World, handle: &UntypedHandle) {
    world
        .resource_mut::<Assets<T>>()
        .remove(handle.id().typed::<T>());
}

pub fn poll_dependencies(world: &mut World) {
    ensure_runtime_resources(world);
    let now = world.resource::<Time>().elapsed();
    for id in world
        .resource_mut::<DependencyRegistry>()
        .due_retry_ids(now)
    {
        dispatch_dependency_command(world, id, DependencyCommand::RetryDeadlineReached { now });
    }

    let ids = world.resource::<DependencyRegistry>().pending_ids();
    for id in ids {
        let dependency = match world.resource::<DependencyRegistry>().get(id).cloned() {
            Some(dependency) => dependency,
            None => continue,
        };

        if dependency
            .timeout
            .is_some_and(|timeout| now.saturating_sub(dependency.state.started_at) >= timeout)
        {
            dispatch_dependency_command(
                world,
                id,
                DependencyCommand::LoadFailed {
                    failure: DependencyFailure::TimedOut,
                    now,
                },
            );
            continue;
        }

        let asset_server = world.resource::<AssetServer>();
        let load_state = asset_server.load_state(dependency.handle.id());
        let recursive = asset_server.recursive_dependency_load_state(dependency.handle.id());
        let command = match load_state {
            LoadState::Loaded => match recursive {
                RecursiveDependencyLoadState::Loaded => {
                    DependencyCommand::LoadObserved(DependencyStatus::Ready)
                }
                RecursiveDependencyLoadState::Failed(error) => {
                    if let Some(current) = world.resource_mut::<DependencyRegistry>().get_mut(id) {
                        current.load_error = Some(error);
                    }
                    DependencyCommand::LoadFailed {
                        failure: DependencyFailure::LoadFailed,
                        now,
                    }
                }
                RecursiveDependencyLoadState::Loading | RecursiveDependencyLoadState::NotLoaded => {
                    DependencyCommand::LoadObserved(match dependency.state_safe_threshold {
                        StateSafeThreshold::Ready => DependencyStatus::Loading,
                        StateSafeThreshold::Loaded => DependencyStatus::StateSafe,
                    })
                }
            },
            LoadState::Loading => DependencyCommand::LoadObserved(DependencyStatus::Loading),
            LoadState::NotLoaded if dependency.immediate => {
                DependencyCommand::LoadObserved(DependencyStatus::Ready)
            }
            LoadState::NotLoaded => DependencyCommand::LoadObserved(DependencyStatus::NotStarted),
            LoadState::Failed(error) => {
                if let Some(current) = world.resource_mut::<DependencyRegistry>().get_mut(id) {
                    current.load_error = Some(error);
                }
                DependencyCommand::LoadFailed {
                    failure: DependencyFailure::LoadFailed,
                    now,
                }
            }
        };
        dispatch_dependency_command(world, id, command);
    }

    insert_state_safe_resources(world);
}

pub(crate) fn apply_asset_load_failures(
    world: &mut World,
    failures: &[UntypedAssetLoadFailedEvent],
) {
    ensure_runtime_resources(world);
    let now = world.resource::<Time>().elapsed();
    let mut details = HashMap::new();
    for failure in failures {
        let ids = world
            .resource::<DependencyRegistry>()
            .ids_for_handle(failure.id)
            .to_vec();
        for id in ids {
            details.insert(id, (failure.path.clone(), failure.error.clone()));
        }
    }
    let ordered = world.resource::<DependencyRegistry>().ids();
    for id in ordered {
        let Some((path, error)) = details.remove(&id) else {
            continue;
        };
        world
            .resource_mut::<DependencyRegistry>()
            .set_failure_details(id, path, error);
        dispatch_dependency_command(
            world,
            id,
            DependencyCommand::LoadFailed {
                failure: DependencyFailure::LoadFailed,
                now,
            },
        );
    }
}

fn dispatch_dependency_command(world: &mut World, id: DependencyId, command: DependencyCommand) {
    let Some(dependency) = world.resource::<DependencyRegistry>().get(id).cloned() else {
        return;
    };
    let previous = dependency.state.status.clone();
    let now = world.resource::<Time>().elapsed();
    let reduction = reduce_dependency(&dependency.state, &dependency.policy, command);
    if let Some(current) = world.resource_mut::<DependencyRegistry>().get_mut(id) {
        current.state = reduction.state.clone();
    }

    for effect in reduction.effects {
        match effect {
            DependencyEffect::AcquirePrimary { .. } => {
                let new_handle = (dependency.acquire)(world);
                (dependency.replace)(world, &dependency.prepared_handle, new_handle.clone());
                let registry = &mut *world.resource_mut::<DependencyRegistry>();
                registry.update_handle(id, new_handle);
                if let Some(current) = registry.get_mut(id) {
                    current.resolved_path = current.primary_path.map(str::to_owned);
                    current.load_error = None;
                }
            }
            DependencyEffect::AcquireFallback { path } => {
                let new_handle = (dependency.acquire_fallback)(world, path);
                (dependency.replace)(world, &dependency.prepared_handle, new_handle.clone());
                let registry = &mut *world.resource_mut::<DependencyRegistry>();
                registry.update_handle(id, new_handle);
                if let Some(current) = registry.get_mut(id) {
                    current.resolved_path = Some(path.to_owned());
                    current.load_error = None;
                }
            }
            DependencyEffect::ScheduleRetry { deadline } => {
                world
                    .resource_mut::<DependencyRegistry>()
                    .schedule_retry(id, deadline);
            }
            DependencyEffect::Poll => {
                world.resource_mut::<DependencyRegistry>().mark_pending(id);
            }
            DependencyEffect::StopPolling => {
                world.resource_mut::<DependencyRegistry>().stop_polling(id);
            }
            DependencyEffect::ReportFailure { failure, terminal } => {
                report_failure(world, id, failure, terminal, now, previous.clone());
            }
        }
    }

    let current = world
        .resource::<DependencyRegistry>()
        .get(id)
        .map(|dependency| dependency.state.status.clone());
    if let Some(current) = current.filter(|current| *current != previous) {
        record_transition(
            world,
            id,
            previous.clone(),
            current.clone(),
            None,
            false,
            now,
        );
        emit_status_change(world, id, previous, current);
    }
}

fn report_failure(
    world: &mut World,
    id: DependencyId,
    failure: DependencyFailure,
    terminal: bool,
    now: Duration,
    from: DependencyStatus,
) {
    let dependency = world
        .resource::<DependencyRegistry>()
        .get(id)
        .cloned()
        .expect("dependency exists while reporting failure");
    let path = dependency.resolved_path.clone().map(AssetPath::from);
    world.trigger(DependencyFailed {
        dependency: id,
        resource: id.resource,
        field: id.field,
        role: dependency.role,
        failure: failure.clone(),
        terminal,
        path,
        load_error: dependency.load_error.clone(),
    });
    record_transition(
        world,
        id,
        from,
        dependency.state.status,
        Some(failure),
        terminal,
        now,
    );
}

fn record_transition(
    world: &mut World,
    id: DependencyId,
    from: DependencyStatus,
    to: DependencyStatus,
    failure: Option<DependencyFailure>,
    terminal: bool,
    at: Duration,
) {
    let ordinal = world
        .resource::<DependencyRegistry>()
        .get(id)
        .map(|dependency| dependency.resource_ordinal)
        .unwrap_or_default();
    world
        .resource_mut::<TransitionJournal>()
        .push(DependencyTransition {
            sequence: 0,
            at,
            dependency: id,
            resource_ordinal: ordinal,
            from,
            to,
            failure,
            terminal,
        });
}

fn emit_status_change(
    world: &mut World,
    id: DependencyId,
    from: DependencyStatus,
    to: DependencyStatus,
) {
    world.trigger(DependencyStatusChanged {
        dependency: id,
        resource: id.resource,
        field: id.field,
        from: from.clone(),
        to: to.clone(),
    });
    if !from.is_state_safe() && to.is_state_safe() {
        world.trigger(DependencyStateSafe {
            dependency: id,
            resource: id.resource,
            field: id.field,
        });
    }
    if !from.is_ready() && to.is_ready() {
        world.trigger(DependencyReady {
            dependency: id,
            resource: id.resource,
            field: id.field,
        });
    }
    let dependency = world.resource::<DependencyRegistry>().get(id).cloned();
    if dependency.is_some_and(|dependency| dependency.is_settled()) {
        let role = world.resource::<DependencyRegistry>().get(id).unwrap().role;
        world.trigger(DependencySettled {
            dependency: id,
            resource: id.resource,
            field: id.field,
            role,
        });
    }
}

pub fn insert_state_safe_resources(world: &mut World) {
    let ready = {
        let resources = world.resource::<PreparedResources>();
        let dependencies = world.resource::<DependencyRegistry>();
        resources
            .iter()
            .filter(|entry| {
                entry.insert_enabled
                    && !entry.inserted_as_resource
                    && entry.is_state_safe(dependencies)
            })
            .map(|entry| (entry.id, entry.handle.clone(), entry.insert_fn))
            .collect::<Vec<_>>()
    };
    for (id, handle, insert) in ready {
        insert(world, &handle);
        if let Some(entry) = world.resource_mut::<PreparedResources>().get_mut(id) {
            entry.inserted_as_resource = true;
        }
    }
    emit_resource_events(world);
}

pub fn retry_dependency(world: &mut World, id: DependencyId) -> Result<bool, LoadUpOperationError> {
    ensure_runtime_resources(world);
    let dependency = world
        .resource::<DependencyRegistry>()
        .get(id)
        .ok_or(LoadUpOperationError::DependencyNotFound { field: id.field })?;
    if !dependency.status().is_failed() {
        return Ok(false);
    }
    let now = world.resource::<Time>().elapsed();
    dispatch_dependency_command(world, id, DependencyCommand::RetryRequested { now });
    reset_resource_reports(world, id.resource);
    Ok(true)
}

pub fn retry_prepared_resource<T: LoadAssetResource>(
    world: &mut World,
) -> Result<usize, LoadUpOperationError> {
    ensure_runtime_resources(world);
    let id = T::id();
    let dependencies = world
        .resource::<PreparedResources>()
        .get(id)
        .ok_or(LoadUpOperationError::PreparedResourceNotFound { resource: T::NAME })?
        .dependencies
        .clone();
    let mut restarted = 0;
    for dependency in dependencies {
        restarted += usize::from(retry_dependency(world, dependency)?);
    }
    if restarted > 0 {
        reset_resource_reports(world, id);
    }
    Ok(restarted)
}

pub fn reset_prepared_resource<T: LoadAssetResource>(
    world: &mut World,
) -> Result<(), LoadUpOperationError> {
    if !world
        .get_resource::<PreparedResources>()
        .is_some_and(|resources| resources.is_started(T::id()))
    {
        return Err(LoadUpOperationError::PreparedResourceNotFound { resource: T::NAME });
    }
    release_prepared_resource::<T>(world);
    prepare_resource_world::<T>(world);
    Ok(())
}

pub fn remove_prepared_resource_by_id(world: &mut World, id: PreparedResourceId) -> bool {
    let remove = world
        .get_resource::<PreparedResources>()
        .and_then(|resources| resources.get(id))
        .map(|entry| entry.remove_fn);
    if let Some(remove) = remove {
        remove(world);
        if let Some(entry) = world.resource_mut::<PreparedResources>().get_mut(id) {
            entry.inserted_as_resource = false;
            entry.insert_enabled = false;
        }
        true
    } else {
        false
    }
}

pub fn release_prepared_resource_by_id(world: &mut World, id: PreparedResourceId) -> bool {
    let Some(entry) = world
        .get_resource_mut::<PreparedResources>()
        .and_then(|mut resources| resources.remove(id))
    else {
        return false;
    };
    for dependency in &entry.dependencies {
        world
            .resource_mut::<DependencyRegistry>()
            .remove(*dependency);
    }
    (entry.remove_fn)(world);
    (entry.release_asset_fn)(world, &entry.handle);
    true
}

pub fn release_prepared_resource<T: LoadAssetResource>(world: &mut World) -> bool {
    release_prepared_resource_by_id(world, T::id())
}

fn reset_resource_reports(world: &mut World, id: PreparedResourceId) {
    if let Some(entry) = world.resource_mut::<PreparedResources>().get_mut(id) {
        entry.state_safe_reported = false;
        entry.ready_reported = false;
        entry.blocked_reported = None;
        entry.settled_reported = false;
    }
}

fn emit_resource_events(world: &mut World) {
    let ids = world.resource::<PreparedResources>().ids();
    for id in ids {
        let snapshot = {
            let resources = world.resource::<PreparedResources>();
            let dependencies = world.resource::<DependencyRegistry>();
            let entry = resources.get(id).unwrap();
            (
                entry.is_state_safe(dependencies),
                entry.is_ready(dependencies),
                entry.is_settled(dependencies),
                entry.blocked_by_failure(dependencies),
                entry.state_safe_reported,
                entry.ready_reported,
                entry.blocked_reported,
                entry.settled_reported,
            )
        };
        if snapshot.0 && !snapshot.4 {
            world.trigger(PreparedResourceStateSafe { resource: id });
            world
                .resource_mut::<PreparedResources>()
                .get_mut(id)
                .unwrap()
                .state_safe_reported = true;
        }
        if snapshot.1 && !snapshot.5 {
            world.trigger(PreparedResourceReady { resource: id });
            world
                .resource_mut::<PreparedResources>()
                .get_mut(id)
                .unwrap()
                .ready_reported = true;
        }
        if snapshot.2 && !snapshot.7 {
            world.trigger(PreparedResourceSettled { resource: id });
            world
                .resource_mut::<PreparedResources>()
                .get_mut(id)
                .unwrap()
                .settled_reported = true;
        }
        if snapshot.3 != snapshot.6 {
            if let Some(dependency) = snapshot.3 {
                world.trigger(PreparedResourceBlocked {
                    resource: id,
                    dependency,
                });
            }
            world
                .resource_mut::<PreparedResources>()
                .get_mut(id)
                .unwrap()
                .blocked_reported = snapshot.3;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LoadProgress {
    pub done: u32,
    pub total: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedResourceSnapshot {
    pub id: PreparedResourceId,
    pub ordinal: u64,
    pub inserted: bool,
    pub state_safe: bool,
    pub ready: bool,
    pub settled: bool,
    pub dependencies: Vec<DependencySnapshot>,
}

#[derive(Clone, Debug)]
pub struct LoadUpSnapshot<S> {
    pub active_target: Option<S>,
    pub resources: Vec<PreparedResourceSnapshot>,
    pub blocking_failure: Option<DependencySnapshot>,
    pub progress: LoadProgress,
    pub journal: Vec<DependencyTransition>,
}

pub fn load_up_snapshot<S: States + Clone>(world: &World) -> LoadUpSnapshot<S> {
    let dependencies = world.resource::<DependencyRegistry>();
    let resources = world.resource::<PreparedResources>();
    let active = world.get_resource::<ActiveStateLoad<S>>();
    let active_ids = active
        .map(|load| {
            load.blocking_resources
                .iter()
                .chain(&load.streaming_resources)
                .copied()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut done = 0;
    let mut total = 0;
    for id in &active_ids {
        if let Some(entry) = resources.get(*id) {
            for dependency in &entry.dependencies {
                total += 1;
                done += u32::from(
                    dependencies
                        .get(*dependency)
                        .is_some_and(RuntimeDependency::counts_toward_progress),
                );
            }
        }
    }
    let snapshots = dependencies.snapshots();
    let resource_snapshots = resources
        .iter()
        .map(|entry| PreparedResourceSnapshot {
            id: entry.id,
            ordinal: entry.ordinal,
            inserted: entry.inserted_as_resource,
            state_safe: entry.is_state_safe(dependencies),
            ready: entry.is_ready(dependencies),
            settled: entry.is_settled(dependencies),
            dependencies: snapshots
                .iter()
                .filter(|dependency| dependency.resource_ordinal == entry.ordinal)
                .cloned()
                .collect(),
        })
        .collect::<Vec<_>>();
    let active_ordinals = active_ids
        .iter()
        .filter_map(|id| resources.ordinal(*id))
        .collect::<Vec<_>>();
    let blocking_failure = snapshots
        .iter()
        .find(|snapshot| {
            active_ordinals.contains(&snapshot.resource_ordinal)
                && snapshot.role == DependencyRole::Blocking
                && snapshot.status.is_failed()
        })
        .cloned();
    LoadUpSnapshot {
        active_target: active.map(|active| active.target.clone()),
        resources: resource_snapshots,
        blocking_failure,
        progress: LoadProgress { done, total },
        journal: world
            .resource::<TransitionJournal>()
            .entries()
            .cloned()
            .collect(),
    }
}

pub fn ensure_runtime_resources(world: &mut World) {
    if !world.contains_resource::<PreparedResources>() {
        world.init_resource::<PreparedResources>();
    }
    if !world.contains_resource::<DependencyRegistry>() {
        world.init_resource::<DependencyRegistry>();
    }
    if !world.contains_resource::<TransitionJournal>() {
        world.init_resource::<TransitionJournal>();
    }
}

pub struct PrepareResourceWorldCommand<T: LoadAssetResource>(PhantomData<T>);
pub struct RetryPreparedResourceCommand<T: LoadAssetResource>(PhantomData<T>);

impl<T: LoadAssetResource> Default for PrepareResourceWorldCommand<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: LoadAssetResource> Command for PrepareResourceWorldCommand<T> {
    fn apply(self, world: &mut World) {
        prepare_resource_world::<T>(world);
    }
}

impl<T: LoadAssetResource> Default for RetryPreparedResourceCommand<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: LoadAssetResource> Command for RetryPreparedResourceCommand<T> {
    fn apply(self, world: &mut World) {
        let _ = retry_prepared_resource::<T>(world);
    }
}
