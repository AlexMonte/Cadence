//! Keyboard movement uses the same projected cells and authored identities as picking.
//! No document or selection is mutated here.

use crate::application::editor::{EditorAttention, FocusTarget, PickHit};
use crate::application::pipeline::scene_sync::VisibleBoardState;
use crate::domain::board::{BoardSlot, SurfaceLayoutKind};
use crate::domain::document::PlacementAddress;

#[derive(Debug, Clone, Copy)]
pub enum Motion {
    Step(i32, i32),
    Expression(i32),
    First,
    Last,
}

pub fn focused_address(
    attention: &EditorAttention,
    visible: &VisibleBoardState,
) -> Option<PlacementAddress> {
    match &attention.focus {
        FocusTarget::EmptySlot { slot, .. } => Some(PlacementAddress::BoardSlot(*slot)),
        FocusTarget::StackInsert { index, .. } => Some(PlacementAddress::StackIndex(*index)),
        FocusTarget::Tile { node }
        | FocusTarget::Atom { node }
        | FocusTarget::Port { node, .. } => visible.address_of(node),
        _ => None,
    }
}

fn position(address: PlacementAddress) -> BoardSlot {
    match address {
        PlacementAddress::BoardSlot(slot) => slot,
        PlacementAddress::StackIndex(index) => BoardSlot::new(index.0 as i32, 0),
    }
}

fn target_at(visible: &VisibleBoardState, position: BoardSlot) -> Option<FocusTarget> {
    Some(match visible.pick_at(position)? {
        PickHit::Tile { node } => FocusTarget::Tile { node },
        PickHit::AtomCompound { primary_node, .. } => FocusTarget::Tile { node: primary_node },
        PickHit::EmptySlot { surface, slot } => FocusTarget::EmptySlot { surface, slot },
        PickHit::StackInsert { surface, index } => FocusTarget::StackInsert { surface, index },
        PickHit::Port { node, port } => FocusTarget::Port { node, port },
    })
}

pub fn range_nodes(
    visible: &VisibleBoardState,
    anchor: &FocusTarget,
    target: &FocusTarget,
) -> Vec<tessera::prelude::NodeId> {
    let Some(surface) = visible.active_surface else {
        return Vec::new();
    };
    let coordinate = |focus: &FocusTarget| {
        let mut attention = EditorAttention::new(surface);
        attention.focus = focus.clone();
        focused_address(&attention, visible)
            .map(|address| position(visible.display_address(address)))
    };
    let (Some(a), Some(b)) = (coordinate(anchor), coordinate(target)) else {
        return Vec::new();
    };
    visible
        .nodes
        .iter()
        .filter_map(|node| {
            let slot = position(visible.display_address(node.address));
            let within = slot.x >= a.x.min(b.x)
                && slot.x <= a.x.max(b.x)
                && slot.y >= a.y.min(b.y)
                && slot.y <= a.y.max(b.y);
            if !within {
                return None;
            }
            match target_at(visible, slot) {
                Some(FocusTarget::Tile { node: owner }) if owner == node.node => Some(owner),
                _ => None,
            }
        })
        .collect()
}

pub fn navigate(
    attention: &EditorAttention,
    visible: &VisibleBoardState,
    motion: Motion,
    count: u16,
) -> Option<FocusTarget> {
    if visible.active_surface != Some(attention.active_board()) {
        return None;
    }
    let mut at = focused_address(attention, visible)
        .map(|address| position(visible.display_address(address)))
        .or_else(|| {
            visible
                .nodes
                .first()
                .map(|node| position(visible.display_address(node.address)))
        })
        .unwrap_or(BoardSlot::new(0, 0));
    if matches!(motion, Motion::Expression(_) | Motion::First | Motion::Last) {
        let mut positions: Vec<_> = visible
            .nodes
            .iter()
            .filter_map(|node| {
                let slot = position(visible.display_address(node.address));
                match target_at(visible, slot) {
                    Some(FocusTarget::Tile { node: owner }) if owner == node.node => Some(slot),
                    _ => None,
                }
            })
            .collect();
        positions.sort_by_key(|slot| (slot.y, slot.x));
        positions.dedup();
        at = match motion {
            Motion::First => *positions.first()?,
            Motion::Last => *positions.last()?,
            Motion::Expression(direction) => {
                for _ in 0..count.clamp(1, 999) {
                    let next = if direction > 0 {
                        positions
                            .iter()
                            .find(|slot| (slot.y, slot.x) > (at.y, at.x))
                    } else {
                        positions
                            .iter()
                            .rev()
                            .find(|slot| (slot.y, slot.x) < (at.y, at.x))
                    };
                    if let Some(next) = next {
                        at = *next;
                    } else {
                        break;
                    }
                }
                at
            }
            _ => unreachable!(),
        };
    } else if let Motion::Step(dx, dy) = motion {
        for _ in 0..count.clamp(1, 999) {
            let before = target_at(visible, at);
            // A wide face is one navigation target. Step beyond its footprint,
            // while retaining empty authored cells as valid insertion targets.
            for _ in 0..64 {
                let next = match visible.layout {
                    SurfaceLayoutKind::Board => {
                        BoardSlot::new(at.x.saturating_add(dx), at.y.saturating_add(dy))
                    }
                    SurfaceLayoutKind::Stack => {
                        let stride = crate::domain::board::geometry::STACK_COLUMNS as i32;
                        BoardSlot::new(at.x.saturating_add(dx + dy * stride).max(0), 0)
                    }
                };
                if next == at {
                    break;
                }
                at = next;
                if target_at(visible, at) != before {
                    break;
                }
            }
        }
    }
    target_at(visible, at)
}
