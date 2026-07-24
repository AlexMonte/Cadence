use bevy::prelude::*;

use crate::domain::document::{Accidental, AtomValue, NoteName, OperatorValue};

use super::tile_assets::AtomTileAssets;

impl AtomTileAssets {
    pub fn image_for_atom(&self, value: &AtomValue) -> Option<&Handle<Image>> {
        Some(match value {
            AtomValue::NoteName(NoteName::A) => &self.note_a,
            AtomValue::NoteName(NoteName::B) => &self.note_b,
            AtomValue::NoteName(NoteName::C) => &self.note_c,
            AtomValue::NoteName(NoteName::D) => &self.note_d,
            AtomValue::NoteName(NoteName::E) => &self.note_e,
            AtomValue::NoteName(NoteName::F) => &self.note_f,
            AtomValue::NoteName(NoteName::G) => &self.note_g,
            AtomValue::Rest => &self.note_silence,
            AtomValue::Accidental(Accidental::Sharp) => &self.accidental_sharp,
            AtomValue::Accidental(Accidental::Flat) => &self.accidental_flat,
            AtomValue::Accidental(Accidental::Natural) => &self.note_silence,
            AtomValue::Octave(o) => self.scalar_for_octave(*o)?,
            AtomValue::Number(n) => self.scalar_for_number(*n)?,
            AtomValue::Operator(OperatorValue::Power) => &self.operator_replication,
            AtomValue::Operator(OperatorValue::At) => &self.operator_elongation,
            AtomValue::Operator(OperatorValue::Multiply) => &self.operator_multiply,
            AtomValue::Operator(OperatorValue::Divide) => &self.operator_divide,
        })
    }

    fn scalar_for_octave(&self, octave: i8) -> Option<&Handle<Image>> {
        self.scalar_for_number(octave as i32)
    }

    fn scalar_for_number(&self, n: i32) -> Option<&Handle<Image>> {
        match n {
            0 => Some(&self.scalar_0),
            1 => Some(&self.scalar_1),
            2 => Some(&self.scalar_2),
            3 => Some(&self.scalar_3),
            4 => Some(&self.scalar_4),
            5 => Some(&self.scalar_5),
            6 => Some(&self.scalar_6),
            7 => Some(&self.scalar_7),
            8 => Some(&self.scalar_8),
            9 => Some(&self.scalar_9),
            _ => None,
        }
    }
}
