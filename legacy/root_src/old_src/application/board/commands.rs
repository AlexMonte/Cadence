use crate::domain::board::{BoardSurfaceId, SlotCoord, TileClass, TileInstanceId};

use super::{BoardPointer, BoardStore};

#[derive(Debug, Clone)]
pub enum BoardCommand {
    SelectSlot {
        board_id: BoardSurfaceId,
        slot: Option<SlotCoord>,
    },
    OpenContainer {
        tile_id: TileInstanceId,
    },
    CloseContainer,
    StartPaletteDrag {
        piece_id: String,
        class: TileClass,
    },
    StartTileDrag {
        tile_id: TileInstanceId,
    },
    UpdateDrag {
        pointer: BoardPointer,
    },
    CommitDrag,
    CancelDrag,
    ZoomViewport {
        factor: f32,
    },
    PanViewport {
        delta: crate::application::board::BoardSpaceVec2,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("{0}")]
    Message(String),
}

impl From<String> for CommandError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

pub fn assert_store_invariants(store: &BoardStore) -> CommandResult<()> {
    use std::collections::BTreeSet;

    let mut seen_tiles = BTreeSet::new();
    for (surface_id, surface) in &store.surfaces {
        for (slot, tile_id) in surface.occupancy.iter() {
            let tile = store
                .tiles
                .get(tile_id)
                .ok_or_else(|| CommandError::Message("occupied slot points to missing tile".into()))?;
            if &tile.parent_surface != surface_id {
                return Err(CommandError::Message(
                    "tile parent surface does not match occupancy surface".into(),
                ));
            }
            if store.slot_index.get(tile_id) != Some(slot) {
                return Err(CommandError::Message(
                    "tile slot index does not match occupancy".into(),
                ));
            }
            if !seen_tiles.insert(tile_id.clone()) {
                return Err(CommandError::Message(
                    "tile appears in more than one occupied slot".into(),
                ));
            }
            if tile.class == TileClass::Container {
                let Some(local_surface_id) = &tile.local_surface_id else {
                    return Err(CommandError::Message(
                        "container tile missing local surface".into(),
                    ));
                };
                let Some(local_surface) = store.surfaces.get(local_surface_id) else {
                    return Err(CommandError::Message(
                        "container local surface missing".into(),
                    ));
                };
                if local_surface.kind != crate::domain::board::BoardSurfaceKind::ContainerLocal {
                    return Err(CommandError::Message(
                        "container local surface has wrong kind".into(),
                    ));
                }
            }
        }
    }

    Ok(())
}
