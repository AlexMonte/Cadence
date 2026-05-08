use crate::domain::board::{
    BoardSurfaceKind, PlacementRule, PlacementVerdict, SlotCoord, TileClass, TileInstanceId,
};

use super::BoardStore;

pub fn validate_placement(
    store: &BoardStore,
    surface_id: &crate::domain::board::BoardSurfaceId,
    slot: SlotCoord,
    class: TileClass,
    moving_tile: Option<&TileInstanceId>,
) -> PlacementVerdict {
    let Some(surface) = store.surface(surface_id) else {
        return PlacementVerdict::Invalid {
            reason: "unknown board surface".to_string(),
        };
    };

    if !surface.extent.contains(slot) {
        return PlacementVerdict::Invalid {
            reason: "slot out of bounds".to_string(),
        };
    }

    if !PlacementRule::accepts(surface.kind, class) {
        let reason = match surface.kind {
            BoardSurfaceKind::Flow => "flow board only accepts containers, transforms, and terminals",
            BoardSurfaceKind::ContainerLocal => {
                "container board only accepts atoms and nested containers"
            }
        };
        return PlacementVerdict::Invalid {
            reason: reason.to_string(),
        };
    }

    if let Some(occupant) = surface.occupancy.get(slot)
        && Some(occupant) != moving_tile
    {
        return PlacementVerdict::Invalid {
            reason: "slot already occupied".to_string(),
        };
    }

    if let Some(tile_id) = moving_tile
        && let Some(tile) = store.tile(tile_id)
        && tile.class == TileClass::Container
        && let Some(local_surface_id) = &tile.local_surface_id
        && store.is_descendant_surface(local_surface_id, surface_id)
    {
        return PlacementVerdict::Invalid {
            reason: "container cannot move into its own descendant surface".to_string(),
        };
    }

    PlacementVerdict::Valid
}
