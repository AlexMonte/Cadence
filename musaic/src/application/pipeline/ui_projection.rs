//! Unified UI read model — one projection after SceneSync, one region dirty set.
//!
//! **Single writer:** [`compute_and_store_ui_projection`] (SceneSync, after
//! `VisibleBoardState`). Region systems in RenderUi / board consume
//! [`UiDirty`] flags; they never recompute inspector layout or invent fingerprints.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use tessera::prelude::NodeId;

use crate::application::board_view_settings::BoardViewSettings;
use crate::application::editor::{
    ConnectionEndpointView, CursorInteraction, CursorInteractionPhase, DrawerPanelState,
    EditorAttention, EditorSession, InspectorLayout, InspectorPanelKind, MinimapPanelState,
    SelectionState, TileDrawerItem, TileLibraryContextKind, TimelinePanelState,
    WorkspaceLayoutKind, basic_tile_options, derive_inspector_layout, drawer_item_short_label,
    inspector_panel_title, inspector_title, minimap_surface_buttons, placement_target_label,
    tile_inspect_description, tile_inspect_title,
};
use crate::application::pipeline::runtime::{
    ProjectedEventId, RuntimePreviewSnapshot, TimelineEventKind,
};
use crate::application::pipeline::scene_sync::{VisibleBoardState, focused_node_from_attention};
use crate::application::session::MusaicProject;
use crate::domain::board::{BoardSlot, BoardSurfaceId, BoardSurfaceKind};
use crate::domain::document::{DocumentQueries, TileSpawnKind};
use crate::infrastructure::diagnostics::DiagnosticStore;

/// Chrome dimensions and readiness that drive shell structure / layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellChrome {
    pub drawer_open: bool,
    pub minimap_open: bool,
    pub minimap_width: f32,
    pub timeline_open: bool,
    pub timeline_height: f32,
    pub ui_sprites_ready: bool,
}

/// Interaction flags that must not force shell/inspector rebuilds alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionChrome {
    pub cursor_phase: CursorInteractionPhase,
    pub placing: bool,
    pub connecting: bool,
}

/// Palette identity — context, armed tile, and option content (not just count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteFingerprint {
    pub context: TileLibraryContextKind,
    pub armed: Option<TileSpawnKind>,
    pub options_hash: u64,
    /// Catalog options for the current drawer context (RenderUi paints these).
    pub options: Vec<TileDrawerItem>,
    /// Short label for the armed tile ("Armed: …"), if any.
    pub armed_label: Option<String>,
}

/// Paint-ready inspector panel payloads (1:1 with [`InspectorLayout::panels`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectorPanelPaint {
    Drawer {
        target_label: Option<String>,
        armed_label: Option<String>,
        panel_title: String,
    },
    PlacementPrompt {
        target_label: Option<String>,
        panel_title: String,
    },
    TileInspect {
        title: String,
        code: Box<crate::application::tricks::TileCodePaint>,
        description: String,
        selected: Box<super::selected_tile::SelectedTilePaint>,
        editable_atom: Option<crate::domain::document::AtomValue>,
        compound:
            Option<crate::application::pipeline::scene_sync::surface_content::OwnedCompoundView>,
        ports: Option<ConnectionEndpointView>,
        sound: Option<Box<crate::application::pipeline::sound::SoundPaint>>,
        panel_title: String,
    },
    SelectionSummary {
        count: usize,
        primary_label: Option<String>,
        panel_title: String,
    },
    TimelineEvent {
        event: ProjectedEventId,
        label: String,
        panel_title: String,
    },
    ProjectOverview {
        channels: u16,
        surface: BoardSurfaceId,
        panel_title: String,
    },
}

/// Inspector chrome strings derived once in the projection writer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InspectorPaint {
    pub header_title: String,
    pub panels: Vec<InspectorPanelPaint>,
}

/// Paint-ready timeline event (no live [`RuntimePreviewSnapshot`] read in RenderUi).
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineEventPaint {
    pub id: ProjectedEventId,
    pub output_id: String,
    pub kind: TimelineEventKind,
    pub visible_start: f64,
    pub visible_end: f64,
    pub label: String,
}

/// The exact preview window and events for the pattern preview band.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimelinePaint {
    pub browsing: bool,
    pub beats_per_cycle: u32,
    pub output_names: std::collections::BTreeMap<String, String>,
    pub pending_cycle: Option<f64>,
    pub window_start: f64,
    pub window_end: f64,
    pub start_count: usize,
    pub control_count: usize,
    pub events: Vec<TimelineEventPaint>,
}

/// Minimap surface navigation buttons.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MinimapPaint {
    pub surface_buttons: Vec<(String, BoardSurfaceId)>,
}

/// Bottom-tab breadcrumb trail (label + surface), derived once in the projection writer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BreadcrumbPaint {
    pub entries: Vec<(String, BoardSurfaceId)>,
}

/// Canonical UI facts for one frame. Diffed to produce [`UiDirty`].
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct EditorUiProjection {
    pub playback_feedback: super::runtime::feedback::PlaybackFeedback,
    pub sample_bank_labels: Vec<String>,
    pub document_revision: u64,
    pub active_surface: Option<BoardSurfaceId>,
    pub layout_kind: WorkspaceLayoutKind,
    pub inspector: InspectorLayout,
    pub inspector_identity: String,
    pub inspector_paint: InspectorPaint,
    pub minimap_paint: MinimapPaint,
    pub breadcrumbs: BreadcrumbPaint,
    pub selection_ids: BTreeSet<NodeId>,
    pub focused_node: Option<NodeId>,
    pub focused_slot: Option<BoardSlot>,
    pub board_occupancy_hash: u64,
    pub connection_hash: u64,
    pub preview_event_ids: BTreeSet<ProjectedEventId>,
    pub timeline_paint: TimelinePaint,
    pub view_settings: BoardViewSettings,
    pub diagnostics_summary: String,
    pub palette: PaletteFingerprint,
    pub chrome: ShellChrome,
    pub interaction: InteractionChrome,
    pub transform_tiles_ready: bool,
}

impl Default for EditorUiProjection {
    fn default() -> Self {
        Self {
            playback_feedback: default(),
            sample_bank_labels: Vec::new(),
            document_revision: 0,
            active_surface: None,
            layout_kind: WorkspaceLayoutKind::ComposeBoardInspector,
            inspector: InspectorLayout::default(),
            inspector_identity: "empty".into(),
            inspector_paint: InspectorPaint {
                header_title: "Context".into(),
                panels: Vec::new(),
            },
            minimap_paint: MinimapPaint::default(),
            breadcrumbs: BreadcrumbPaint::default(),
            selection_ids: BTreeSet::new(),
            focused_node: None,
            focused_slot: None,
            board_occupancy_hash: 0,
            connection_hash: 0,
            preview_event_ids: BTreeSet::new(),
            timeline_paint: TimelinePaint::default(),
            view_settings: BoardViewSettings::default(),
            diagnostics_summary: String::new(),
            palette: PaletteFingerprint {
                context: TileLibraryContextKind::RootBoard,
                armed: None,
                options_hash: 0,
                options: Vec::new(),
                armed_label: None,
            },
            chrome: ShellChrome {
                drawer_open: false,
                minimap_open: false,
                minimap_width: 0.0,
                timeline_open: false,
                timeline_height: 0.0,
                ui_sprites_ready: false,
            },
            interaction: InteractionChrome {
                cursor_phase: CursorInteractionPhase::Idle,
                placing: false,
                connecting: false,
            },
            transform_tiles_ready: false,
        }
    }
}

/// Previous-frame projection for region diffing.
#[derive(Resource, Debug, Clone, Default)]
pub struct LastEditorUiProjection(pub EditorUiProjection);

/// Per-region dirty flags for one frame. Board 3D owns its own keyed reconcile.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiDirty {
    pub shell_structure: bool,
    pub shell_layout: bool,
    pub inspector: bool,
    pub minimap: bool,
    pub timeline: bool,
    pub diagnostics: bool,
    pub palette: bool,
}

impl UiDirty {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn all() -> Self {
        Self {
            shell_structure: true,
            shell_layout: true,
            inspector: true,
            minimap: true,
            timeline: true,
            diagnostics: true,
            palette: true,
        }
    }

    pub fn any(&self) -> bool {
        self.shell_structure
            || self.shell_layout
            || self.inspector
            || self.minimap
            || self.timeline
            || self.diagnostics
            || self.palette
    }
}

/// Inputs needed to build a projection (pure; no Bevy world).
pub struct UiProjectionInputs<'a> {
    pub project: &'a MusaicProject,
    pub attention: &'a EditorAttention,
    pub selection: &'a SelectionState,
    pub session: &'a EditorSession,
    pub cursor: &'a CursorInteraction,
    pub drawer_panel: &'a DrawerPanelState,
    pub minimap_panel: &'a MinimapPanelState,
    pub timeline_panel: &'a TimelinePanelState,
    pub visible: &'a VisibleBoardState,
    pub preview: &'a RuntimePreviewSnapshot,
    pub view_settings: BoardViewSettings,
    pub diagnostics: &'a DiagnosticStore,
    pub transform_tiles_ready: bool,
    pub ui_sprites_ready: bool,
}

pub fn compute_editor_ui_projection(inputs: &UiProjectionInputs<'_>) -> EditorUiProjection {
    let queries = DocumentQueries::new(&inputs.project.document);
    let drawer_open = inputs.drawer_panel.effective_open(inputs.attention);
    let inspector =
        derive_inspector_layout(inputs.attention, inputs.selection, &queries, drawer_open);
    let inspector_identity = inspector_layout_identity(Some(&inspector));
    let active_surface = Some(inputs.attention.active_board());
    let palette_context = inspector
        .drawer_context()
        .unwrap_or(TileLibraryContextKind::RootBoard);
    let mut options = basic_tile_options(palette_context);
    if palette_context == crate::application::editor::TileLibraryContextKind::RootBoard {
        options.extend(
            inputs
                .project
                .document
                .tricks
                .iter()
                .map(
                    |(id, definition)| crate::application::editor::TileDrawerItem {
                        label: definition.name.clone(),
                        spawn: crate::domain::document::TileSpawnKind::TrickInstance {
                            prototype: crate::domain::document::GraphTilePrototypeId(*id),
                        },
                    },
                ),
        );
    }
    let armed = inputs.session.armed_tile().cloned();
    let armed_label = armed.as_ref().map(drawer_item_short_label);
    let inspector_paint = build_inspector_paint(
        inputs.project,
        &inspector,
        &queries,
        inputs.visible,
        inputs.preview,
        &armed_label,
    );
    let timeline_paint = build_timeline_paint(inputs.preview, inputs.project);
    let minimap_paint = MinimapPaint {
        surface_buttons: minimap_surface_buttons(&queries, inputs.attention.active_board()),
    };
    let breadcrumbs = BreadcrumbPaint {
        entries: breadcrumb_entries(&queries, active_surface),
    };

    EditorUiProjection {
        sample_bank_labels: inputs
            .project
            .samples
            .manifest()
            .banks
            .values()
            .flat_map(|bank| {
                bank.variants
                    .iter()
                    .filter_map(|variant| variant.bank.clone())
            })
            .filter(|name| !name.trim().is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        document_revision: inputs.project.document.revision.0,
        active_surface,
        layout_kind: if inputs.timeline_panel.open {
            WorkspaceLayoutKind::TimelineStackedOverBoardInspector
        } else {
            WorkspaceLayoutKind::ComposeBoardInspector
        },
        inspector,
        inspector_identity,
        inspector_paint,
        minimap_paint,
        breadcrumbs,
        selection_ids: inputs.selection.nodes.iter().cloned().collect(),
        focused_node: focused_node_from_attention(inputs.attention),
        focused_slot: focused_slot_from_attention(inputs.attention),
        board_occupancy_hash: hash_board_occupancy(inputs.visible),
        connection_hash: hash_connections(inputs.visible),
        preview_event_ids: inputs.preview.events().map(|e| e.id).collect(),
        timeline_paint,
        view_settings: inputs.view_settings,
        diagnostics_summary: format_diagnostics_summary(inputs.diagnostics),
        playback_feedback: inputs.preview.feedback.clone(),
        palette: PaletteFingerprint {
            context: palette_context,
            armed,
            options_hash: hash_tile_options(&options),
            options,
            armed_label,
        },
        chrome: ShellChrome {
            drawer_open,
            minimap_open: inputs.minimap_panel.open,
            minimap_width: inputs.minimap_panel.visible_width(),
            timeline_open: inputs.timeline_panel.open,
            timeline_height: inputs.timeline_panel.visible_height(),
            ui_sprites_ready: inputs.ui_sprites_ready,
        },
        interaction: InteractionChrome {
            cursor_phase: inputs.cursor.phase(),
            placing: inputs.session.is_placing_from_drawer(),
            connecting: inputs.session.is_connecting(),
        },
        transform_tiles_ready: inputs.transform_tiles_ready,
    }
}

fn build_timeline_paint(
    preview: &RuntimePreviewSnapshot,
    project: &MusaicProject,
) -> TimelinePaint {
    TimelinePaint {
        browsing: preview.browsing,
        beats_per_cycle: project.document.playback.beats_per_cycle,
        output_names: preview
            .events()
            .filter_map(|event| {
                let node = project
                    .document
                    .graph
                    .node(&NodeId(event.output_id.clone()))?;
                let crate::domain::document::DocumentNodeKind::Output(_) = &node.kind else {
                    return None;
                };
                Some((
                    event.output_id.clone(),
                    crate::application::outputs::timeline_name(project, &node.id),
                ))
            })
            .collect(),
        pending_cycle: preview.pending_cycle.map(|cycle| cycle.value()),
        window_start: preview
            .window
            .map(|window| window.start().value())
            .unwrap_or(0.0),
        window_end: preview
            .window
            .map(|window| window.end().value())
            .unwrap_or(1.0),
        start_count: preview.starts().count(),
        control_count: preview.control_updates().count(),
        events: preview
            .events()
            .map(|event| TimelineEventPaint {
                id: event.id,
                output_id: event.output_id.clone(),
                kind: event.kind,
                visible_start: event.visible_start.value(),
                visible_end: event.visible_end.value(),
                label: event.label.clone(),
            })
            .collect(),
    }
}

fn build_inspector_paint(
    project: &MusaicProject,
    layout: &InspectorLayout,
    queries: &DocumentQueries<'_>,
    visible: &VisibleBoardState,
    preview: &RuntimePreviewSnapshot,
    armed_label: &Option<String>,
) -> InspectorPaint {
    let header_title = inspector_title(layout, queries);
    let panels = layout
        .panels
        .iter()
        .map(|panel| {
            let panel_title = inspector_panel_title(panel, queries);
            match panel {
                InspectorPanelKind::DrawerPanel { target, .. } => InspectorPanelPaint::Drawer {
                    target_label: target.as_ref().map(placement_target_label),
                    armed_label: armed_label.clone(),
                    panel_title,
                },
                InspectorPanelKind::PlacementPromptPanel { target, .. } => {
                    InspectorPanelPaint::PlacementPrompt {
                        target_label: target.as_ref().map(placement_target_label),
                        panel_title,
                    }
                }
                InspectorPanelKind::TileInspectPanel { node } => {
                    let ports = visible
                        .nodes
                        .iter()
                        .find(|n| &n.node == node)
                        .and_then(|n| n.ports.clone())
                        .or_else(|| {
                            crate::application::editor::connection_endpoint_view(
                                queries, node, None, None,
                            )
                        });
                    InspectorPanelPaint::TileInspect {
                        title: tile_inspect_title(queries, node),
                        code: Box::new(crate::application::tricks::paint(project, node)),
                        description: tile_inspect_description(queries, node),
                        selected: Box::new(super::selected_tile::selected_tile_paint(queries, node)),
                        editable_atom: match queries.node_kind(node) {
                            Some(crate::domain::document::DocumentNodeKind::Atom(atom)) => Some(atom.atom.clone()),
                            _ => None,
                        },
                        compound: crate::application::pipeline::scene_sync::surface_content::owned_compound_for_node(queries, node),
                        ports,
                        sound: crate::application::pipeline::sound::sound_paint(project, node)
                            .map(Box::new),
                        panel_title,
                    }
                }
                InspectorPanelKind::SelectionSummaryPanel { count, primary } => {
                    InspectorPanelPaint::SelectionSummary {
                        count: *count,
                        primary_label: primary.as_ref().map(|id| crate::application::editor::tile_inspect_title(queries, id)),
                        panel_title,
                    }
                }
                InspectorPanelKind::TimelineEventPanel { event } => {
                    let label = preview
                        .event(*event)
                        .map(|record| record.label.clone())
                        .unwrap_or_default();
                    InspectorPanelPaint::TimelineEvent {
                        event: *event,
                        label,
                        panel_title,
                    }
                }
                InspectorPanelKind::ProjectOverviewPanel { surface } => {
                    InspectorPanelPaint::ProjectOverview {
                        channels: project.document.channels,
                        surface: *surface,
                        panel_title,
                    }
                }
            }
        })
        .collect();
    InspectorPaint {
        header_title,
        panels,
    }
}

/// Diff previous vs next projection into region dirty flags.
pub fn diff_ui_regions(prev: &EditorUiProjection, next: &EditorUiProjection) -> UiDirty {
    let shell_structure = prev.active_surface != next.active_surface
        || prev.chrome.ui_sprites_ready != next.chrome.ui_sprites_ready
        || prev.breadcrumbs != next.breadcrumbs;

    let shell_layout = prev.layout_kind != next.layout_kind
        || prev.chrome.minimap_open != next.chrome.minimap_open
        || prev.chrome.minimap_width != next.chrome.minimap_width
        || prev.chrome.timeline_open != next.chrome.timeline_open
        || prev.chrome.timeline_height != next.chrome.timeline_height;

    let stable_numeric_control = prev.inspector == next.inspector
        && numeric_inspector_paint_matches(&prev.inspector_paint, &next.inspector_paint);
    let inspector = prev.inspector != next.inspector
        || (!stable_numeric_control && prev.inspector_paint != next.inspector_paint)
        || (!stable_numeric_control && prev.document_revision != next.document_revision)
        || prev.focused_node != next.focused_node
        || prev.selection_ids != next.selection_ids
        || (!stable_numeric_control && prev.board_occupancy_hash != next.board_occupancy_hash)
        || prev.chrome.drawer_open != next.chrome.drawer_open
        || prev.transform_tiles_ready != next.transform_tiles_ready
        || prev.view_settings != next.view_settings;

    let minimap = prev.document_revision != next.document_revision
        || prev.active_surface != next.active_surface
        || prev.board_occupancy_hash != next.board_occupancy_hash
        || prev.connection_hash != next.connection_hash
        || prev.focused_slot != next.focused_slot
        || prev.focused_node != next.focused_node
        || prev.minimap_paint != next.minimap_paint;

    let timeline = prev.document_revision != next.document_revision
        || prev.active_surface != next.active_surface
        || prev.preview_event_ids != next.preview_event_ids
        || prev.timeline_paint != next.timeline_paint;

    let diagnostics = prev.diagnostics_summary != next.diagnostics_summary
        || prev.playback_feedback != next.playback_feedback;

    let palette = prev.palette != next.palette;

    UiDirty {
        shell_structure,
        shell_layout,
        inspector,
        minimap,
        timeline,
        diagnostics,
        palette,
    }
}

/// A value update paints the existing slider instead of destroying its pointer capture.
fn numeric_inspector_paint_matches(before: &InspectorPaint, after: &InspectorPaint) -> bool {
    use crate::application::pipeline::scene_sync::surface_content::OwnedTileGroupRole;
    use crate::domain::document::AtomValue;
    fn normalize(paint: &mut InspectorPaint) -> bool {
        let mut numeric = false;
        let mut zero = |atom: &mut AtomValue| match atom {
            AtomValue::Number(_) | AtomValue::Ratio(_) => {
                *atom = AtomValue::Number(0);
                numeric = true;
            }
            AtomValue::Octave(value) => {
                *value = 0;
                numeric = true;
            }
            AtomValue::Modifier(tessera::prelude::AtomModifier::Elongate(value)) => {
                *value = tessera::prelude::Rational::one();
                numeric = true;
            }
            AtomValue::Modifier(tessera::prelude::AtomModifier::Replicate(value)) => {
                *value = 1;
                numeric = true;
            }
            AtomValue::Modifier(tessera::prelude::AtomModifier::Degrade(value)) => {
                *value = Some(tessera::prelude::Rational::new(1, 2));
                numeric = true;
            }
            AtomValue::Modifier(tessera::prelude::AtomModifier::Modulation {
                parameter,
                value,
            }) => {
                *value = tessera::prelude::ModulationParameters::for_parameter(*parameter);
                numeric = true;
            }
            AtomValue::Modifier(modifier)
                if matches!(
                    modifier.parameter_value(),
                    Some(
                        tessera::prelude::FieldValue::Rational { .. }
                            | tessera::prelude::FieldValue::Delay { .. }
                            | tessera::prelude::FieldValue::Reverb { .. }
                            | tessera::prelude::FieldValue::Compressor { .. }
                    )
                ) =>
            {
                if let Some(default) = modifier.parameter_key().and_then(|key| key.spec().default) {
                    if let Ok(normalized) = modifier.with_parameter_value(default) {
                        *modifier = normalized;
                        numeric = true;
                    }
                }
            }
            _ => {}
        };
        for panel in &mut paint.panels {
            if let InspectorPanelPaint::TileInspect {
                editable_atom,
                compound,
                selected,
                ..
            } = panel
            {
                if let Some(atom) = editable_atom {
                    if atom.numeric_rational().is_some()
                        || matches!(
                            atom,
                            AtomValue::Octave(_)
                                | AtomValue::Modifier(
                                    tessera::prelude::AtomModifier::Elongate(_)
                                        | tessera::prelude::AtomModifier::Replicate(_)
                                        | tessera::prelude::AtomModifier::Degrade(_)
                                        | tessera::prelude::AtomModifier::Modulation { .. }
                                )
                        )
                        || matches!(
                            atom.owned_parameter_value(),
                            Some(
                                tessera::prelude::FieldValue::Rational { .. }
                                    | tessera::prelude::FieldValue::Delay { .. }
                                    | tessera::prelude::FieldValue::Reverb { .. }
                                    | tessera::prelude::FieldValue::Compressor { .. }
                            )
                        )
                    {
                        selected.glyph.clear();
                        selected.content = super::scene_sync::TileSurfaceContent::Empty;
                    }
                    zero(atom);
                }
                if let Some(compound) = compound {
                    for group in &mut compound.groups {
                        if matches!(
                            group.role,
                            OwnedTileGroupRole::Octave
                                | OwnedTileGroupRole::Value
                                | OwnedTileGroupRole::Modifier(_)
                                | OwnedTileGroupRole::SoundModifier(_)
                                | OwnedTileGroupRole::RhythmModifier
                        ) {
                            group.label.clear();
                        }
                        if let Some(value) = &mut group.owned_value {
                            zero(&mut value.atom);
                            value.display.clear();
                        }
                    }
                }
            }
        }
        for panel in &mut paint.panels {
            if let InspectorPanelPaint::TileInspect {
                sound: Some(sound), ..
            } = panel
            {
                sound.instrument.rate = tessera::prelude::Rational::one();
                sound.instrument.start = tessera::prelude::Rational::zero();
                sound.instrument.end = tessera::prelude::Rational::one();
                sound.instrument.reverse = false;
                // Keep controls mounted while changing a value; topology changes
                // (adding/removing an effect or send) still rebuild the panel.
                sound.instrument.sound.envelope = None;
                sound.instrument.sound.gain = None;
                if let Some(compressor) = &mut sound.instrument.sound.compressor {
                    *compressor = Default::default();
                }
                if let Some(delay) = &mut sound.instrument.sound.delay {
                    *delay = Default::default();
                }
                if let Some(reverb) = &mut sound.instrument.sound.reverb {
                    *reverb = Default::default();
                }
                for effect in &mut sound.instrument.sound.effects {
                    use crate::domain::instrument::InstrumentEffect as E;
                    match effect {
                        E::LowPass {
                            cutoff_hz,
                            resonance,
                        }
                        | E::HighPass {
                            cutoff_hz,
                            resonance,
                        } => {
                            *cutoff_hz = tessera::prelude::Rational::zero();
                            *resonance = tessera::prelude::Rational::zero();
                        }
                        E::Drive {
                            amount,
                            wet,
                            output_gain,
                        } => {
                            *amount = tessera::prelude::Rational::zero();
                            *wet = tessera::prelude::Rational::zero();
                            *output_gain = tessera::prelude::Rational::zero();
                        }
                    }
                }
                if let Some(sample) = &mut sound.sample_options {
                    sample.options.root_pitch = sample.options.root_pitch.map(|_| 0.0);
                    sample.options.default_gain = Some(1.0);
                }
                numeric = true;
            }
        }
        numeric
    }
    let mut a = before.clone();
    let mut b = after.clone();
    let has_numeric = normalize(&mut a) | normalize(&mut b);
    has_numeric && a == b
}

fn breadcrumb_entries(
    queries: &DocumentQueries<'_>,
    active_surface: Option<BoardSurfaceId>,
) -> Vec<(String, BoardSurfaceId)> {
    let Some(mut surface) = active_surface else {
        return Vec::new();
    };

    let mut labels = Vec::new();

    loop {
        match queries.surface_kind(surface) {
            Some(BoardSurfaceKind::RootBoard) => {
                labels.push(("Pattern board".to_string(), surface));
                break;
            }
            Some(BoardSurfaceKind::ContainerStack { container }) => {
                let container_node = NodeId::new(container.0.clone());
                if let Some(location) = queries.location_of(&container_node) {
                    let label = tile_inspect_title(queries, &container_node);
                    labels.push((label, surface));
                    surface = location.surface;
                } else {
                    labels.push(("Container".to_string(), surface));
                    break;
                }
            }
            None => {
                labels.push(("Unknown".to_string(), surface));
                break;
            }
        }
    }

    labels.reverse();
    labels
}

/// Structural identity of the panel stack for inspector slide animation.
pub fn inspector_layout_identity(layout: Option<&InspectorLayout>) -> String {
    let Some(layout) = layout.filter(|layout| !layout.is_empty()) else {
        return "empty".into();
    };
    layout
        .panels
        .iter()
        .map(|panel| match panel {
            InspectorPanelKind::DrawerPanel { target, context } => {
                format!("drawer:{target:?}:{context:?}")
            }
            InspectorPanelKind::PlacementPromptPanel { target, context } => {
                format!("placement:{target:?}:{context:?}")
            }
            InspectorPanelKind::TileInspectPanel { node } => format!("inspect:{node:?}"),
            InspectorPanelKind::ProjectOverviewPanel { .. } => "overview".into(),
            InspectorPanelKind::SelectionSummaryPanel { count, primary } => {
                format!("selection:{count}:{primary:?}")
            }
            InspectorPanelKind::TimelineEventPanel { event } => format!("timeline:{event:?}"),
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn focused_slot_from_attention(attention: &EditorAttention) -> Option<BoardSlot> {
    match &attention.focus {
        crate::application::editor::FocusTarget::EmptySlot { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn hash_board_occupancy(visible: &VisibleBoardState) -> u64 {
    let mut hasher = DefaultHasher::new();
    for node in &visible.nodes {
        node.node.hash(&mut hasher);
        format!("{:?}", node.address).hash(&mut hasher);
        format!("{:?}", node.kind).hash(&mut hasher);
        node.selected.hash(&mut hasher);
        node.focused.hash(&mut hasher);
        if let Some(display) = node.surface_content.display() {
            display.hash(&mut hasher);
        }
    }
    for compound in &visible.atom_compounds {
        compound.slot.hash(&mut hasher);
        format!("{:?}", compound.compound).hash(&mut hasher);
    }
    for index in &visible.stack_inserts {
        index.hash(&mut hasher);
    }
    for index in &visible.stack_locked_slots {
        index.hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_connections(visible: &VisibleBoardState) -> u64 {
    let mut hasher = DefaultHasher::new();
    for conn in &visible.connections {
        conn.from.hash(&mut hasher);
        conn.to.hash(&mut hasher);
        conn.from_slot.hash(&mut hasher);
        conn.to_slot.hash(&mut hasher);
        format!("{:?}", conn.kind).hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_tile_options(options: &[crate::application::editor::TileDrawerItem]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for item in options {
        format!("{:?}", item.spawn).hash(&mut hasher);
        item.label.hash(&mut hasher);
    }
    hasher.finish()
}

pub fn format_diagnostics_summary(store: &DiagnosticStore) -> String {
    use crate::infrastructure::diagnostics::{
        AppDiagnostic, HostDiagnostic, LoweringDiagnostic, RuntimeDiagnostic, TransactionDiagnostic,
    };
    if store.items.is_empty() {
        return String::new();
    }
    store
        .items
        .iter()
        .map(|item| match &item.diagnostic {
            AppDiagnostic::Tessera(d) => format!("[compile] {}", d.message),
            AppDiagnostic::Host(HostDiagnostic::BoardExportFailed { detail }) => {
                format!("[host] export: {detail}")
            }
            AppDiagnostic::Host(HostDiagnostic::TransactionRejected { message }) => {
                format!("[host] {message}")
            }
            AppDiagnostic::Lowering(LoweringDiagnostic::UnsupportedPatternNode { node }) => {
                format!("[lower] unsupported: {node}")
            }
            AppDiagnostic::Lowering(LoweringDiagnostic::InvalidControlMapping { key }) => {
                format!("[lower] control: {key}")
            }
            AppDiagnostic::Runtime(RuntimeDiagnostic::ProjectionFailed { detail }) => {
                format!("[runtime] {detail}")
            }
            AppDiagnostic::Transaction(TransactionDiagnostic::Info { message }) => message.clone(),
            AppDiagnostic::Transaction(TransactionDiagnostic::Rejected { message }) => {
                format!("[txn] {message}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn reset_ui_projection(
    mut projection: ResMut<EditorUiProjection>,
    mut last: ResMut<LastEditorUiProjection>,
    mut dirty: ResMut<UiDirty>,
) {
    *projection = EditorUiProjection::default();
    *last = LastEditorUiProjection::default();
    *dirty = UiDirty::all();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::pipeline::runtime::TimelineEventKind;
    use crate::application::pipeline::runtime::TimelineEventRecord;
    use crate::application::pipeline::scene_sync::{
        TileSurfaceContent, VisibleBoardNode, VisibleNodeKind,
    };
    use crate::domain::document::PlacementAddress;
    use cadence::prelude::Time as CycleTime;

    fn base_inputs<'a>(
        project: &'a MusaicProject,
        attention: &'a EditorAttention,
        selection: &'a SelectionState,
        session: &'a EditorSession,
        cursor: &'a CursorInteraction,
        drawer: &'a DrawerPanelState,
        minimap: &'a MinimapPanelState,
        timeline: &'a TimelinePanelState,
        visible: &'a VisibleBoardState,
        preview: &'a RuntimePreviewSnapshot,
        diagnostics: &'a DiagnosticStore,
    ) -> UiProjectionInputs<'a> {
        UiProjectionInputs {
            project,
            attention,
            selection,
            session,
            cursor,
            drawer_panel: drawer,
            minimap_panel: minimap,
            timeline_panel: timeline,
            visible,
            preview,
            view_settings: BoardViewSettings::default(),
            diagnostics,
            transform_tiles_ready: true,
            ui_sprites_ready: true,
        }
    }

    fn empty_world() -> (
        MusaicProject,
        EditorAttention,
        SelectionState,
        EditorSession,
        CursorInteraction,
        DrawerPanelState,
        MinimapPanelState,
        TimelinePanelState,
        VisibleBoardState,
        RuntimePreviewSnapshot,
        DiagnosticStore,
    ) {
        let project = MusaicProject::new_empty();
        let attention = EditorAttention::new(project.document.root_surface);
        (
            project,
            attention,
            SelectionState::default(),
            EditorSession::default(),
            CursorInteraction::default(),
            DrawerPanelState::default(),
            MinimapPanelState::default(),
            TimelinePanelState::default(),
            VisibleBoardState::default(),
            RuntimePreviewSnapshot::default(),
            DiagnosticStore::default(),
        )
    }

    #[test]
    fn editing_numeric_value_keeps_the_inspector_control_alive() {
        let mut a = EditorUiProjection::default();
        a.inspector_paint
            .panels
            .push(InspectorPanelPaint::TileInspect {
                code: Default::default(),
                title: "Value".into(),
                selected: Default::default(),
                description: "Edit the connected value".into(),
                editable_atom: Some(crate::domain::document::AtomValue::Number(2)),
                compound: None,
                ports: None,
                sound: None,
                panel_title: "Value".into(),
            });
        let mut b = a.clone();
        b.document_revision += 1;
        b.board_occupancy_hash = a.board_occupancy_hash.wrapping_add(1);
        if let InspectorPanelPaint::TileInspect { editable_atom, .. } =
            &mut b.inspector_paint.panels[0]
        {
            *editable_atom = Some(crate::domain::document::AtomValue::Ratio(
                tessera::prelude::Rational::new(3, 2),
            ));
        }
        let dirty = diff_ui_regions(&a, &b);
        assert!(
            !dirty.inspector,
            "a drag must not replace its own control entity"
        );
        assert!(dirty.timeline, "the musical preview still needs to update");
        b.focused_node = Some(NodeId::new("different"));
        assert!(
            diff_ui_regions(&a, &b).inspector,
            "selecting a different tile replaces the panel"
        );
    }

    #[test]
    fn modulation_values_keep_owned_inspector_mounted_but_target_changes_rebuild() {
        use crate::domain::document::{AtomValue, NoteName, PlacementAddress, TileSpawnKind};
        use tessera::prelude::{
            AtomModifier, ModulationParameters, ModulationWaveform, ParameterKey, Rational,
        };
        let mut project = MusaicProject::new_empty();
        let root = project.document.root_surface;
        let mut modulation = None;
        for (x, atom) in [
            AtomValue::NoteName(NoteName::C),
            AtomValue::Modifier(AtomModifier::Modulation {
                parameter: ParameterKey::Gain,
                value: ModulationParameters::default(),
            }),
        ]
        .into_iter()
        .enumerate()
        {
            modulation = Some(
                project
                    .document
                    .graph
                    .insert_tile(
                        &mut project.document.surfaces,
                        root,
                        PlacementAddress::BoardSlot(BoardSlot::new(x as i32, 0)),
                        TileSpawnKind::Atom { atom },
                    )
                    .unwrap(),
            );
        }
        let node = modulation.unwrap();
        let paint = |project: &MusaicProject| {
            let queries = DocumentQueries::new(&project.document);
            let crate::domain::document::DocumentNodeKind::Atom(atom) =
                queries.node_kind(&node).unwrap()
            else {
                unreachable!()
            };
            let mut projection = EditorUiProjection::default();
            projection
                .inspector_paint
                .panels
                .push(InspectorPanelPaint::TileInspect {
                    code: Default::default(),
                    title: tile_inspect_title(&queries, &node),
                    selected: Box::new(super::super::selected_tile::selected_tile_paint(
                        &queries, &node,
                    )),
                    description: tile_inspect_description(&queries, &node),
                    editable_atom: Some(atom.atom.clone()),
                    compound: super::super::scene_sync::surface_content::owned_compound_for_node(
                        &queries, &node,
                    ),
                    ports: None,
                    sound: None,
                    panel_title: "Modulation".into(),
                });
            projection
        };
        let before = paint(&project);
        let changed = ModulationParameters {
            waveform: ModulationWaveform::SmoothNoise,
            rate: Rational::new(-2, 3),
            phase: Rational::new(5, 7),
            minimum: Rational::new(1, 9),
            maximum: Rational::new(7, 9),
            seed: u64::MAX,
        };
        project
            .document
            .graph
            .set_atom_value(
                &node,
                AtomValue::Modifier(AtomModifier::Modulation {
                    parameter: ParameterKey::Gain,
                    value: changed,
                }),
            )
            .unwrap();
        assert!(
            !diff_ui_regions(&before, &paint(&project)).inspector,
            "waveform, clock, range and seed edits retain the focused controls"
        );
        project
            .document
            .graph
            .set_atom_value(
                &node,
                AtomValue::Modifier(AtomModifier::Modulation {
                    parameter: ParameterKey::LowPassCutoff,
                    value: ModulationParameters::for_parameter(ParameterKey::LowPassCutoff),
                }),
            )
            .unwrap();
        assert!(
            diff_ui_regions(&before, &paint(&project)).inspector,
            "a different target rebuilds its range labels and description"
        );
    }

    #[test]
    fn sound_gestures_keep_controls_alive_but_source_changes_rebuild() {
        use crate::application::pipeline::sound::SoundPaint;
        use crate::domain::instrument::{InstrumentDefinition, InstrumentSource};
        let mut before = EditorUiProjection::default();
        before
            .inspector_paint
            .panels
            .push(InspectorPanelPaint::TileInspect {
                code: Default::default(),
                title: "Output".into(),
                selected: Default::default(),
                description: "Output".into(),
                editable_atom: None,
                compound: None,
                ports: None,
                panel_title: "Output".into(),
                sound: Some(Box::new(SoundPaint {
                    presets: Vec::new(),
                    sample_options: None,
                    instrument: InstrumentDefinition::new(InstrumentSource::Drum("bd".into())),
                    samples: Vec::new(),
                    details: "Built-in percussion".into(),
                    waveform: Vec::new(),
                    diagnostic: None,
                })),
            });
        let mut after = before.clone();
        after.document_revision += 1;
        if let InspectorPanelPaint::TileInspect {
            sound: Some(sound), ..
        } = &mut after.inspector_paint.panels[0]
        {
            sound.instrument.rate = tessera::prelude::Rational::new(3, 2);
            sound.instrument.start = tessera::prelude::Rational::new(1, 4);
            sound.instrument.reverse = true;
        }
        assert!(!diff_ui_regions(&before, &after).inspector);
        assert!(diff_ui_regions(&before, &after).timeline);
        if let InspectorPanelPaint::TileInspect {
            sound: Some(sound), ..
        } = &mut after.inspector_paint.panels[0]
        {
            sound.instrument.source = InstrumentSource::Drum("sd".into());
        }
        assert!(diff_ui_regions(&before, &after).inspector);
    }

    #[test]
    fn owned_numeric_inspector_updates_preserve_role_and_boolean_controls_refresh() {
        use crate::domain::document::{AtomValue, PlacementAddress, TileSpawnKind};
        use tessera::prelude::{AtomModifier as M, Rational};
        let mut project = MusaicProject::new_empty();
        let root = project.document.root_surface;
        let node = project
            .document
            .graph
            .insert_tile(
                &mut project.document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Modifier(M::Legato(Rational::one())),
                },
            )
            .unwrap();
        let paint = |project: &MusaicProject, atom| {
            let queries = DocumentQueries::new(&project.document);
            let mut projection = EditorUiProjection::default();
            projection
                .inspector_paint
                .panels
                .push(InspectorPanelPaint::TileInspect {
                    code: Default::default(),
                    title: tile_inspect_title(&queries, &node),
                    selected: Box::new(
                        crate::application::pipeline::selected_tile::selected_tile_paint(
                            &queries, &node,
                        ),
                    ),
                    description: tile_inspect_description(&queries, &node),
                    editable_atom: Some(atom),
                    compound: None,
                    ports: None,
                    sound: None,
                    panel_title: "Modifier".into(),
                });
            projection
        };
        let a = paint(&project, AtomValue::Modifier(M::Legato(Rational::one())));
        project
            .document
            .graph
            .set_atom_value(&node, AtomValue::Modifier(M::Legato(Rational::new(3, 2))))
            .unwrap();
        let b = paint(
            &project,
            AtomValue::Modifier(M::Legato(Rational::new(3, 2))),
        );
        assert!(
            !diff_ui_regions(&a, &b).inspector,
            "real modifier descriptions must not invalidate a dragging control"
        );
        let c = paint(&project, AtomValue::Modifier(M::Sustain(Rational::one())));
        assert!(
            diff_ui_regions(&a, &c).inspector,
            "changing the modifier role rebuilds the right control"
        );
        let off = paint(&project, AtomValue::Modifier(M::Gate(false)));
        let on = paint(&project, AtomValue::Modifier(M::Gate(true)));
        assert!(
            diff_ui_regions(&off, &on).inspector,
            "the gate button needs its new label and next action"
        );
    }

    #[test]
    fn cursor_only_change_does_not_dirty_shell_or_inspector() {
        let a = EditorUiProjection::default();
        let mut b = a.clone();
        b.interaction.cursor_phase = CursorInteractionPhase::Panning;
        let dirty = diff_ui_regions(&a, &b);
        assert!(!dirty.shell_structure);
        assert!(!dirty.shell_layout);
        assert!(!dirty.inspector);
        assert!(!dirty.minimap);
        assert!(!dirty.timeline);
        assert!(!dirty.diagnostics);
        assert!(!dirty.palette);
    }

    #[test]
    fn diagnostics_change_dirties_diagnostics_only() {
        let a = EditorUiProjection::default();
        let mut b = a.clone();
        b.diagnostics_summary = "[compile] boom".into();
        let dirty = diff_ui_regions(&a, &b);
        assert!(dirty.diagnostics);
        assert!(!dirty.shell_structure);
        assert!(!dirty.inspector);
        assert!(!dirty.minimap);
        assert!(!dirty.timeline);
    }

    #[test]
    fn timeline_same_count_different_ids_dirties_timeline() {
        let a = EditorUiProjection {
            preview_event_ids: BTreeSet::from([ProjectedEventId(1), ProjectedEventId(2)]),
            ..EditorUiProjection::default()
        };
        let b = EditorUiProjection {
            preview_event_ids: BTreeSet::from([ProjectedEventId(3), ProjectedEventId(4)]),
            ..a.clone()
        };
        let dirty = diff_ui_regions(&a, &b);
        assert!(dirty.timeline);
        assert!(!dirty.shell_structure);
    }

    #[test]
    fn selection_membership_change_at_same_size_dirties_inspector() {
        let a = EditorUiProjection {
            selection_ids: BTreeSet::from([NodeId::new("a"), NodeId::new("b")]),
            ..EditorUiProjection::default()
        };
        let b = EditorUiProjection {
            selection_ids: BTreeSet::from([NodeId::new("a"), NodeId::new("c")]),
            ..a.clone()
        };
        let dirty = diff_ui_regions(&a, &b);
        assert!(dirty.inspector);
    }

    #[test]
    fn focused_slot_change_dirties_minimap_not_shell() {
        let a = EditorUiProjection::default();
        let mut b = a.clone();
        b.focused_slot = Some(BoardSlot::new(3, 4));
        let dirty = diff_ui_regions(&a, &b);
        assert!(dirty.minimap);
        assert!(!dirty.shell_structure);
        assert!(!dirty.shell_layout);
    }

    #[test]
    fn surface_change_dirties_shell_structure() {
        let a = EditorUiProjection {
            active_surface: Some(BoardSurfaceId(0)),
            ..EditorUiProjection::default()
        };
        let b = EditorUiProjection {
            active_surface: Some(BoardSurfaceId(1)),
            ..a.clone()
        };
        let dirty = diff_ui_regions(&a, &b);
        assert!(dirty.shell_structure);
        assert!(dirty.minimap);
        assert!(dirty.timeline);
    }

    #[test]
    fn layout_kind_change_resizes_without_destroying_drag_handles() {
        let a = EditorUiProjection::default();
        let mut b = a.clone();
        b.layout_kind = WorkspaceLayoutKind::TimelineStackedOverBoardInspector;
        let dirty = diff_ui_regions(&a, &b);
        assert!(!dirty.shell_structure);
        assert!(dirty.shell_layout);
    }

    #[test]
    fn breadcrumb_change_dirties_shell_structure() {
        let a = EditorUiProjection::default();
        let mut b = a.clone();
        b.breadcrumbs = BreadcrumbPaint {
            entries: vec![("Home".into(), BoardSurfaceId(0))],
        };
        let dirty = diff_ui_regions(&a, &b);
        assert!(dirty.shell_structure);
    }

    #[test]
    fn occupancy_hash_changes_when_node_moves() {
        let mut visible = VisibleBoardState::default();
        visible.nodes.push(VisibleBoardNode {
            node: NodeId::new("n1"),
            address: PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
            tessera_footprint: None,
            kind: VisibleNodeKind::Container,
            selected: false,
            focused: false,
            icon: None,
            atom: None,
            ports: None,
            surface_content: TileSurfaceContent::Empty,
        });
        let h1 = hash_board_occupancy(&visible);
        visible.nodes[0].address = PlacementAddress::BoardSlot(BoardSlot::new(1, 0));
        let h2 = hash_board_occupancy(&visible);
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_projection_includes_preview_event_ids() {
        let (
            project,
            attention,
            selection,
            session,
            cursor,
            drawer,
            minimap,
            timeline,
            visible,
            mut preview,
            diagnostics,
        ) = empty_world();
        preview.insert_test_event(ProjectedEventId(9));
        let proj = compute_editor_ui_projection(&base_inputs(
            &project,
            &attention,
            &selection,
            &session,
            &cursor,
            &drawer,
            &minimap,
            &timeline,
            &visible,
            &preview,
            &diagnostics,
        ));
        assert!(proj.preview_event_ids.contains(&ProjectedEventId(9)));
        assert!(
            proj.timeline_paint
                .events
                .iter()
                .any(|event| event.id == ProjectedEventId(9))
        );
        let _ = TimelineEventRecord {
            source_id: None,
            id: ProjectedEventId(0),
            output_id: String::new(),
            kind: TimelineEventKind::StartVoice,
            visible_start: CycleTime::ZERO,
            visible_end: CycleTime::ZERO,
            label: String::new(),
        };
    }

    #[test]
    fn view_settings_change_dirties_inspector() {
        let a = EditorUiProjection::default();
        let mut b = a.clone();
        b.view_settings.root_board =
            crate::application::board_view_settings::AtomDisplayMode::Stack;
        let dirty = diff_ui_regions(&a, &b);
        assert!(dirty.inspector);
        assert!(!dirty.timeline);
    }
}
