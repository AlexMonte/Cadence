use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;

use crate::application::pipeline::runtime::ProjectedEventId;
use crate::domain::board::{BoardSlot, BoardSurfaceId, BoardSurfaceKind, BoardSurfaces};
use crate::domain::document::{
    AtomValue, ContainerKind, DocumentNodeKind, DocumentQueries, GraphTilePrototypeId,
    NoteName as DocumentNoteName, OperatorValue, PlacementAddress, StackIndex, TileSpawnKind,
};

use crate::application::editor::connection::{ConnectionEndpointView, PortSlotState};
use crate::application::editor::transaction::PlacementTarget;
use crate::application::editor::workspace::{
    ActiveSpace, EditorAttention, FocusTarget, WorkspaceMode,
};

/// One panel in the right-rail inspector stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectorPanelKind {
    DrawerPanel {
        target: Option<PlacementTarget>,
        context: TileLibraryContextKind,
    },
    PlacementPromptPanel {
        target: Option<PlacementTarget>,
        context: TileLibraryContextKind,
    },
    /// Name, description, and IO readout for the focused tile/atom/port.
    TileInspectPanel {
        node: NodeId,
    },
    SelectionSummaryPanel {
        count: usize,
        primary: Option<NodeId>,
    },
    TimelineEventPanel {
        event: ProjectedEventId,
    },
    ProjectOverviewPanel {
        surface: BoardSurfaceId,
    },
}

/// Right-rail presentation derived from editor focus — an ordered panel stack.
///
/// `panels` is ordered bottom-to-top: `[DrawerPanel, TileInspectPanel]` means
/// the tile inspect panel renders above the drawer. Exclusive panels
/// (selection summary, timeline event, project overview) occupy the stack
/// alone. An empty stack means "nothing to inspect".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectorLayout {
    pub panels: Vec<InspectorPanelKind>,
}

impl InspectorLayout {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn single(panel: InspectorPanelKind) -> Self {
        Self {
            panels: vec![panel],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }

    /// Topmost panel (visually front of the stack).
    pub fn top(&self) -> Option<&InspectorPanelKind> {
        self.panels.last()
    }

    /// Tile library context when a drawer panel is part of the stack.
    pub fn drawer_context(&self) -> Option<TileLibraryContextKind> {
        self.panels.iter().find_map(|panel| match panel {
            InspectorPanelKind::DrawerPanel { context, .. } => Some(*context),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileLibraryContextKind {
    RootBoard,
    ContainerBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileDrawerItem {
    pub label: String,
    pub spawn: TileSpawnKind,
}

impl TileDrawerItem {
    pub fn container(label: impl Into<String>, kind: ContainerKind) -> Self {
        Self {
            label: label.into(),
            spawn: TileSpawnKind::Container { kind },
        }
    }

    pub fn atom(label: impl Into<String>, atom: AtomValue) -> Self {
        Self {
            label: label.into(),
            spawn: TileSpawnKind::Atom { atom },
        }
    }
}

/// Derive the inspector panel stack from editor focus, selection, and drawer state.
///
/// Stack precedence (bottom → top): drawer, placement prompt, tile inspect.
/// Selection summary, timeline event, and project overview replace the stack.
pub fn derive_inspector_layout(
    attention: &EditorAttention,
    selection: &crate::application::editor::SelectionState,
    queries: &DocumentQueries<'_>,
    drawer_open: bool,
) -> InspectorLayout {
    let selection_size = selection.nodes.len();
    if selection_size > 1 {
        let primary = match &attention.focus {
            FocusTarget::Tile { node }
            | FocusTarget::Atom { node }
            | FocusTarget::Port { node, .. } => Some(node.clone()),
            _ => None,
        };
        return InspectorLayout::single(InspectorPanelKind::SelectionSummaryPanel {
            count: selection_size,
            primary,
        });
    }

    if let FocusTarget::TimelineEvent { event } = attention.focus {
        return InspectorLayout::single(InspectorPanelKind::TimelineEventPanel { event });
    }

    let mut panels = Vec::new();

    if drawer_open {
        if let Some((target, context)) = drawer_target_for_attention(attention, queries) {
            panels.push(InspectorPanelKind::DrawerPanel { target, context });
        }
    }

    match &attention.focus {
        FocusTarget::None => {
            if panels.is_empty() {
                if attention.workspace_mode == WorkspaceMode::Compose && drawer_open {
                    let surface = attention.active_board();
                    panels.push(InspectorPanelKind::DrawerPanel {
                        target: None,
                        context: library_context_for_surface(queries, surface),
                    });
                } else {
                    return InspectorLayout::single(InspectorPanelKind::ProjectOverviewPanel {
                        surface: attention.active_board(),
                    });
                }
            }
        }
        FocusTarget::EmptySlot { surface, slot } => {
            if panels.is_empty() {
                match empty_slot_prompt(attention, &queries.document.surfaces, *surface, *slot) {
                    Some(prompt) => panels.push(prompt),
                    None => return InspectorLayout::empty(),
                }
            }
        }
        FocusTarget::StackInsert { surface, index } => {
            if panels.is_empty() {
                panels.push(InspectorPanelKind::PlacementPromptPanel {
                    target: Some(PlacementTarget::StackIndex {
                        surface: *surface,
                        index: *index,
                    }),
                    context: TileLibraryContextKind::ContainerBody,
                });
            }
        }
        FocusTarget::Tile { node }
        | FocusTarget::Atom { node }
        | FocusTarget::Port { node, .. } => {
            panels.push(InspectorPanelKind::TileInspectPanel { node: node.clone() });
        }
        FocusTarget::TimelineEvent { .. } => unreachable!("timeline focus handled above"),
    }

    InspectorLayout { panels }
}

fn library_context_for_surface(
    queries: &DocumentQueries<'_>,
    surface: BoardSurfaceId,
) -> TileLibraryContextKind {
    match queries.surface_kind(surface) {
        Some(BoardSurfaceKind::ContainerStack { .. }) => TileLibraryContextKind::ContainerBody,
        Some(BoardSurfaceKind::RootBoard) | None => TileLibraryContextKind::RootBoard,
    }
}

fn drawer_target_for_attention(
    attention: &EditorAttention,
    queries: &DocumentQueries<'_>,
) -> Option<(Option<PlacementTarget>, TileLibraryContextKind)> {
    let active = attention.active_board();
    match &attention.focus {
        FocusTarget::EmptySlot { surface, slot } if *surface == active => Some((
            Some(PlacementTarget::BoardSlot {
                surface: *surface,
                slot: *slot,
            }),
            if queries.surface_kind(*surface) == Some(BoardSurfaceKind::RootBoard) {
                TileLibraryContextKind::RootBoard
            } else {
                TileLibraryContextKind::ContainerBody
            },
        )),
        FocusTarget::StackInsert { surface, index } if *surface == active => Some((
            Some(PlacementTarget::StackIndex {
                surface: *surface,
                index: *index,
            }),
            TileLibraryContextKind::ContainerBody,
        )),
        FocusTarget::Atom { .. } | FocusTarget::Tile { .. } | FocusTarget::Port { .. } => {
            match queries.surface_kind(active) {
                Some(BoardSurfaceKind::RootBoard) => {
                    Some((None, TileLibraryContextKind::RootBoard))
                }
                Some(BoardSurfaceKind::ContainerStack { .. }) => {
                    let next_index = next_stack_insert_index(queries, active);
                    Some((
                        Some(PlacementTarget::StackIndex {
                            surface: active,
                            index: next_index,
                        }),
                        TileLibraryContextKind::ContainerBody,
                    ))
                }
                None => None,
            }
        }
        FocusTarget::None => {
            if queries.surface_kind(active) == Some(BoardSurfaceKind::RootBoard) {
                None
            } else {
                let next_index = next_stack_insert_index(queries, active);
                Some((
                    Some(PlacementTarget::StackIndex {
                        surface: active,
                        index: next_index,
                    }),
                    TileLibraryContextKind::ContainerBody,
                ))
            }
        }
        _ => None,
    }
}

fn next_stack_insert_index(queries: &DocumentQueries<'_>, surface: BoardSurfaceId) -> StackIndex {
    let mut max = None::<usize>;
    for (location, _node) in queries.document.graph.nodes_on_surface(surface) {
        if let PlacementAddress::StackIndex(index) = location.address {
            max = Some(max.map_or(index.0, |m| m.max(index.0)));
        }
    }
    StackIndex(max.map_or(0, |m| m + 1))
}

pub fn labeled_tile_drawer_items() -> Vec<TileDrawerItem> {
    vec![
        TileDrawerItem::container("Sequence", ContainerKind::Sequence),
        TileDrawerItem::container("Subdivision", ContainerKind::Subdivision),
        TileDrawerItem::container("Alternating", ContainerKind::Alternating),
        TileDrawerItem::container("Parallel", ContainerKind::Parallel),
        TileDrawerItem {
            label: "Output".into(),
            spawn: TileSpawnKind::Output {
                name: "main".into(),
            },
        },
        TileDrawerItem {
            label: "Fast".into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(0),
            },
        },
        TileDrawerItem {
            label: "Slow".into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(1),
            },
        },
        TileDrawerItem {
            label: "Legato".into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(2),
            },
        },
        TileDrawerItem {
            label: "Gain".into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(3),
            },
        },
    ]
}

pub fn atom_tile_drawer_rows() -> Vec<(&'static str, Vec<TileDrawerItem>)> {
    use crate::domain::document::Accidental as DocumentAccidental;

    vec![
        (
            "Accidentals",
            vec![
                TileDrawerItem::atom("♯", AtomValue::Accidental(DocumentAccidental::Sharp)),
                TileDrawerItem::atom("♭", AtomValue::Accidental(DocumentAccidental::Flat)),
                TileDrawerItem::atom("♮", AtomValue::Accidental(DocumentAccidental::Natural)),
            ],
        ),
        (
            "Notes",
            vec![
                TileDrawerItem::atom("A", AtomValue::NoteName(DocumentNoteName::A)),
                TileDrawerItem::atom("B", AtomValue::NoteName(DocumentNoteName::B)),
                TileDrawerItem::atom("C", AtomValue::NoteName(DocumentNoteName::C)),
                TileDrawerItem::atom("D", AtomValue::NoteName(DocumentNoteName::D)),
                TileDrawerItem::atom("E", AtomValue::NoteName(DocumentNoteName::E)),
                TileDrawerItem::atom("F", AtomValue::NoteName(DocumentNoteName::F)),
                TileDrawerItem::atom("G", AtomValue::NoteName(DocumentNoteName::G)),
                TileDrawerItem::atom("~", AtomValue::Rest),
            ],
        ),
        (
            "Scalars",
            (0..=9)
                .map(|digit| {
                    TileDrawerItem::atom(digit.to_string(), AtomValue::Number(i32::from(digit)))
                })
                .collect(),
        ),
        (
            "Operators",
            vec![
                TileDrawerItem::atom("^", AtomValue::Operator(OperatorValue::Power)),
                TileDrawerItem::atom("@", AtomValue::Operator(OperatorValue::At)),
                TileDrawerItem::atom("×", AtomValue::Operator(OperatorValue::Multiply)),
                TileDrawerItem::atom("÷", AtomValue::Operator(OperatorValue::Divide)),
            ],
        ),
    ]
}

/// Title for one panel in the stack.
pub fn inspector_panel_title(panel: &InspectorPanelKind, queries: &DocumentQueries<'_>) -> String {
    match panel {
        InspectorPanelKind::PlacementPromptPanel { .. } => "Empty slot".to_string(),
        InspectorPanelKind::DrawerPanel {
            context: TileLibraryContextKind::RootBoard,
            ..
        } => "Tile Drawer".to_string(),
        InspectorPanelKind::DrawerPanel {
            context: TileLibraryContextKind::ContainerBody,
            ..
        } => "Atom Tile Drawer".to_string(),
        InspectorPanelKind::TileInspectPanel { node } => tile_inspect_title(queries, node),
        InspectorPanelKind::SelectionSummaryPanel { count, .. } => format!("{count} selected"),
        InspectorPanelKind::ProjectOverviewPanel { .. } => "Board".to_string(),
        InspectorPanelKind::TimelineEventPanel { .. } => "Timeline".to_string(),
    }
}

/// Inspector header title: the topmost panel names the stack.
pub fn inspector_title(layout: &InspectorLayout, queries: &DocumentQueries<'_>) -> String {
    layout
        .top()
        .map(|panel| inspector_panel_title(panel, queries))
        .unwrap_or_else(|| "Context".to_string())
}

pub fn tile_inspect_title(queries: &DocumentQueries<'_>, node: &NodeId) -> String {
    match queries.node_kind(node) {
        Some(DocumentNodeKind::Container(container)) => match container.kind {
            ContainerKind::Sequence => "Sequence".to_string(),
            ContainerKind::Subdivision => "Subdivision".to_string(),
            ContainerKind::Alternating => "Alternating".to_string(),
            ContainerKind::Parallel => "Parallel".to_string(),
        },
        Some(DocumentNodeKind::Output(output)) => {
            format!("Output · {}", output.name)
        }
        Some(DocumentNodeKind::TrickInstance(trick)) => trick_label(trick.prototype),
        Some(DocumentNodeKind::Atom(_)) => "Atom".to_string(),
        Some(DocumentNodeKind::Tile(_)) => "Transform".to_string(),
        Some(DocumentNodeKind::Arrangement(_)) => "Arrangement".to_string(),
        None => format!("Tile {}", node.0),
    }
}

pub fn tile_inspect_description(queries: &DocumentQueries<'_>, node: &NodeId) -> String {
    match queries.node_kind(node) {
        Some(DocumentNodeKind::Container(container)) => match container.kind {
            ContainerKind::Sequence => "Play child patterns one after another.".to_string(),
            ContainerKind::Subdivision => "Divide time inside this container.".to_string(),
            ContainerKind::Alternating => "Cycle through child patterns each loop.".to_string(),
            ContainerKind::Parallel => "Play all child patterns together.".to_string(),
        },
        Some(DocumentNodeKind::Output(_)) => {
            "Final output for this branch of the graph.".to_string()
        }
        Some(DocumentNodeKind::TrickInstance(trick)) => {
            format!(
                "{} transform on the signal path.",
                trick_label(trick.prototype)
            )
        }
        Some(DocumentNodeKind::Atom(atom)) => format!("{:?}", atom.atom),
        Some(DocumentNodeKind::Tile(_)) => "Transform tile on the board.".to_string(),
        Some(DocumentNodeKind::Arrangement(_)) => {
            "Timed flow arrangement (Strudel-style arrange).".to_string()
        }
        None => "Unknown tile.".to_string(),
    }
}

pub fn tile_inspect_io_lines(view: &ConnectionEndpointView) -> [String; 4] {
    [
        format!("North · {}", port_slot_label(view.north)),
        format!("East · {}", port_slot_label(view.east)),
        format!("South · {}", port_slot_label(view.south)),
        format!("West · {}", port_slot_label(view.west)),
    ]
}

fn port_slot_label(state: PortSlotState) -> &'static str {
    match state {
        PortSlotState::None => "None",
        PortSlotState::Input => "Input",
        PortSlotState::Output => "Output",
    }
}

fn trick_label(prototype: GraphTilePrototypeId) -> String {
    match prototype.0 {
        0 => "Fast".to_string(),
        1 => "Slow".to_string(),
        2 => "Legato".to_string(),
        3 => "Gain".to_string(),
        _ => format!("Trick {}", prototype.0),
    }
}

/// Placement prompt for a focused empty slot; `None` when the slot is not
/// actionable (wrong mode, inactive surface, unknown surface).
fn empty_slot_prompt(
    attention: &EditorAttention,
    surfaces: &BoardSurfaces,
    surface: BoardSurfaceId,
    slot: BoardSlot,
) -> Option<InspectorPanelKind> {
    if attention.workspace_mode != WorkspaceMode::Compose {
        return None;
    }

    let ActiveSpace::Board(active_surface) = attention.active_space;
    if active_surface != surface {
        return None;
    }

    let surface_data = surfaces.get(surface)?;
    let context = match surface_data.kind {
        BoardSurfaceKind::RootBoard => TileLibraryContextKind::RootBoard,
        BoardSurfaceKind::ContainerStack { .. } => TileLibraryContextKind::ContainerBody,
    };

    Some(InspectorPanelKind::PlacementPromptPanel {
        target: Some(PlacementTarget::BoardSlot { surface, slot }),
        context,
    })
}

pub fn basic_tile_options(context: TileLibraryContextKind) -> Vec<TileDrawerItem> {
    match context {
        TileLibraryContextKind::RootBoard => labeled_tile_drawer_items(),
        TileLibraryContextKind::ContainerBody => atom_tile_drawer_rows()
            .into_iter()
            .flat_map(|(_, items)| items)
            .collect(),
    }
}

pub fn placement_target_label(target: &PlacementTarget) -> String {
    match target {
        PlacementTarget::BoardSlot { surface, slot } => {
            format!(
                "Board surface {} · slot ({}, {})",
                surface.0, slot.x, slot.y
            )
        }
        PlacementTarget::StackIndex { surface, index } => {
            format!("Stack surface {} · insert {}", surface.0, index.0)
        }
    }
}

/// Short catalog label for an armed / palette spawn kind.
pub fn drawer_item_short_label(spawn: &TileSpawnKind) -> String {
    labeled_tile_drawer_items()
        .into_iter()
        .find(|item| &item.spawn == spawn)
        .map(|item| item.label)
        .or_else(|| {
            atom_tile_drawer_rows()
                .into_iter()
                .flat_map(|(_, row)| row)
                .find(|item| &item.spawn == spawn)
                .map(|item| item.label)
        })
        .unwrap_or_else(|| format!("{spawn:?}"))
}

/// Minimap surface navigation buttons (Home + active container).
pub fn minimap_surface_buttons(
    queries: &DocumentQueries<'_>,
    active_surface: BoardSurfaceId,
) -> Vec<(String, BoardSurfaceId)> {
    let mut out = Vec::new();
    let root = queries.document.root_surface;
    out.push(("Home".to_string(), root));
    if active_surface != root {
        if let Some(kind) = queries.surface_kind(active_surface) {
            let label = match kind {
                BoardSurfaceKind::RootBoard => "Home".to_string(),
                BoardSurfaceKind::ContainerStack { container } => container.0.to_string(),
            };
            if !out.iter().any(|(_, s)| *s == active_surface) {
                out.push((label, active_surface));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use tessera::prelude::{ContainerId, NodeId};

    use super::*;
    use crate::application::editor::workspace::NavigationMode;
    use crate::domain::board::BoardSurface;
    use crate::domain::document::MusaicDocument;

    fn root_surface() -> BoardSurfaceId {
        BoardSurfaceId(1)
    }

    fn container_surface() -> BoardSurfaceId {
        BoardSurfaceId(2)
    }

    fn unknown_surface() -> BoardSurfaceId {
        BoardSurfaceId(999)
    }

    fn node(id: &str) -> NodeId {
        NodeId::new(id)
    }

    fn event(id: u64) -> ProjectedEventId {
        ProjectedEventId(id)
    }

    fn surfaces() -> BoardSurfaces {
        let mut surfaces = BoardSurfaces::default();
        surfaces
            .insert(BoardSurface {
                id: root_surface(),
                kind: BoardSurfaceKind::RootBoard,
            })
            .unwrap();
        surfaces
            .insert(BoardSurface {
                id: container_surface(),
                kind: BoardSurfaceKind::ContainerStack {
                    container: ContainerId::new("container"),
                },
            })
            .unwrap();
        surfaces
    }

    fn document_with_surfaces() -> MusaicDocument {
        let mut document = MusaicDocument::new_empty();
        document.root_surface = root_surface();
        document.surfaces = surfaces();
        document
    }

    use crate::application::editor::SelectionState;
    use crate::application::editor::panels::logic::default_drawer_open_for_focus;

    fn compose_attention(focus: FocusTarget) -> EditorAttention {
        EditorAttention {
            workspace_mode: WorkspaceMode::Compose,
            active_space: ActiveSpace::Board(root_surface()),
            focus,
        }
    }

    #[test]
    fn empty_slot_focus_with_drawer_open_yields_drawer_only() {
        let slot = BoardSlot::new(2, 3);
        let attention = compose_attention(FocusTarget::EmptySlot {
            surface: root_surface(),
            slot,
        });
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        // Empty slot focus defaults the drawer open (see panels::logic).
        assert!(default_drawer_open_for_focus(
            &attention.focus,
            attention.active_board()
        ));
        assert_eq!(
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, true),
            InspectorLayout::single(InspectorPanelKind::DrawerPanel {
                target: Some(PlacementTarget::BoardSlot {
                    surface: root_surface(),
                    slot,
                }),
                context: TileLibraryContextKind::RootBoard,
            })
        );
    }

    #[test]
    fn empty_slot_focus_with_drawer_closed_yields_placement_prompt() {
        let slot = BoardSlot::new(2, 3);
        let attention = compose_attention(FocusTarget::EmptySlot {
            surface: root_surface(),
            slot,
        });
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        assert_eq!(
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, false),
            InspectorLayout::single(InspectorPanelKind::PlacementPromptPanel {
                target: Some(PlacementTarget::BoardSlot {
                    surface: root_surface(),
                    slot,
                }),
                context: TileLibraryContextKind::RootBoard,
            })
        );
    }

    #[test]
    fn tile_focus_with_drawer_closed_yields_tile_inspect_only() {
        let attention = compose_attention(FocusTarget::Tile { node: node("tile") });
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        // Tile focus defaults the drawer closed.
        assert!(!default_drawer_open_for_focus(
            &attention.focus,
            attention.active_board()
        ));
        assert_eq!(
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, false),
            InspectorLayout::single(InspectorPanelKind::TileInspectPanel { node: node("tile") })
        );
    }

    #[test]
    fn tile_focus_with_user_override_open_stacks_inspect_above_drawer() {
        use crate::application::editor::panels::logic::effective_drawer_open;

        let attention = compose_attention(FocusTarget::Tile { node: node("tile") });
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        // User explicitly opened the drawer; it stays open on tile focus.
        let drawer_open = effective_drawer_open(Some(true), &attention);
        assert!(drawer_open);
        assert_eq!(
            derive_inspector_layout(
                &attention,
                &SelectionState::default(),
                &queries,
                drawer_open
            ),
            InspectorLayout {
                panels: vec![
                    InspectorPanelKind::DrawerPanel {
                        target: None,
                        context: TileLibraryContextKind::RootBoard,
                    },
                    InspectorPanelKind::TileInspectPanel { node: node("tile") },
                ],
            }
        );
    }

    #[test]
    fn drawer_override_persists_across_focus_changes() {
        use crate::application::editor::panels::logic::effective_drawer_open;

        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        let override_open = Some(true);

        // Place-after-inspect workflow: empty slot → tile → back to empty slot.
        let focus_sequence = [
            FocusTarget::EmptySlot {
                surface: root_surface(),
                slot: BoardSlot::new(0, 0),
            },
            FocusTarget::Tile { node: node("tile") },
            FocusTarget::EmptySlot {
                surface: root_surface(),
                slot: BoardSlot::new(1, 0),
            },
        ];

        for focus in focus_sequence {
            let attention = compose_attention(focus);
            let drawer_open = effective_drawer_open(override_open, &attention);
            assert!(drawer_open, "override Some(true) keeps drawer open");
            let layout = derive_inspector_layout(
                &attention,
                &SelectionState::default(),
                &queries,
                drawer_open,
            );
            assert!(
                layout.drawer_context().is_some(),
                "drawer stays in the stack while override is set: {layout:?}"
            );
        }
    }

    #[test]
    fn tile_inspect_renders_above_drawer_in_stack_order() {
        let attention = compose_attention(FocusTarget::Tile { node: node("tile") });
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        let layout =
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, true);
        assert_eq!(
            layout.top(),
            Some(&InspectorPanelKind::TileInspectPanel { node: node("tile") })
        );
    }

    #[test]
    fn multi_selection_replaces_stack_with_selection_summary() {
        use tessera::bevy::TesseraBoard;

        use crate::application::command::{EditorCommand, execute_command};
        use crate::application::editor::transaction::PlacementTarget;
        use crate::application::pipeline::runtime::TimelineProvenanceStore;
        use crate::domain::board::BoardSlot;
        use crate::domain::document::{ContainerKind, TileSpawnKind};

        let mut document = MusaicDocument::new_empty();
        let mut board = TesseraBoard::new();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;
        let mut placed = Vec::new();
        for x in (0..4).step_by(2) {
            execute_command(
                &mut document,
                &mut board,
                &mut attention,
                &mut selection,
                &provenance,
                &EditorCommand::PlaceTile {
                    target: PlacementTarget::BoardSlot {
                        surface: root_surface,
                        slot: BoardSlot::new(x, 0),
                    },
                    tile: TileSpawnKind::Container {
                        kind: ContainerKind::Sequence,
                    },
                },
            )
            .expect("place");
            placed.push(selection.nodes.iter().next().unwrap().clone());
        }
        for node in &placed {
            selection.select(node.clone(), crate::application::editor::SelectionMode::Add);
        }
        assert!(selection.nodes.len() > 1);

        let queries = DocumentQueries::new(&document);
        let layout = derive_inspector_layout(&attention, &selection, &queries, true);
        assert_eq!(layout.panels.len(), 1);
        assert!(matches!(
            layout.panels[0],
            InspectorPanelKind::SelectionSummaryPanel { .. }
        ));
    }

    #[test]
    fn stack_insert_opens_container_drawer_when_visible() {
        let attention = EditorAttention {
            workspace_mode: WorkspaceMode::Compose,
            active_space: ActiveSpace::Board(container_surface()),
            focus: FocusTarget::StackInsert {
                surface: container_surface(),
                index: StackIndex(2),
            },
        };
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        assert_eq!(
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, true),
            InspectorLayout::single(InspectorPanelKind::DrawerPanel {
                target: Some(PlacementTarget::StackIndex {
                    surface: container_surface(),
                    index: StackIndex(2),
                }),
                context: TileLibraryContextKind::ContainerBody,
            })
        );
    }

    #[test]
    fn timeline_event_focus_keeps_active_space() {
        let attention = EditorAttention {
            workspace_mode: WorkspaceMode::Navigate(NavigationMode::Timeline),
            active_space: ActiveSpace::Board(container_surface()),
            focus: FocusTarget::TimelineEvent { event: event(7) },
        };
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        assert_eq!(
            attention.active_space,
            ActiveSpace::Board(container_surface())
        );
        assert_eq!(
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, true),
            InspectorLayout::single(InspectorPanelKind::TimelineEventPanel { event: event(7) })
        );
    }
}
