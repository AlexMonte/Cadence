//! State-aware asset preparation and loading orchestration for Bevy.
//!
//! Add [`LoadUpPlugin`] after Bevy's `DefaultPlugins` or `StatesPlugin`,
//! register resources with [`state::LoadUpStateAppExt::require_resource_for_state`], and state transitions
//! will route through your loading state until blocking dependencies are ready.

use std::hash::Hash;
use std::marker::PhantomData;

use bevy_app::{App, Plugin, PreUpdate, Update};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_state::state::{StateTransition, StateTransitionSystems};

extern crate self as load_up;

use crate::{
    asset_events::process_untyped_asset_load_failures,
    dependency::{DependencyRegistry, TransitionJournal},
    prepared::{insert_state_safe_resources, poll_dependencies, PreparedResources},
    state::{
        finish_active_state_load, intercept_state_assets, LoadRequestCounter, LoadUpConfig,
        LoadingState, LoadingStateFor, PackRegistry,
    },
};

pub mod asset_events;
pub mod dependency;
pub mod error;
pub mod prelude;
pub mod prepared;
pub mod state;
pub mod triggers;

#[cfg(feature = "progress")]
pub mod progress;

pub use load_up_macros::LoadAssetResource;

#[cfg(feature = "progress")]
pub use progress::{calculate_load_up_progress, LoadUpProgressPlugin};

pub use error::{LoadUpOperationError, LoadUpSetupError};

/// Orchestrates asset preparation and state-gated resource insertion for a game state type.
///
/// Requires [`StatesPlugin`](bevy_state::app::StatesPlugin) or `DefaultPlugins` to be added
/// before this plugin so the `StateTransition` schedule exists.
pub struct LoadUpPlugin<S> {
    marker: PhantomData<S>,
    config: LoadUpConfig,
}

impl<S> Default for LoadUpPlugin<S> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
            config: LoadUpConfig::default(),
        }
    }
}

impl<S> LoadUpPlugin<S> {
    pub fn with_cancellation_policy(mut self, policy: state::CancellationPolicy) -> Self {
        self.config.cancellation_policy = policy;
        self
    }

    pub fn with_journal_capacity(mut self, capacity: usize) -> Self {
        self.config.journal_capacity = capacity;
        self
    }
}

impl<S> Plugin for LoadUpPlugin<S>
where
    S: States + LoadingStateFor + Clone + Eq + Hash,
{
    fn build(&self, app: &mut App) {
        if app.get_schedule_mut(StateTransition).is_none() {
            panic!("{}", LoadUpSetupError::MissingStateTransitionSchedule);
        }

        app.init_resource::<PreparedResources>();
        app.init_resource::<DependencyRegistry>();
        app.init_resource::<PackRegistry<S>>();
        app.init_resource::<LoadRequestCounter>();
        app.insert_resource(self.config);
        app.insert_resource(TransitionJournal::new(self.config.journal_capacity));
        app.insert_resource(LoadingState::<S> {
            state: S::loading_state(),
        });

        app.add_systems(
            PreUpdate,
            (
                process_untyped_asset_load_failures,
                poll_dependencies,
                insert_state_safe_resources,
            )
                .chain(),
        );

        app.add_systems(
            StateTransition,
            intercept_state_assets::<S>.before(StateTransitionSystems::DependentTransitions),
        );

        app.add_systems(
            Update,
            finish_active_state_load::<S>.run_if(in_state(S::loading_state())),
        );
    }
}
