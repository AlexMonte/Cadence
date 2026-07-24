//! Semantic tile / flow colors sampled from `assets/palette/artist_color.png` (16×4 grid).
//!
//! Grid indices below are **0-based** (column, row). Row 0 is the top, brightest row.

use bevy::prelude::*;

use crate::domain::document::AtomValue;

/// Semantic colors for board tiles, minimap cells, and timeline events.
#[derive(Debug, Clone, Copy)]
pub struct SemanticColors {
    pub transform_generic: Color,
    pub container: Color,
    pub output: Color,
    pub atom_note: Color,
    pub atom_scalar: Color,
    pub atom_operator: Color,
    pub atom_accidental: Color,
    pub trick_unset: Color,
    pub flow_control: Color,
    pub flow_scalar: Color,
    pub focus_accent: Color,
    pub focus_halo: Color,
    pub empty_cell: Color,
}

impl SemanticColors {
    /// Colors sampled from the artist palette PNG.
    pub const fn artist_default() -> Self {
        Self {
            // grid (1, 1) `#353537`
            transform_generic: srgb_hex(0x35, 0x35, 0x37),
            // grid (5, 1) `#3DBB7E`
            container: srgb_hex(0x3d, 0xbb, 0x7e),
            // grid (3, 1) `#EF3B2A`
            output: srgb_hex(0xef, 0x3b, 0x2a),
            // grid (8, 1) `#CC8531`
            atom_note: srgb_hex(0xcc, 0x85, 0x31),
            // grid (11, 1) `#50A0C8`
            atom_scalar: srgb_hex(0x50, 0xa0, 0xc8),
            // grid (5, 0) `#63E97D`
            atom_operator: srgb_hex(0x63, 0xe9, 0x7d),
            // grid (14, 1) `#B4359A`
            atom_accidental: srgb_hex(0xb4, 0x35, 0x9a),
            // grid (0, 0) `#898D97` — placeholder until a trick slot is chosen
            trick_unset: srgb_hex(0x89, 0x8d, 0x97),
            // grid (13, 1) `#8132B6`
            flow_control: srgb_hex(0x81, 0x32, 0xb6),
            flow_scalar: srgb_hex(0x50, 0xa0, 0xc8),
            // grid (11, 0) `#88D5E6`
            focus_accent: srgb_hex(0x88, 0xd5, 0xe6),
            focus_halo: Color::srgba(0.53, 0.84, 0.90, 0.35),
            empty_cell: Color::srgba(0.10, 0.11, 0.14, 0.85),
        }
    }

    pub fn atom_value_color(self, atom: AtomValue) -> Color {
        match atom {
            AtomValue::NoteName(_) | AtomValue::Rest => self.atom_note,
            AtomValue::Number(_) | AtomValue::Octave(_) => self.atom_scalar,
            AtomValue::Operator(_) => self.atom_operator,
            AtomValue::Accidental(_) => self.atom_accidental,
        }
    }
}

/// Backward-compatible const accessors used by board materials and paint helpers.
pub struct ArtistPalette;

impl ArtistPalette {
    pub const TRANSFORM_GENERIC: Color = SemanticColors::artist_default().transform_generic;
    pub const CONTAINER: Color = SemanticColors::artist_default().container;
    pub const OUTPUT: Color = SemanticColors::artist_default().output;
    pub const ATOM_NOTE: Color = SemanticColors::artist_default().atom_note;
    pub const ATOM_SCALAR: Color = SemanticColors::artist_default().atom_scalar;
    pub const ATOM_OPERATOR: Color = SemanticColors::artist_default().atom_operator;
    pub const ATOM_ACCIDENTAL: Color = SemanticColors::artist_default().atom_accidental;
    pub const TRICK_UNSET: Color = SemanticColors::artist_default().trick_unset;
    pub const FLOW_CONTROL: Color = SemanticColors::artist_default().flow_control;
    pub const FLOW_SCALAR: Color = SemanticColors::artist_default().flow_scalar;
    pub const FOCUS_ACCENT: Color = SemanticColors::artist_default().focus_accent;
    pub const FOCUS_HALO: Color = SemanticColors::artist_default().focus_halo;
    pub const EMPTY_CELL: Color = SemanticColors::artist_default().empty_cell;
}

pub fn atom_value_color(atom: AtomValue) -> Color {
    SemanticColors::artist_default().atom_value_color(atom)
}

const fn srgb_hex(r: u8, g: u8, b: u8) -> Color {
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}
