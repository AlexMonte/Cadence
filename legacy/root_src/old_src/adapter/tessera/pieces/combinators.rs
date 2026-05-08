use crate::adapter::tessera::cadence_schema::{pattern_port, pattern_schema};
use tessera::piece::{ParamDef, Piece, PieceDef};
use tessera::types::{PieceCategory, TileSide};

use super::cadence_piece_def_with_tags;

pub struct OverlayPiece {
    def: PieceDef,
}

impl OverlayPiece {
    pub fn new() -> Self {
        Self {
            def: structure_piece(
                "cadence.overlay",
                "overlay",
                "Merge all connected pattern branches in parallel.",
                vec!["stack"],
            ),
        }
    }
}

impl Piece for OverlayPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct LayerPiece {
    def: PieceDef,
}

impl LayerPiece {
    pub fn new() -> Self {
        Self {
            def: structure_piece(
                "cadence.layer",
                "layer",
                "Layer all connected pattern branches in parallel.",
                vec!["layer"],
            ),
        }
    }
}

impl Piece for LayerPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct ArrangePiece {
    def: PieceDef,
}

impl ArrangePiece {
    pub fn new() -> Self {
        Self {
            def: structure_piece(
                "cadence.arrange",
                "arrange",
                "Route connected branches across successive cycles.",
                vec!["arrange"],
            ),
        }
    }
}

impl Piece for ArrangePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PolymeterPiece {
    def: PieceDef,
}

impl PolymeterPiece {
    pub fn new() -> Self {
        Self {
            def: structure_piece(
                "cadence.polymeter",
                "polymeter",
                "Place connected branches into independent cycle slots.",
                vec!["polymeter", "s_polymeter"],
            ),
        }
    }
}

impl Piece for PolymeterPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct SilencePiece {
    def: PieceDef,
}

impl SilencePiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.silence",
                "silence",
                PieceCategory::Generator,
                Vec::new(),
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Emit no events.",
                vec!["silence", "rest"],
            ),
        }
    }
}

impl Piece for SilencePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

fn structure_piece(id: &str, label: &str, description: &str, tags: Vec<&str>) -> PieceDef {
    cadence_piece_def_with_tags(
        id,
        label,
        PieceCategory::Generator,
        variadic_pattern_params(),
        Some(pattern_port()),
        Some(TileSide::RIGHT),
        description,
        tags,
    )
}

fn variadic_pattern_params() -> Vec<ParamDef> {
    vec![
        variadic_pattern_param("in_w", TileSide::LEFT),
        variadic_pattern_param("in_n", TileSide::TOP),
        variadic_pattern_param("in_s", TileSide::BOTTOM),
        variadic_pattern_param("in_e", TileSide::RIGHT),
    ]
}

fn variadic_pattern_param(id: &str, side: TileSide) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: id.into(),
        side,
        schema: pattern_schema(),
        text_semantics: Default::default(),
        variadic_group: Some("patterns".into()),
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
    OverlayPiece,
    LayerPiece,
    ArrangePiece,
    PolymeterPiece,
    SilencePiece,
);
