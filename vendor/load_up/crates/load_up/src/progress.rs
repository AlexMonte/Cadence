//! Optional integration with [`iyes_progress`](https://docs.rs/iyes_progress) for loading bars.

use std::hash::Hash;

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_state::state::FreelyMutableState;
use iyes_progress::prelude::*;

use crate::dependency::DependencyRegistry;
use crate::prepared::PreparedResources;
use crate::state::{ActiveStateLoad, LoadingStateFor};

/// Reports aggregate load_up dependency progress to [`iyes_progress`](https://docs.rs/iyes_progress).
///
/// Add this alongside [`ProgressPlugin`] from `iyes_progress`.
/// The loading state type must implement [`LoadingStateFor`].
pub struct LoadUpProgressPlugin<S> {
    marker: std::marker::PhantomData<S>,
}

impl<S> Default for LoadUpProgressPlugin<S> {
    fn default() -> Self {
        Self {
            marker: std::marker::PhantomData,
        }
    }
}

impl<S> Plugin for LoadUpProgressPlugin<S>
where
    S: States + LoadingStateFor + Clone + Eq + Hash + FreelyMutableState,
{
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            report_load_up_progress::<S>.run_if(in_state(S::loading_state())),
        );
    }
}

/// Computes loading progress for the active state transition.
///
/// Numerator: dependencies that are state-safe, ready, or settled (including exhausted failures).
/// Denominator: total dependencies across prepared resources for the active load.
pub fn calculate_load_up_progress<S>(
    active: Option<Res<ActiveStateLoad<S>>>,
    prepared: Res<PreparedResources>,
    dependencies: Res<DependencyRegistry>,
) -> Progress
where
    S: States,
{
    let Some(active) = active else {
        return Progress { done: 0, total: 0 };
    };

    let mut done = 0u32;
    let mut total = 0u32;

    for resource_id in active
        .blocking_resources
        .iter()
        .chain(active.streaming_resources.iter())
    {
        let Some(entry) = prepared.get(*resource_id) else {
            continue;
        };

        for dependency_id in &entry.dependencies {
            total += 1;
            if dependencies
                .get(*dependency_id)
                .is_some_and(|dependency| dependency.counts_toward_progress())
            {
                done += 1;
            }
        }
    }

    Progress { done, total }
}

fn report_load_up_progress<S>(
    active: Option<Res<ActiveStateLoad<S>>>,
    prepared: Res<PreparedResources>,
    dependencies: Res<DependencyRegistry>,
    entry: ProgressEntry<S>,
) where
    S: States + FreelyMutableState,
{
    let progress = calculate_load_up_progress(active, prepared, dependencies);
    entry.set_progress(progress.done, progress.total);
}
