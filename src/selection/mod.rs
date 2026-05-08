use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Default)]
pub struct SelectionStore {
    pub revision: u64,
}
