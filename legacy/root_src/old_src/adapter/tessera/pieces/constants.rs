use std::collections::BTreeMap;

use tessera::piece::{ParamDef, ParamSchema, Piece, PieceDef};
use tessera::types::{PieceCategory, PortType, TileSide};

use crate::adapter::tessera::cadence_schema::{pattern_port, pattern_schema};
use crate::adapter::tessera::pieces::cadence_piece_def_with_tags;

pub struct PatternAtomNotePiece {
    def: PieceDef,
}

impl PatternAtomNotePiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.atom.note",
                "note atom",
                PieceCategory::Constant,
                Vec::new(),
                Some(PortType::new("pattern")),
                Some(TileSide::TOP),
                "First-class Tessera note atom for container-local pitch structure.",
                vec!["pattern", "atom", "note", "tessera"],
            ),
        }
    }
}

impl Piece for PatternAtomNotePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternAtomScalarPiece {
    def: PieceDef,
}

impl PatternAtomScalarPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.atom.scalar",
                "scalar atom",
                PieceCategory::Constant,
                Vec::new(),
                Some(PortType::new("pattern")),
                Some(TileSide::TOP),
                "First-class Tessera scalar atom for container-local numeric structure.",
                vec!["pattern", "atom", "scalar", "number", "tessera"],
            ),
        }
    }
}

impl Piece for PatternAtomScalarPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternAtomRestPiece {
    def: PieceDef,
}

impl PatternAtomRestPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.atom.rest",
                "rest atom",
                PieceCategory::Constant,
                Vec::new(),
                Some(PortType::new("pattern")),
                Some(TileSide::TOP),
                "First-class Tessera rest atom for container-local silence.",
                vec!["pattern", "atom", "rest", "silence", "tessera"],
            ),
        }
    }
}

impl Piece for PatternAtomRestPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternAtomElongationPiece {
    def: PieceDef,
}

impl PatternAtomElongationPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.atom.operator.elongation",
                "elongation op",
                PieceCategory::Constant,
                Vec::new(),
                Some(PortType::new("pattern")),
                Some(TileSide::TOP),
                "First-class Tessera elongation operator atom for local duration weighting.",
                vec!["pattern", "atom", "operator", "elongation", "tessera"],
            ),
        }
    }
}

impl Piece for PatternAtomElongationPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternAtomPitchShiftPiece {
    def: PieceDef,
}

impl PatternAtomPitchShiftPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.atom.operator.pitch_shift",
                "pitch shift op",
                PieceCategory::Constant,
                Vec::new(),
                Some(PortType::new("pattern")),
                Some(TileSide::TOP),
                "First-class Tessera pitch-shift operator atom for local pitch modulation.",
                vec!["pattern", "atom", "operator", "pitch_shift", "tessera"],
            ),
        }
    }
}

impl Piece for PatternAtomPitchShiftPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternAtomSlowPiece {
    def: PieceDef,
}

impl PatternAtomSlowPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.atom.operator.slow",
                "slow op",
                PieceCategory::Constant,
                Vec::new(),
                Some(PortType::new("pattern")),
                Some(TileSide::TOP),
                "First-class Tessera slow operator atom for local tempo reduction.",
                vec!["pattern", "atom", "operator", "slow", "tessera"],
            ),
        }
    }
}

impl Piece for PatternAtomSlowPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternAtomFastPiece {
    def: PieceDef,
}

impl PatternAtomFastPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.atom.operator.fast",
                "fast op",
                PieceCategory::Constant,
                Vec::new(),
                Some(PortType::new("pattern")),
                Some(TileSide::TOP),
                "First-class Tessera fast operator atom for local tempo acceleration.",
                vec!["pattern", "atom", "operator", "fast", "tessera"],
            ),
        }
    }
}

impl Piece for PatternAtomFastPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternContainerBasicPiece {
    def: PieceDef,
}

impl PatternContainerBasicPiece {
    pub fn new() -> Self {
        Self {
            def: pattern_container_piece(
                "cadence.container.basic",
                "\"\" pattern",
                "\"\" regular container. Owns one cycle and chains consecutive cycles through its pattern input.",
                vec!["pattern", "container", "regular", "\"\""],
            ),
        }
    }
}

impl Piece for PatternContainerBasicPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternContainerSubdividePiece {
    def: PieceDef,
}

impl PatternContainerSubdividePiece {
    pub fn new() -> Self {
        Self {
            def: pattern_container_piece(
                "cadence.container.subdivide",
                "[] pattern",
                "[] subdivide container. Owns one cycle and subdivides its local slots equally.",
                vec!["pattern", "container", "subdivide", "[]"],
            ),
        }
    }
}

impl Piece for PatternContainerSubdividePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternContainerAlternatePiece {
    def: PieceDef,
}

impl PatternContainerAlternatePiece {
    pub fn new() -> Self {
        Self {
            def: pattern_container_piece(
                "cadence.container.alternate",
                "<> pattern",
                "<> alternate container. Owns one cycle and selects one branch per cycle.",
                vec!["pattern", "container", "alternate", "<>"],
            ),
        }
    }
}

impl Piece for PatternContainerAlternatePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PatternContainerParallelPiece {
    def: PieceDef,
}

impl PatternContainerParallelPiece {
    pub fn new() -> Self {
        Self {
            def: pattern_container_piece(
                "cadence.container.parallel",
                "|| pattern",
                "|| parallel container. Owns one cycle and layers its local slots simultaneously.",
                vec!["pattern", "container", "parallel", "stack", "||"],
            ),
        }
    }
}

impl Piece for PatternContainerParallelPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct ControlInputPiece {
    def: PieceDef,
}

impl ControlInputPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.control_input",
                "control input",
                PieceCategory::Constant,
                vec![
                    ParamDef {
                        id: "value".into(),
                        label: "value".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: "0".into(),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                    ParamDef {
                        id: "widget".into(),
                        label: "widget".into(),
                        side: TileSide::RIGHT,
                        schema: ParamSchema::Enum {
                            options: vec![
                                "dial".into(),
                                "slider".into(),
                                "text".into(),
                                "toggle".into(),
                                "select".into(),
                                "sample_select".into(),
                                "bank_select".into(),
                            ],
                            default: "dial".into(),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                    ParamDef {
                        id: "options".into(),
                        label: "options".into(),
                        side: TileSide::TOP,
                        schema: ParamSchema::Text {
                            default: "".into(),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                    ParamDef {
                        id: "default".into(),
                        label: "default".into(),
                        side: TileSide::LEFT,
                        schema: ParamSchema::Text {
                            default: "".into(),
                            can_inline: true,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                ],
                Some(PortType::any()),
                Some(TileSide::TOP),
                "Primary control authoring tile for scalar, toggle, select, and catalog-driven controls.",
                vec!["control", "input", "dial", "slider", "toggle", "select"],
            ),
        }
    }
}

impl Piece for ControlInputPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn infer_output_type(
        &self,
        _input_types: &BTreeMap<String, PortType>,
        inline_params: &BTreeMap<String, serde_json::Value>,
    ) -> Option<PortType> {
        let widget = inline_params
            .get("widget")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("dial");
        Some(match widget {
            "toggle" => PortType::bool(),
            "select" | "sample_select" | "bank_select" => PortType::text(),
            "text" => PortType::text(),
            _ => PortType::number(),
        })
    }
}

fn pattern_container_piece(id: &str, label: &str, description: &str, tags: Vec<&str>) -> PieceDef {
    cadence_piece_def_with_tags(
        id,
        label,
        PieceCategory::Constant,
        vec![pattern_input_param()],
        Some(pattern_port()),
        Some(TileSide::TOP),
        description,
        tags,
    )
}

fn pattern_input_param() -> ParamDef {
    ParamDef {
        id: "pattern".into(),
        label: "pattern".into(),
        side: TileSide::LEFT,
        schema: pattern_schema(),
        text_semantics: Default::default(),
        variadic_group: None,
        required: false,
        role: Default::default(),
    }
}

macro_rules! impl_default_from_new {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Default for $ty {
                fn default() -> Self {
                    Self::new()
                }
            }
        )*
    };
}

impl_default_from_new!(
    PatternAtomNotePiece,
    PatternAtomScalarPiece,
    PatternAtomRestPiece,
    PatternAtomElongationPiece,
    PatternAtomPitchShiftPiece,
    PatternAtomSlowPiece,
    PatternAtomFastPiece,
    PatternContainerBasicPiece,
    PatternContainerSubdividePiece,
    PatternContainerAlternatePiece,
    PatternContainerParallelPiece,
    ControlInputPiece
);
