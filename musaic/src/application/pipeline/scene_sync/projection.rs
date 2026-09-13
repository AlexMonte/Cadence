//! Direct authored-document projection into the board read model.

use std::collections::{BTreeMap, BTreeSet};

use tessera::prelude::NodeId;

use super::{
    logic::{focused_node_from_attention, render_focus_from_attention},
    surface_content::{
        atom_compounds_for_surface, display_compounds_for_surface, surface_content_for_node,
        visible_node_kind,
    },
    types::{TileSurfaceContent, VisibleBoardConnection, VisibleBoardNode, VisibleNodeKind},
    view_model::VisibleBoardState,
};
use crate::{
    application::{
        board_view_settings::{AtomDisplayMode, BoardViewSettings},
        editor::{EditorAttention, SelectionState, connection_endpoint_view},
        session::MusaicProject,
    },
    domain::{
        board::{BoardSlot, BoardSurfaceId, SurfaceLayoutKind},
        document::{
            DocumentNodeKind, DocumentQueries, MusaicDocument, PlacementAddress, StackIndex,
        },
        instrument::InstrumentSource,
    },
};

/// Builds the only retained board projection. All intermediate collections are
/// local indexes used to finish this value in one pass.
pub(super) fn project_visible_board(
    project: &MusaicProject,
    attention: &EditorAttention,
    selection: &SelectionState,
    view_settings: BoardViewSettings,
) -> VisibleBoardState {
    let queries = DocumentQueries::new(&project.document);
    let active_surface = attention.active_board();
    let layout = queries
        .surface_layout(active_surface)
        .unwrap_or(SurfaceLayoutKind::Board);
    let display_mode =
        atom_display_mode_for_surface(&project.document, active_surface, layout, view_settings);
    let focused_node = focused_node_from_attention(attention);
    let focus = render_focus_from_attention(&queries, attention, active_surface);

    let mut nodes = Vec::new();
    let mut occupied_stack_indices = Vec::new();
    let mut board_addresses_by_node = BTreeMap::new();

    for (placement, authored) in queries.active_surface_tiles(active_surface) {
        let address = placement.address;
        let slot = match address {
            PlacementAddress::BoardSlot(slot) => {
                board_addresses_by_node.insert(authored.id.clone(), slot);
                slot
            }
            PlacementAddress::StackIndex(index) => {
                occupied_stack_indices.push(index);
                BoardSlot::new(index.0 as i32, 0)
            }
        };
        let kind = visible_node_kind(&authored.kind);
        let tessera_footprint =
            if layout == SurfaceLayoutKind::Stack && kind == VisibleNodeKind::Container {
                Some(tessera::prelude::TileFootprint::new(2, 1))
            } else {
                root_board_footprint(&project.document, active_surface, &authored.id)
            };
        let icon = match &authored.kind {
            DocumentNodeKind::TrickInstance(trick) => Some(
                crate::adapter::tile_icons::icon_for_trick_prototype(trick.prototype),
            ),
            _ => None,
        };
        let atom = match &authored.kind {
            DocumentNodeKind::Atom(atom) => Some(atom.atom.clone()),
            _ => None,
        };
        let ports = match address {
            PlacementAddress::BoardSlot(_) => {
                connection_endpoint_view(&queries, &authored.id, Some(slot), None)
            }
            PlacementAddress::StackIndex(_) => None,
        };

        nodes.push(VisibleBoardNode {
            node: authored.id.clone(),
            address,
            tessera_footprint,
            kind,
            selected: selection.contains(authored.id.clone()),
            focused: focused_node.as_ref() == Some(&authored.id),
            icon,
            atom,
            ports,
            surface_content: TileSurfaceContent::Empty,
        });
    }

    let raw_compounds = atom_compounds_for_surface(&queries, active_surface, display_mode);
    let wide_containers = nodes.iter().filter_map(|node| {
        (node.kind == VisibleNodeKind::Container)
            .then_some(node.address)
            .and_then(|address| match address {
                PlacementAddress::StackIndex(index) => Some(index),
                PlacementAddress::BoardSlot(_) => None,
            })
    });
    let (atom_compounds, stack_display) =
        display_compounds_for_surface(&queries, layout, &raw_compounds, wide_containers);

    let mut compound_by_member = BTreeMap::new();
    let mut compound_members = BTreeSet::new();
    let mut compound_highlights = BTreeMap::new();
    for compound in &atom_compounds {
        let Some(anchor) = compound.compound.members.first() else {
            continue;
        };
        compound_by_member.insert(
            anchor.clone(),
            (
                compound.compound.display.clone(),
                compound.compound.members.clone(),
            ),
        );
        compound_members.extend(compound.compound.members.iter().cloned());
        compound_highlights.insert(
            anchor.clone(),
            (
                nodes
                    .iter()
                    .any(|node| compound.compound.members.contains(&node.node) && node.selected),
                nodes
                    .iter()
                    .any(|node| compound.compound.members.contains(&node.node) && node.focused),
            ),
        );
    }

    for node in &mut nodes {
        node.surface_content =
            surface_content_for_node(&queries, &node.node, &compound_by_member, &compound_members);
        if matches!(
            project
                .document
                .graph
                .sound_definition(&node.node)
                .map(|instrument| &instrument.source),
            Some(InstrumentSource::Kit)
        ) {
            if let TileSurfaceContent::Transform { label, .. } = &mut node.surface_content {
                *label = "Kit".into();
            }
        }
        if let Some((selected, focused)) = compound_highlights.get(&node.node) {
            node.selected = *selected;
            node.focused = *focused;
        }
    }

    occupied_stack_indices.sort_by_key(|index| index.0);
    let occupied_set = occupied_stack_indices
        .iter()
        .map(|index| index.0)
        .collect::<BTreeSet<_>>();
    let (stack_inserts, stack_locked_slots) = match layout {
        SurfaceLayoutKind::Board => (Vec::new(), Vec::new()),
        SurfaceLayoutKind::Stack => {
            let max_index = occupied_stack_indices.iter().map(|index| index.0).max();
            let insert = StackIndex(max_index.map_or(0, |max| max + 1));
            let locked = max_index.map_or_else(Vec::new, |max| {
                (0..=max)
                    .filter(|index| !occupied_set.contains(index))
                    .map(StackIndex)
                    .collect()
            });
            (vec![insert], locked)
        }
    };

    let connections = if layout == SurfaceLayoutKind::Board {
        queries
            .connections_on_surface(active_surface)
            .into_iter()
            .filter_map(|connection| {
                let from_slot = board_addresses_by_node.get(&connection.from).copied()?;
                let to_slot = board_addresses_by_node.get(&connection.to).copied()?;
                (from_slot != to_slot).then_some(VisibleBoardConnection {
                    side: connection.spatial_side,
                    from: connection.from,
                    to: connection.to,
                    from_slot,
                    to_slot,
                    kind: connection.kind,
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    VisibleBoardState {
        active_surface: Some(active_surface),
        layout,
        nodes,
        atom_compounds,
        stack_inserts,
        stack_locked_slots,
        stack_display,
        connections,
        focus,
    }
}

fn atom_display_mode_for_surface(
    document: &MusaicDocument,
    surface: BoardSurfaceId,
    layout: SurfaceLayoutKind,
    view_settings: BoardViewSettings,
) -> AtomDisplayMode {
    match layout {
        SurfaceLayoutKind::Stack => view_settings.container_interior,
        SurfaceLayoutKind::Board if surface == document.root_surface => view_settings.root_board,
        SurfaceLayoutKind::Board => view_settings.container_preview_on_root,
    }
}

fn root_board_footprint(
    document: &MusaicDocument,
    surface: BoardSurfaceId,
    node_id: &NodeId,
) -> Option<tessera::prelude::TileFootprint> {
    if surface != document.root_surface {
        return None;
    }
    document
        .graph
        .node(node_id)
        .map(|node| crate::domain::document::root_board_tile_footprint(&node.kind))
}
