use bevy::prelude::*;
use cadence_core::domain::{rational::Time as MusicalTime, span::Span};

#[derive(Resource, Debug, Clone)]
pub struct TransportState {
    pub position: MusicalTime,
    pub bpm: f64,
    pub loop_region: Option<Span>,
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            position: MusicalTime::ZERO,
            bpm: 120.0,
            loop_region: None,
        }
    }
}

pub struct TransportPlugin;

impl Plugin for TransportPlugin {
    fn build(&self, _app: &mut App) {}
}
