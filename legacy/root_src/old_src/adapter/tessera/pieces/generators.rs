use super::cadence_piece_def_with_tags;
use crate::adapter::tessera::cadence_schema::{pattern_port, pattern_schema};
use tessera::piece::{ParamDef, ParamSchema, ParamTextSemantics, Piece, PieceDef};
use tessera::types::{PieceCategory, TileSide};

pub struct SoundPiece {
    def: PieceDef,
}

impl SoundPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.sound",
                "sound",
                PieceCategory::Generator,
                vec![
                    pattern_input("pattern", false),
                    ParamDef {
                        id: "value".into(),
                        label: "value".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: "".into(),
                            can_inline: true,
                        },
                        text_semantics: ParamTextSemantics::new("cadence_sample_script"),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Sample or builtin waveform source pattern.",
                vec!["s", "sound"],
            ),
        }
    }
}

impl Piece for SoundPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct NotePiece {
    def: PieceDef,
}

impl NotePiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.note",
                "note",
                PieceCategory::Generator,
                vec![
                    pattern_input("pattern", false),
                    ParamDef {
                        id: "value".into(),
                        label: "value".into(),
                        side: TileSide::BOTTOM,
                        schema: ParamSchema::Text {
                            default: "c3".into(),
                            can_inline: true,
                        },
                        text_semantics: ParamTextSemantics::new("cadence_note_script"),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Pitch control pattern for sample playback.",
                vec!["n", "note"],
            ),
        }
    }
}

impl Piece for NotePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

fn pattern_input(id: &str, required: bool) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: id.into(),
        side: TileSide::LEFT,
        schema: pattern_schema(),
        text_semantics: Default::default(),
        variadic_group: None,
        required,
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

impl_default_from_new!(SoundPiece, NotePiece,);

#[cfg(test)]
mod tests {
    use super::{NotePiece, SoundPiece};
    use tessera::piece::Piece;

    #[test]
    fn generator_value_params_are_marked_with_contextual_script_semantics() {
        let sound = SoundPiece::new();
        let sound_value = sound
            .def()
            .params
            .iter()
            .find(|param| param.id == "value")
            .expect("sound value param");
        assert_eq!(sound_value.text_semantics.as_str(), "cadence_sample_script");

        let note = NotePiece::new();
        let note_value = note
            .def()
            .params
            .iter()
            .find(|param| param.id == "value")
            .expect("note value param");
        assert_eq!(note_value.text_semantics.as_str(), "cadence_note_script");
    }
}
