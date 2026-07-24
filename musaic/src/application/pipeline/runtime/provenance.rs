use std::collections::BTreeMap;

use bevy::prelude::*;
use tessera::prelude::NodeId;

use crate::application::editor::{TimelineSource, TimelineSourceResolver};
use crate::domain::board::BoardSurfaceId;

use super::ProjectedEventId;

#[derive(Resource, Debug, Clone, Default)]
pub struct TimelineProvenanceStore {
    event_sources: BTreeMap<ProjectedEventId, TimelineSource>,
}

impl TimelineProvenanceStore {
    pub fn insert_source(
        &mut self,
        event: ProjectedEventId,
        surface: BoardSurfaceId,
        node: NodeId,
    ) {
        self.event_sources
            .insert(event, TimelineSource { surface, node });
    }

    pub fn clear(&mut self) {
        self.event_sources.clear();
    }
}

impl TimelineSourceResolver for TimelineProvenanceStore {
    fn source_for_event(&self, event: ProjectedEventId) -> Option<TimelineSource> {
        self.event_sources.get(&event).cloned()
    }
}
