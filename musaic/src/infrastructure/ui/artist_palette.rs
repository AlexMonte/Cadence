//! Canonical UI colors sampled from `assets/palette/artist_color.png` (16×4 grid).
//!
//! Grid indices below are **0-based** (column, row). Row 0 is the top, brightest row.

use bevy::prelude::*;

use crate::domain::document::AtomValue;

/// Shared Musaic / Tessera tile and flow colors.
pub struct ArtistPalette;

impl ArtistPalette {
    /// Transform / generic tile — grid (1, 1) `#353537`
    pub const TRANSFORM_GENERIC: Color = srgb_hex(0x35, 0x35, 0x37);
    /// Container tile — grid (5, 1) `#3DBB7E`
    pub const CONTAINER: Color = srgb_hex(0x3d, 0xbb, 0x7e);
    /// Output tile — grid (3, 1) `#EF3B2A`
    pub const OUTPUT: Color = srgb_hex(0xef, 0x3b, 0x2a);
    /// Atom: note names & rest — grid (8, 1) `#CC8531`
    pub const ATOM_NOTE: Color = srgb_hex(0xcc, 0x85, 0x31);
    /// Atom: scalar values (number, octave) — grid (11, 1) `#50A0C8`
    pub const ATOM_SCALAR: Color = srgb_hex(0x50, 0xa0, 0xc8);
    /// Atom: operators — grid (5, 0) `#63E97D`
    pub const ATOM_OPERATOR: Color = srgb_hex(0x63, 0xe9, 0x7d);
    /// Atom: flat / sharp / natural — grid (14, 1) `#B4359A`
    pub const ATOM_ACCIDENTAL: Color = srgb_hex(0xb4, 0x35, 0x9a);
    /// Trick tiles — no palette slot chosen yet; grid (0, 0) `#898D97` as neutral placeholder.
    pub const TRICK_UNSET: Color = srgb_hex(0x89, 0x8d, 0x97);

    /// Valid control / composition flow — grid (13, 1) `#8132B6`
    pub const FLOW_CONTROL: Color = srgb_hex(0x81, 0x32, 0xb6);
    /// Scalar value flow — grid (11, 1) `#50A0C8`
    pub const FLOW_SCALAR: Color = Self::ATOM_SCALAR;

    /// Focus ring on minimap cells — grid (11, 0) `#88D5E6`
    pub const FOCUS_ACCENT: Color = srgb_hex(0x88, 0xd5, 0xe6);
    pub const FOCUS_HALO: Color = Color::srgba(0.53, 0.84, 0.90, 0.35);
    pub const EMPTY_CELL: Color = Color::srgba(0.10, 0.11, 0.14, 0.85);
}

const fn srgb_hex(r: u8, g: u8, b: u8) -> Color {
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

pub fn atom_value_color(atom: AtomValue) -> Color {
    match atom {
        AtomValue::NoteName(_) | AtomValue::Rest => ArtistPalette::ATOM_NOTE,
        AtomValue::Number(_) | AtomValue::Octave(_) => ArtistPalette::ATOM_SCALAR,
        AtomValue::Operator(_) => ArtistPalette::ATOM_OPERATOR,
        AtomValue::Accidental(_) => ArtistPalette::ATOM_ACCIDENTAL,
    }
}
