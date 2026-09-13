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
    /// Resolve the exact authored note at its current location. Deleted notes
    /// remain unlinked even if the last valid score is still playing.
    pub fn rebuild(
        &mut self,
        preview: &super::RuntimePreviewSnapshot,
        source_nodes: &BTreeMap<u64, NodeId>,
        document: &crate::domain::document::MusaicDocument,
    ) {
        self.clear();
        for event in preview.events() {
            let Some(node) = event.source_id.and_then(|id| source_nodes.get(&id)) else {
                continue;
            };
            if let Some(location) = document.graph.location_of(node) {
                self.insert_source(event.id, location.surface, node.clone());
            }
        }
    }

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
