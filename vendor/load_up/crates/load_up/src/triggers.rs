use std::marker::PhantomData;

use bevy_app::App;
use bevy_asset::AssetApp;
use bevy_ecs::{event::Event, observer::On, prelude::*, system::Command};
use bevy_state::prelude::States;

use crate::{
    prepared::{prepare_resource_world, LoadAssetResource},
    state::{prepare_state_world, PackRegistry},
};

pub struct PrepareResourceCommand<T>
where
    T: LoadAssetResource,
{
    marker: PhantomData<T>,
}

impl<T> Default for PrepareResourceCommand<T>
where
    T: LoadAssetResource,
{
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<T> Command for PrepareResourceCommand<T>
where
    T: LoadAssetResource,
{
    fn apply(self, world: &mut World) {
        prepare_resource_world::<T>(world);
    }
}

pub struct PrepareStateCommand<S>
where
    S: States,
{
    pub state: S,
}

impl<S> Command for PrepareStateCommand<S>
where
    S: States + Clone + Eq + std::hash::Hash,
{
    fn apply(self, world: &mut World) {
        prepare_state_world::<S>(world, self.state);
    }
}

pub trait PrepareTriggerAppExt {
    fn prepare_resource_on<T, E>(&mut self) -> &mut Self
    where
        T: LoadAssetResource,
        E: Event;

    fn prepare_state_on<S, E>(&mut self, state: S) -> &mut Self
    where
        S: States + Clone + Eq + std::hash::Hash,
        E: Event;
}

impl PrepareTriggerAppExt for App {
    fn prepare_resource_on<T, E>(&mut self) -> &mut Self
    where
        T: LoadAssetResource,
        E: Event,
    {
        self.init_asset::<T>();
        self.init_resource::<crate::prepared::PreparedResources>();
        self.init_resource::<crate::dependency::DependencyRegistry>();

        self.add_observer(|_: On<E>, mut commands: Commands| {
            commands.queue(PrepareResourceCommand::<T>::default());
        });

        self
    }

    fn prepare_state_on<S, E>(&mut self, state: S) -> &mut Self
    where
        S: States + Clone + Eq + std::hash::Hash,
        E: Event,
    {
        self.init_resource::<PackRegistry<S>>();

        self.add_observer(move |_: On<E>, mut commands: Commands| {
            commands.queue(PrepareStateCommand::<S> {
                state: state.clone(),
            });
        });

        self
    }
}
