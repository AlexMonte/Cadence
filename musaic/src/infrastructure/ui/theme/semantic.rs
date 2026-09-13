//! Semantic tile and flow colors from the approved paper/forest/brass editor study.
//! Consumers read colors from [`MusaicUiTheme::semantic`](super::MusaicUiTheme), not a
//! parallel const facade.

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
    /// Shared semantic colors for the compact editor.
    pub const fn artist_default() -> Self {
        Self {
            transform_generic: srgb_hex(0x87, 0x63, 0x1d),
            container: srgb_hex(0x28, 0x6b, 0x4f),
            output: srgb_hex(0x28, 0x6b, 0x4f),
            atom_note: srgb_hex(0x28, 0x6b, 0x4f),
            atom_scalar: srgb_hex(0x87, 0x63, 0x1d),
            atom_operator: srgb_hex(0x87, 0x63, 0x1d),
            atom_accidental: srgb_hex(0x28, 0x6b, 0x4f),
            trick_unset: srgb_hex(0x65, 0x71, 0x6a),
            flow_control: srgb_hex(0x7b, 0x92, 0x84),
            flow_scalar: srgb_hex(0x87, 0x63, 0x1d),
            focus_accent: srgb_hex(0x28, 0x6b, 0x4f),
            focus_halo: Color::srgba(0.16, 0.42, 0.31, 0.18),
            empty_cell: srgb_hex(0xec, 0xee, 0xe7),
        }
    }

    pub fn atom_value_color(self, atom: AtomValue) -> Color {
        match atom {
            AtomValue::NoteName(_) | AtomValue::DrumHit(_) | AtomValue::Rest => self.atom_note,
            AtomValue::Number(_) | AtomValue::Ratio(_) | AtomValue::Octave(_) => self.atom_scalar,
            AtomValue::Operator(_) | AtomValue::Modifier(_) => self.atom_operator,
            AtomValue::Accidental(_) => self.atom_accidental,
        }
    }
}

const fn srgb_hex(r: u8, g: u8, b: u8) -> Color {
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}
