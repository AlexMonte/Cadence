pub mod occupancy;
pub mod placement;
pub mod slot;
pub mod surface;
pub mod tile_instance;

pub use occupancy::SlotOccupancy;
pub use placement::{PlacementRule, PlacementVerdict};
pub use slot::{BoardExtent, SlotCoord};
pub use surface::{BoardSurface, BoardSurfaceId, BoardSurfaceKind};
pub use tile_instance::{TileClass, TileInstance, TileInstanceId};
