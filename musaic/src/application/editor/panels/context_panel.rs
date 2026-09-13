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
/// `panels` is ordered bottom-to-top. The tile library occupies the rail alone
/// while open, so long note inspectors cannot push its search and choices off
/// screen. Closing it reveals the unchanged focus and selection.
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

/// Stable, visible library sections; categories describe how a tile is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileLibraryCategory {
    Numbers,
    Notes,
    Patterns,
    Rhythm,
    Sound,
    Samples,
    Effects,
    Modulation,
    ConnectedControls,
    Flow,
}

impl TileLibraryCategory {
    pub const ALL: [Self; 10] = [
        Self::Numbers,
        Self::Notes,
        Self::Patterns,
        Self::Rhythm,
        Self::Sound,
        Self::Samples,
        Self::Effects,
        Self::Modulation,
        Self::ConnectedControls,
        Self::Flow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Numbers => "Numbers",
            Self::Notes => "Notes & accidentals",
            Self::Patterns => "Patterns & tricks",
            Self::Rhythm => "Rhythm & operators",
            Self::Sound => "Sound & drums",
            Self::Samples => "Sample controls",
            Self::Effects => "Effects",
            Self::Modulation => "Modulation",
            Self::ConnectedControls => "Connected controls",
            Self::Flow => "Flow & output",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Numbers => "Drop a number on a note to set its octave.",
            Self::Notes => "Notes, rests, and pitch changes.",
            Self::Patterns => "Group tiles to shape time and layers.",
            Self::Rhythm => "Change repetition, timing, and combinations.",
            Self::Sound => "Choose drum hits, samples, loudness, and tone.",
            Self::Samples => "Choose and reshape recorded sound.",
            Self::Effects => "Attach echo, reverb, and compression.",
            Self::Modulation => "Move a sound control through a range.",
            Self::ConnectedControls => "Connect these tiles to a pattern or control lane.",
            Self::Flow => "Combine, route, and output patterns.",
        }
    }
}

impl TileDrawerItem {
    pub fn category(&self) -> TileLibraryCategory {
        use TileLibraryCategory as C;
        use tessera::prelude::AtomModifier as M;
        match &self.spawn {
            TileSpawnKind::Container { .. } => C::Patterns,
            TileSpawnKind::Output { .. } | TileSpawnKind::FlowControl { .. } => C::Flow,
            TileSpawnKind::Sound { .. } => C::Sound,
            TileSpawnKind::TrickInstance { prototype } if prototype.0 == 32 => C::Flow,
            TileSpawnKind::TrickInstance { prototype } if prototype.0 >= 1000 => C::Patterns,
            TileSpawnKind::Tile { .. } | TileSpawnKind::TrickInstance { .. } => {
                C::ConnectedControls
            }
            TileSpawnKind::Atom { atom } => match atom {
                AtomValue::Number(_) | AtomValue::Ratio(_) => C::Numbers,
                AtomValue::DrumHit(_) => C::Sound,
                AtomValue::NoteName(_)
                | AtomValue::Accidental(_)
                | AtomValue::Octave(_)
                | AtomValue::Rest => C::Notes,
                AtomValue::Operator(_) => C::Rhythm,
                AtomValue::Modifier(modifier) => match modifier {
                    M::Modulation { .. } => C::Modulation,
                    M::Rev
                    | M::Fast(_)
                    | M::Slow(_)
                    | M::Late(_)
                    | M::Elongate(_)
                    | M::Replicate(_)
                    | M::Degrade(_)
                    | M::Euclid { .. }
                    | M::EuclidPattern(_)
                    | M::EuclidRot { .. } => C::Rhythm,
                    M::Scale(_) => C::Notes,
                    M::PlaybackRate(_)
                    | M::PlaybackStart(_)
                    | M::PlaybackEnd(_)
                    | M::Reverse(_)
                    | M::Fit(_)
                    | M::Loop(_)
                    | M::Slice { .. }
                    | M::SampleBank(_)
                    | M::SampleVariant(_) => C::Samples,
                    M::Delay(_) | M::Reverb(_) | M::Compressor(_) => C::Effects,
                    _ => C::Sound,
                },
            },
        }
    }

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
/// An open library takes the rail; otherwise focus and selection choose its content.
pub fn derive_inspector_layout(
    attention: &EditorAttention,
    selection: &crate::application::editor::SelectionState,
    queries: &DocumentQueries<'_>,
    drawer_open: bool,
) -> InspectorLayout {
    if drawer_open {
        let (target, context) =
            drawer_target_for_attention(attention, queries).unwrap_or_else(|| {
                (
                    None,
                    library_context_for_surface(queries, attention.active_board()),
                )
            });
        return InspectorLayout::single(InspectorPanelKind::DrawerPanel { target, context });
    }
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

    match &attention.focus {
        FocusTarget::None => {
            return InspectorLayout::single(InspectorPanelKind::ProjectOverviewPanel {
                surface: attention.active_board(),
            });
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
    let mut items = vec![
        TileDrawerItem::atom("Number", AtomValue::Number(2)),
        TileDrawerItem::container("Sequence", ContainerKind::Sequence),
        TileDrawerItem::container("Arrangement", ContainerKind::Arrangement),
        TileDrawerItem::container("Subdivision", ContainerKind::Subdivision),
        TileDrawerItem::container("Alternating", ContainerKind::Alternating),
        TileDrawerItem::container("Parallel", ContainerKind::Parallel),
        TileDrawerItem {
            label: "Output".into(),
            spawn: TileSpawnKind::Output {
                name: String::new(),
            },
        },
        TileDrawerItem {
            label: "Sound".into(),
            spawn: TileSpawnKind::sound(crate::domain::instrument::InstrumentDefinition::default()),
        },
        TileDrawerItem {
            label: "Fast".into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(1),
            },
        },
        TileDrawerItem {
            label: "Slow".into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(0),
            },
        },
        TileDrawerItem {
            label: "Reverse".into(),
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
    ];
    items.extend(
        [
            ("Attack", 4),
            ("Transpose", 5),
            ("Degrade", 6),
            ("Gate", 7),
            ("Note length", 8),
            ("Sustain", 9),
            ("Cutoff", 10),
            ("Resonance", 11),
            ("Sample variant", 12),
            ("Decay", 13),
            ("Release", 14),
            ("Pan", 15),
            ("High-pass cutoff", 16),
            ("High-pass resonance", 17),
            ("Velocity", 18),
            ("Clip length", 19),
            ("Post-effects gain", 20),
            ("Pitch bend", 21),
            ("Expression", 22),
            ("Sample rate", 23),
            ("Sample start", 24),
            ("Sample end", 25),
            ("Reverse sample", 26),
            ("Fit to note", 27),
            ("Loop sample", 28),
            ("Delay pattern", 29),
            ("Reverb pattern", 30),
            ("Compressor pattern", 31),
            ("Wire", 32),
        ]
        .into_iter()
        .map(|(label, prototype)| TileDrawerItem {
            label: label.into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(prototype),
            },
        }),
    );
    items.extend(
        crate::domain::flow::KINDS
            .into_iter()
            .map(|kind| TileDrawerItem {
                label: crate::domain::flow::label(kind).into(),
                spawn: TileSpawnKind::FlowControl {
                    control: tessera::prelude::FlowControlNode::new(kind),
                },
            }),
    );
    items
}

pub fn atom_tile_drawer_rows() -> Vec<(&'static str, Vec<TileDrawerItem>)> {
    use crate::domain::document::Accidental as DocumentAccidental;
    use tessera::prelude::{AtomModifier as M, Rational};

    vec![
        (
            "Accidentals",
            vec![
                TileDrawerItem::atom("Sharp", AtomValue::Accidental(DocumentAccidental::Sharp)),
                TileDrawerItem::atom("Flat", AtomValue::Accidental(DocumentAccidental::Flat)),
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
                TileDrawerItem::atom("Rest", AtomValue::Rest),
                TileDrawerItem::atom(
                    "Scale degrees",
                    AtomValue::Modifier(M::Scale(Default::default())),
                ),
            ],
        ),
        (
            "Drum hits",
            crate::domain::document::DrumHit::ALL
                .into_iter()
                .map(|hit| {
                    TileDrawerItem::atom(
                        format!("{} ({})", hit.label(), hit.code()),
                        AtomValue::DrumHit(hit),
                    )
                })
                .collect(),
        ),
        (
            "Sound",
            [
                M::Gain(Rational::one()),
                M::Attack(Rational::zero()),
                M::Decay(Rational::zero()),
                M::Release(Rational::zero()),
                M::Transpose(Rational::zero()),
                M::Pan(Rational::zero()),
                M::HighPassCutoff(Rational::one()),
                M::HighPassResonance(Rational::zero()),
                M::Velocity(Rational::one()),
                M::ClipLength(Rational::one()),
                M::PostGain(Rational::one()),
                M::PitchBend(Rational::one()),
                M::Expression(Rational::one()),
                M::Gate(true),
                M::Legato(Rational::one()),
                M::Sustain(Rational::one()),
                M::LowPassCutoff(Rational::one()),
                M::LowPassResonance(Rational::zero()),
                M::SampleVariant(0),
            ]
            .into_iter()
            .map(|modifier| {
                let spec = modifier
                    .parameter_key()
                    .expect("catalog sound modifier")
                    .spec();
                let modifier = modifier
                    .with_parameter_value(spec.default.expect("sound default"))
                    .expect("valid catalog default");
                TileDrawerItem::atom(spec.label, AtomValue::Modifier(modifier))
            })
            .collect(),
        ),
        (
            "Modulation",
            [
                tessera::prelude::ParameterKey::Gain,
                tessera::prelude::ParameterKey::Velocity,
                tessera::prelude::ParameterKey::PlaybackRate,
                tessera::prelude::ParameterKey::LowPassCutoff,
                tessera::prelude::ParameterKey::Transpose,
            ]
            .into_iter()
            .map(|parameter| {
                TileDrawerItem::atom(
                    format!("{} modulation", parameter.spec().label),
                    AtomValue::Modifier(M::Modulation {
                        parameter,
                        value: tessera::prelude::ModulationParameters::for_parameter(parameter),
                    }),
                )
            })
            .collect(),
        ),
        (
            "Sample bank",
            vec![TileDrawerItem::atom(
                "Sample bank",
                AtomValue::Modifier(M::SampleBank("bank".into())),
            )],
        ),
        (
            "Samples",
            [
                M::PlaybackRate(Rational::one()),
                M::PlaybackStart(Rational::zero()),
                M::PlaybackEnd(Rational::one()),
                M::Reverse(false),
                M::Fit(false),
                M::Loop(false),
                M::Slice {
                    index: 0,
                    count: 16,
                },
            ]
            .into_iter()
            .map(|modifier| {
                let spec = modifier.parameter_key().expect("sampler parameter").spec();
                let modifier = modifier
                    .with_parameter_value(spec.default.expect("sampler default"))
                    .expect("valid default");
                TileDrawerItem::atom(spec.label, AtomValue::Modifier(modifier))
            })
            .collect(),
        ),
        (
            "Effects",
            vec![
                TileDrawerItem::atom("Delay", AtomValue::Modifier(M::Delay(Default::default()))),
                TileDrawerItem::atom("Reverb", AtomValue::Modifier(M::Reverb(Default::default()))),
                TileDrawerItem::atom(
                    "Compressor",
                    AtomValue::Modifier(M::Compressor(Default::default())),
                ),
            ],
        ),
        (
            "Rhythm",
            vec![
                TileDrawerItem::atom("Reverse pattern", AtomValue::Modifier(M::Rev)),
                TileDrawerItem::atom(
                    "Timing offset",
                    AtomValue::Modifier(M::Late(Rational::new(1, 200))),
                ),
                TileDrawerItem::atom(
                    "Fast ×2",
                    AtomValue::Modifier(M::Fast(Rational::from_integer(2))),
                ),
                TileDrawerItem::atom(
                    "Slow ÷2",
                    AtomValue::Modifier(M::Slow(Rational::from_integer(2))),
                ),
                TileDrawerItem::atom(
                    "Elongate @2",
                    AtomValue::Modifier(M::Elongate(Rational::from_integer(2))),
                ),
                TileDrawerItem::atom("Repeat 2", AtomValue::Modifier(M::Replicate(2))),
                TileDrawerItem::atom(
                    "Probability 50%",
                    AtomValue::Modifier(M::Degrade(Some(Rational::new(1, 2)))),
                ),
                TileDrawerItem::atom(
                    "Euclid 3 / 8",
                    AtomValue::Modifier(M::Euclid {
                        pulses: 3,
                        steps: 8,
                    }),
                ),
                TileDrawerItem::atom(
                    "Euclid rotation",
                    AtomValue::Modifier(M::EuclidRot {
                        pulses: 3,
                        steps: 8,
                        rotation: 1,
                    }),
                ),
                TileDrawerItem::atom(
                    "Euclid cycle pattern",
                    AtomValue::Modifier(M::EuclidPattern(Default::default())),
                ),
            ],
        ),
        (
            "Numbers",
            (0..=9)
                .map(|digit| TileDrawerItem::atom(digit.to_string(), AtomValue::Number(digit)))
                .collect(),
        ),
        (
            "Operators",
            vec![
                TileDrawerItem::atom("Choices |", AtomValue::Operator(OperatorValue::Choice)),
                TileDrawerItem::atom("Together ,", AtomValue::Operator(OperatorValue::Parallel)),
                TileDrawerItem::atom("Power ^", AtomValue::Operator(OperatorValue::Power)),
                TileDrawerItem::atom("Weight @", AtomValue::Operator(OperatorValue::At)),
                TileDrawerItem::atom("Multiply ×", AtomValue::Operator(OperatorValue::Multiply)),
                TileDrawerItem::atom("Divide ÷", AtomValue::Operator(OperatorValue::Divide)),
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
        } => "Tile library".to_string(),
        InspectorPanelKind::DrawerPanel {
            context: TileLibraryContextKind::ContainerBody,
            ..
        } => "Pattern tile library".to_string(),
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
            ContainerKind::Arrangement => "Arrangement".to_string(),
            ContainerKind::Subdivision => "Subdivision".to_string(),
            ContainerKind::Alternating => "Alternating".to_string(),
            ContainerKind::Parallel => "Parallel".to_string(),
        },
        Some(DocumentNodeKind::Output(output)) => {
            if output.name.trim().is_empty() {
                "Output".into()
            } else {
                format!("Output · {}", output.name)
            }
        }
        Some(DocumentNodeKind::Sound(_)) => "Sound".into(),
        Some(DocumentNodeKind::TrickInstance(trick)) => queries
            .document
            .tricks
            .get(&trick.prototype.0)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| trick_label(trick.prototype)),
        Some(DocumentNodeKind::FlowControl(control)) => {
            crate::domain::flow::label(control.kind).into()
        }
        Some(DocumentNodeKind::Atom(atom)) => match &atom.atom {
            AtomValue::Operator(OperatorValue::Choice) => "Alternate choices".into(),
            AtomValue::Operator(OperatorValue::Parallel) => "Play together".into(),
            AtomValue::Operator(OperatorValue::At) => "Sequence weight".into(),
            AtomValue::Operator(OperatorValue::Multiply) => "Pattern speed".into(),
            AtomValue::Operator(OperatorValue::Divide) => "Pattern speed".into(),
            AtomValue::Modifier(tessera::prelude::AtomModifier::Rev) => "Reverse pattern".into(),
            AtomValue::Modifier(tessera::prelude::AtomModifier::Elongate(_)) => {
                "Duration / weight".into()
            }
            AtomValue::Modifier(tessera::prelude::AtomModifier::Scale(_)) => "Scale degrees".into(),
            AtomValue::Modifier(tessera::prelude::AtomModifier::EuclidPattern(_)) => {
                "Euclid cycle pattern".into()
            }
            AtomValue::Modifier(modifier) => modifier
                .parameter_key()
                .map(|key| key.spec().label.to_string())
                .unwrap_or_else(|| "Modifier".into()),
            AtomValue::NoteName(note) => format!("Note {note:?}"),
            AtomValue::DrumHit(hit) => format!("Drum hit · {}", hit.label()),
            AtomValue::Number(_) | AtomValue::Ratio(_) => "Value".into(),
            AtomValue::Octave(_) => "Octave".into(),
            AtomValue::Accidental(_) => "Accidental".into(),
            _ => "Atom".into(),
        },
        Some(DocumentNodeKind::Tile(_)) => "Transform".to_string(),
        Some(DocumentNodeKind::Arrangement(_)) => "Arrangement".to_string(),
        None => format!("Tile {}", node.0),
    }
}

pub fn tile_inspect_description(queries: &DocumentQueries<'_>, node: &NodeId) -> String {
    match queries.node_kind(node) {
        Some(DocumentNodeKind::Container(container)) => match container.kind {
            ContainerKind::Sequence => "Play child patterns one after another.".to_string(),
            ContainerKind::Arrangement => "Run child patterns in order at their own speed. Follow a child with Elongate to set its duration in cycles; the default is one cycle.".to_string(),
            ContainerKind::Subdivision => "Divide time inside this container.".to_string(),
            ContainerKind::Alternating => "Cycle through child patterns each loop.".to_string(),
            ContainerKind::Parallel => "Play all child patterns together.".to_string(),
        },
        Some(DocumentNodeKind::Output(_)) => {
            "Names this timeline lane. Incoming sound passes through unchanged.".to_string()
        }
        Some(DocumentNodeKind::Sound(_)) => {
            "Chooses the sound played by incoming notes or named drum hits.".into()
        }
        Some(DocumentNodeKind::FlowControl(control)) => crate::domain::flow::description(control),
        Some(DocumentNodeKind::TrickInstance(trick)) if trick.prototype.0 >= 1000 => {
            "Runs reusable tile code. Open its source to edit every use.".into()
        }
        Some(DocumentNodeKind::TrickInstance(trick)) if trick.prototype.0 == 32 => {
            "Carries notes or values unchanged. Use the side controls to turn the wire.".into()
        }
        Some(DocumentNodeKind::TrickInstance(trick)) => {
            format!(
                "{} transform on the signal path.",
                trick_label(trick.prototype)
            )
        }
        Some(DocumentNodeKind::Atom(atom)) => match &atom.atom {
            AtomValue::NoteName(_) => "Choose the pitch below. Each modifier group keeps its own value when moved.".into(),
            AtomValue::DrumHit(_) => "Choose a named one-shot below. Connect the containing pattern to an Instrument set to Kit.".into(),
            AtomValue::Accidental(_) => "Raise or lower this note by a semitone. Moving this tile keeps it attached to the same note.".into(),
            AtomValue::Operator(OperatorValue::Choice) => "Choose a different adjacent note on successive cycles: C | E.".into(),
            AtomValue::Operator(OperatorValue::Parallel) => "Play adjacent notes together: C , E.".into(),
            AtomValue::Operator(OperatorValue::At) => "Set this child's sequence weight, or its duration in cycles inside an Arrangement. The value belongs to this modifier.".into(),
            AtomValue::Modifier(tessera::prelude::AtomModifier::Rev) => "Reverse note order and timing within each cycle of the attached pattern. Recorded samples keep their playback direction.".into(),
            AtomValue::Modifier(tessera::prelude::AtomModifier::Elongate(_)) => "In Arrangement, set how many cycles this child plays. In Sequence, set its relative share of the cycle.".into(),
            AtomValue::Operator(OperatorValue::Multiply) => "Repeat this pattern faster within its allotted time. The value belongs to this modifier.".into(),
            AtomValue::Operator(OperatorValue::Divide) => "Run this pattern more slowly. The value belongs to this modifier.".into(),
            AtomValue::Number(_) | AtomValue::Ratio(_) => "Edit this value to reshape the connected pattern. The preview updates while you drag.".into(),
            AtomValue::Octave(_) => "Choose the pitch register. This tile keeps its octave role when moved.".into(),
            AtomValue::Modifier(modifier) => match modifier {
                tessera::prelude::AtomModifier::Scale(_) => "Choose the key and scale for this number pattern.".into(),
                tessera::prelude::AtomModifier::EuclidPattern(_) => "Change the pulse count, steps and rotation across cycles.".into(),
                tessera::prelude::AtomModifier::Modulation { .. } => "Move a sound control through a range. Velocity samples the range when each note starts. This tile owns its shape, clock, range and noise seed.".into(),
                tessera::prelude::AtomModifier::Gate(_) => "Open or close this note without moving its onset.".into(),
                tessera::prelude::AtomModifier::Legato(_) => "Change how long this note is held. Sequence positions stay the same.".into(),
                tessera::prelude::AtomModifier::Gain(_) => "Multiply the loudness of this note.".into(),
                tessera::prelude::AtomModifier::Attack(_) => "Set the time in seconds for this note to reach full level.".into(),
                tessera::prelude::AtomModifier::Decay(_) => "Set the time in seconds from the peak to the held level.".into(),
                tessera::prelude::AtomModifier::Release(_) => "Set the fade time in seconds after the note ends.".into(),
                tessera::prelude::AtomModifier::Transpose(_) => "Shift this note by a number of semitones.".into(),
                tessera::prelude::AtomModifier::Pan(_) => "Place this note between left (-1), center (0), and right (1).".into(),
                tessera::prelude::AtomModifier::HighPassCutoff(_) => "Remove frequencies below this cutoff in hertz.".into(),
                tessera::prelude::AtomModifier::HighPassResonance(_) => "Emphasize frequencies around the high-pass cutoff.".into(),
                tessera::prelude::AtomModifier::Velocity(_) => "Set note intensity from zero to one at its onset.".into(),
                tessera::prelude::AtomModifier::ClipLength(_) => "Scale sounding duration without moving the next note; zero cuts the note off.".into(),
                tessera::prelude::AtomModifier::PostGain(_) => "Multiply volume after the effects.".into(),
                tessera::prelude::AtomModifier::PitchBend(_) => "Bend pitch: -1 lowers two semitones, 0 is unchanged, and 1 raises two semitones.".into(),
                tessera::prelude::AtomModifier::Expression(_) => "Scale live volume from zero to one.".into(),
                tessera::prelude::AtomModifier::Delay(_) => "Set echo amount, time, feedback and damping. After a note this is a modifier; in a control container each group is a time slot.".into(),
                tessera::prelude::AtomModifier::Reverb(_) => "Set reverb amount, decay and damping. After a note this is a modifier; in a control container each group is a time slot.".into(),
                tessera::prelude::AtomModifier::Compressor(_) => "Set threshold, ratio, knee, attack and release. After a note this is a modifier; in a control container each group is a time slot.".into(),
                tessera::prelude::AtomModifier::Sustain(_) => "Set the held envelope level after the attack and decay.".into(),
                tessera::prelude::AtomModifier::LowPassCutoff(_) => "Set the low-pass filter cutoff for this note.".into(),
                tessera::prelude::AtomModifier::LowPassResonance(_) => "Emphasize frequencies around this note's filter cutoff.".into(),
                tessera::prelude::AtomModifier::SampleBank(_) => "Choose the sample bank for this note's assigned sample instrument.".into(),
                tessera::prelude::AtomModifier::SampleVariant(_) => "Choose a variation from this note's sample bank.".into(),
                _ => "Applies to the preceding note or pattern. Its value moves with it.".into(),
            },
            _ => format!("{:?}", atom.atom),
        },
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
    crate::domain::transform::transform_kind_from_prototype(prototype)
        .map(|kind| {
            kind.parameter_key()
                .map(|key| key.spec().label.to_string())
                .unwrap_or_else(|| format!("{kind:?}"))
        })
        .unwrap_or_else(|| format!("Trick {}", prototype.0))
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
    let mut items = match context {
        TileLibraryContextKind::RootBoard => labeled_tile_drawer_items(),
        TileLibraryContextKind::ContainerBody => atom_tile_drawer_rows()
            .into_iter()
            .flat_map(|(_, items)| items)
            .collect(),
    };
    // Numeric values are first-class inputs on either surface. Keep one entry
    // per authored spawn kind, including the existing default Number tile.
    for digit in 0..=9 {
        let item = TileDrawerItem::atom(digit.to_string(), AtomValue::Number(digit));
        if !items.iter().any(|existing| existing.spawn == item.spawn) {
            items.push(item);
        }
    }
    if context == TileLibraryContextKind::ContainerBody {
        for (label, kind) in [
            ("Sequence", ContainerKind::Sequence),
            ("Arrangement", ContainerKind::Arrangement),
            ("Subdivision", ContainerKind::Subdivision),
            ("Alternating", ContainerKind::Alternating),
            ("Parallel", ContainerKind::Parallel),
        ] {
            items.push(TileDrawerItem::container(label, kind));
        }
    }
    items
}

pub fn placement_target_label(target: &PlacementTarget) -> String {
    match target {
        PlacementTarget::BoardSlot { slot, .. } => {
            format!("Pattern board · ({}, {})", slot.x, slot.y)
        }
        PlacementTarget::StackIndex { .. } => "Add inside this pattern".into(),
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
    fn opening_library_replaces_long_inspector_without_changing_selection() {
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
            InspectorLayout::single(InspectorPanelKind::DrawerPanel {
                target: None,
                context: TileLibraryContextKind::RootBoard,
            })
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
    fn closing_library_restores_the_focused_tile_inspector() {
        let attention = compose_attention(FocusTarget::Tile { node: node("tile") });
        let document = document_with_surfaces();
        let queries = DocumentQueries::new(&document);
        let layout =
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, true);
        assert!(matches!(
            layout.top(),
            Some(InspectorPanelKind::DrawerPanel { .. })
        ));
        let closed =
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, false);
        assert_eq!(
            closed.top(),
            Some(&InspectorPanelKind::TileInspectPanel { node: node("tile") })
        );
    }

    #[test]
    fn multi_selection_replaces_stack_with_selection_summary() {
        use crate::application::command::{EditorCommand, execute_command};
        use crate::application::editor::transaction::PlacementTarget;
        use crate::application::pipeline::runtime::TimelineProvenanceStore;
        use crate::domain::board::BoardSlot;
        use crate::domain::document::{ContainerKind, TileSpawnKind};

        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;
        let mut placed = Vec::new();
        for x in (0..10).step_by(5) {
            execute_command(
                &mut document,
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
        let opened = derive_inspector_layout(&attention, &selection, &queries, true);
        assert!(matches!(
            opened.top(),
            Some(InspectorPanelKind::DrawerPanel { .. })
        ));
        let layout = derive_inspector_layout(&attention, &selection, &queries, false);
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
            derive_inspector_layout(&attention, &SelectionState::default(), &queries, false),
            InspectorLayout::single(InspectorPanelKind::TimelineEventPanel { event: event(7) })
        );
    }
}
