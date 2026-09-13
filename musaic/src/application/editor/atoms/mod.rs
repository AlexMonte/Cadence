use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomView {
    pub node: NodeId,
    pub kind: AtomKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomKind {
    NoteName(NoteName),
    DrumHit(crate::domain::document::DrumHit),
    Octave(i8),
    Accidental(Accidental),
    Operator(OperatorKind),
    Number(i32),
    Ratio(tessera::prelude::Rational),
    Modifier(tessera::prelude::AtomModifier),
    Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteName {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accidental {
    Sharp,
    Flat,
    Natural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorKind {
    Choice,
    Parallel,
    Power,
    At,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomCompoundView {
    pub members: Vec<NodeId>,
    pub display: String,
    pub semantic: AtomCompoundSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomCompoundSemantic {
    Pitch,
    OperatorArgument,
    Single,
}

pub fn compound_atoms(atoms: &[AtomView]) -> Vec<AtomCompoundView> {
    let mut compounds = Vec::new();
    let mut index = 0;

    while index < atoms.len() {
        let current = &atoms[index];
        match current.kind {
            AtomKind::NoteName(note) => {
                let (compound, next_index) =
                    compound_pitch(atoms, index, current.node.clone(), note);
                compounds.push(compound);
                index = next_index;
            }
            AtomKind::Operator(operator) => {
                let (compound, next_index) =
                    compound_operator_argument(atoms, index, current.node.clone(), operator);
                compounds.push(compound);
                index = next_index;
            }
            _ => {
                compounds.push(AtomCompoundView {
                    members: vec![current.node.clone()],
                    display: atom_display(&current.kind),
                    semantic: AtomCompoundSemantic::Single,
                });
                index += 1;
            }
        }
    }

    compounds
}

fn compound_pitch(
    atoms: &[AtomView],
    start_index: usize,
    node: NodeId,
    note: NoteName,
) -> (AtomCompoundView, usize) {
    let mut members = vec![node];
    let mut display = note_display(note).to_string();
    let mut next = start_index + 1;

    if let Some(atom) = atoms.get(next) {
        if let AtomKind::Accidental(accidental) = atom.kind {
            members.push(atom.node.clone());
            display.push_str(accidental_display(accidental));
            next += 1;
        }
    }

    if let Some(atom) = atoms.get(next) {
        if let AtomKind::Octave(octave) = atom.kind {
            members.push(atom.node.clone());
            display.push_str(&octave.to_string());
            next += 1;
        }
    }

    (
        AtomCompoundView {
            members,
            display,
            semantic: AtomCompoundSemantic::Pitch,
        },
        next,
    )
}

fn compound_operator_argument(
    atoms: &[AtomView],
    start_index: usize,
    node: NodeId,
    operator: OperatorKind,
) -> (AtomCompoundView, usize) {
    let mut members = vec![node];
    let mut display = operator_display(operator).to_string();
    let mut next = start_index + 1;

    if let Some(atom) = atoms.get(next) {
        if !matches!(operator, OperatorKind::Choice | OperatorKind::Parallel)
            && matches!(atom.kind, AtomKind::Number(_) | AtomKind::Ratio(_))
        {
            members.push(atom.node.clone());
            display.push_str(&atom_display(&atom.kind));
            next += 1;
        }
    }

    (
        AtomCompoundView {
            members,
            display,
            semantic: AtomCompoundSemantic::OperatorArgument,
        },
        next,
    )
}

fn note_display(note: NoteName) -> &'static str {
    match note {
        NoteName::A => "A",
        NoteName::B => "B",
        NoteName::C => "C",
        NoteName::D => "D",
        NoteName::E => "E",
        NoteName::F => "F",
        NoteName::G => "G",
    }
}

fn accidental_display(accidental: Accidental) -> &'static str {
    match accidental {
        Accidental::Sharp => "♯",
        Accidental::Flat => "♭",
        Accidental::Natural => "♮",
    }
}

fn operator_display(operator: OperatorKind) -> &'static str {
    match operator {
        OperatorKind::Power => "^",
        OperatorKind::Choice => "|",
        OperatorKind::Parallel => ",",
        OperatorKind::At => "@",
        OperatorKind::Multiply => "*",
        OperatorKind::Divide => "/",
    }
}

fn atom_display(kind: &AtomKind) -> String {
    match kind {
        AtomKind::NoteName(note) => note_display(*note).to_string(),
        AtomKind::DrumHit(hit) => hit.code().to_string(),
        AtomKind::Octave(octave) => octave.to_string(),
        AtomKind::Accidental(accidental) => accidental_display(*accidental).to_string(),
        AtomKind::Operator(operator) => operator_display(*operator).to_string(),
        AtomKind::Number(value) => value.to_string(),
        AtomKind::Ratio(value) => format!("{}/{}", value.numerator, value.denominator),
        AtomKind::Modifier(modifier) => modifier_display(modifier),
        AtomKind::Rest => "rest".to_string(),
    }
}

/// Compact glyphs used by both the board and its nested previews. The full
/// parameter name and unit remain available in the inspector.
pub(crate) fn modifier_display(modifier: &tessera::prelude::AtomModifier) -> String {
    use tessera::prelude::AtomModifier as M;
    match modifier {
        M::Modulation { parameter, .. } => format!(
            "~{}",
            match parameter {
                tessera::prelude::ParameterKey::Gain => "G",
                tessera::prelude::ParameterKey::Velocity => "V",
                tessera::prelude::ParameterKey::PlaybackRate => "R",
                tessera::prelude::ParameterKey::LowPassCutoff => "Hz",
                tessera::prelude::ParameterKey::Transpose => "Tr",
                _ => "?",
            }
        ),
        M::Gain(value) => format!(
            "G{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Attack(value) => format!(
            "A{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Decay(value) => format!(
            "D{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Release(value) => format!(
            "R{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Transpose(value) => format!(
            "Tr{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Pan(value) => format!(
            "Pan{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::HighPassCutoff(value) => format!(
            "HP{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::HighPassResonance(value) => format!(
            "HQ{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Velocity(value) => format!(
            "Vel{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::ClipLength(value) => format!(
            "Cl{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::PostGain(value) => format!(
            "PG{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::PitchBend(value) => format!(
            "PB{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Expression(value) => format!(
            "Ex{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Delay(_) => "Dly".into(),
        M::Reverb(_) => "Revb".into(),
        M::Compressor(_) => "Cmp".into(),
        M::Gate(value) => format!("Gate{}", u8::from(*value)),
        M::Legato(value) => format!(
            "L{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Sustain(value) => format!(
            "S{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::LowPassCutoff(value) => format!(
            "Hz{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::LowPassResonance(value) => format!(
            "Q{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::SampleBank(value) => format!("B:{value}"),
        M::SampleVariant(value) => format!("V{value}"),
        M::PlaybackRate(value) => format!(
            "Rate{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::PlaybackStart(value) => format!(
            "In{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::PlaybackEnd(value) => format!(
            "Out{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Reverse(value) => format!("Rev{}", u8::from(*value)),
        M::Fit(value) => format!("Fit{}", u8::from(*value)),
        M::Loop(value) => format!("Loop{}", u8::from(*value)),
        M::Slice { index, count } => format!("Sl{}/{}", index + 1, count),
        M::Rev => "Rev".into(),
        M::Late(value) => format!(
            "Late {}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Fast(value) => format!(
            "×{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Slow(value) => format!(
            "/{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Elongate(value) => format!(
            "@{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Replicate(value) => format!("^{value}"),
        M::Degrade(Some(value)) => format!(
            "%{}",
            parameter_display(&tessera::prelude::FieldValue::rational(*value))
        ),
        M::Degrade(None) => "%".into(),
        M::Euclid { pulses, steps } => format!("E{pulses}:{steps}"),
        M::Scale(scale) => format!("{} {}", scale.root_label(), scale.mode.short_label()),
        M::EuclidPattern(pattern) => format!("E{}cy", pattern.period().unwrap_or(0)),
        M::EuclidRot {
            pulses,
            steps,
            rotation,
        } => format!("E{pulses}:{steps}:{rotation}"),
    }
}

pub(crate) fn parameter_display(value: &tessera::prelude::FieldValue) -> String {
    use tessera::prelude::FieldValue as V;
    match value {
        V::Modulation { value } => format!(
            "{}–{}",
            parameter_display(&V::rational(value.minimum)),
            parameter_display(&V::rational(value.maximum))
        ),
        V::Rational { value } if value.denominator == 1 => value.numerator.to_string(),
        V::Rational { value } => format!("{}/{}", value.numerator, value.denominator),
        V::Bool { value } => u8::from(*value).to_string(),
        V::Symbol { value } => value.clone(),
        V::Delay { value } => format!("{} s", parameter_display(&V::rational(value.time))),
        V::Reverb { value } => format!("{} s", parameter_display(&V::rational(value.decay))),
        V::Compressor { value } => format!("{}:1", parameter_display(&V::rational(value.ratio))),
        V::Slice { index, count } => format!("{}/{}", index + 1, count),
    }
}

#[cfg(test)]
mod tests {
    use tessera::prelude::NodeId;

    use super::*;

    fn id(value: &str) -> NodeId {
        NodeId::new(value)
    }

    #[test]
    fn note_and_octave_display_as_pitch_compound() {
        let atoms = vec![
            AtomView {
                node: id("a"),
                kind: AtomKind::NoteName(NoteName::A),
            },
            AtomView {
                node: id("oct"),
                kind: AtomKind::Octave(2),
            },
        ];
        let compounds = compound_atoms(&atoms);
        assert_eq!(compounds.len(), 1);
        assert_eq!(compounds[0].display, "A2");
        assert_eq!(compounds[0].members, vec![id("a"), id("oct")]);
        assert_eq!(compounds[0].semantic, AtomCompoundSemantic::Pitch);
    }

    #[test]
    fn note_accidental_and_octave_display_as_pitch_compound() {
        let atoms = vec![
            AtomView {
                node: id("c"),
                kind: AtomKind::NoteName(NoteName::C),
            },
            AtomView {
                node: id("sharp"),
                kind: AtomKind::Accidental(Accidental::Sharp),
            },
            AtomView {
                node: id("oct"),
                kind: AtomKind::Octave(3),
            },
        ];
        let compounds = compound_atoms(&atoms);
        assert_eq!(compounds.len(), 1);
        assert_eq!(compounds[0].display, "C♯3");
        assert_eq!(compounds[0].members, vec![id("c"), id("sharp"), id("oct")]);
    }

    #[test]
    fn flat_accidental_uses_music_symbol_not_b() {
        let atoms = vec![
            AtomView {
                node: id("d"),
                kind: AtomKind::NoteName(NoteName::D),
            },
            AtomView {
                node: id("flat"),
                kind: AtomKind::Accidental(Accidental::Flat),
            },
            AtomView {
                node: id("oct"),
                kind: AtomKind::Octave(4),
            },
        ];
        let compounds = compound_atoms(&atoms);
        assert_eq!(compounds.len(), 1);
        assert_eq!(compounds[0].display, "D♭4");
    }

    #[test]
    fn operator_and_number_display_as_operator_compound() {
        let atoms = vec![
            AtomView {
                node: id("pow"),
                kind: AtomKind::Operator(OperatorKind::Power),
            },
            AtomView {
                node: id("two"),
                kind: AtomKind::Number(2),
            },
        ];
        let compounds = compound_atoms(&atoms);
        assert_eq!(compounds.len(), 1);
        assert_eq!(compounds[0].display, "^2");
        assert_eq!(
            compounds[0].semantic,
            AtomCompoundSemantic::OperatorArgument
        );
    }

    #[test]
    fn pitch_and_operator_argument_do_not_merge() {
        let atoms = vec![
            AtomView {
                node: id("e"),
                kind: AtomKind::NoteName(NoteName::E),
            },
            AtomView {
                node: id("oct"),
                kind: AtomKind::Octave(3),
            },
            AtomView {
                node: id("at"),
                kind: AtomKind::Operator(OperatorKind::At),
            },
            AtomView {
                node: id("two"),
                kind: AtomKind::Number(2),
            },
        ];
        let compounds = compound_atoms(&atoms);
        assert_eq!(compounds.len(), 2);
        assert_eq!(compounds[0].display, "E3");
        assert_eq!(compounds[1].display, "@2");
    }

    #[test]
    fn octave_without_note_remains_single_atom() {
        let atoms = vec![AtomView {
            node: id("oct"),
            kind: AtomKind::Octave(3),
        }];
        let compounds = compound_atoms(&atoms);
        assert_eq!(compounds.len(), 1);
        assert_eq!(compounds[0].display, "3");
        assert_eq!(compounds[0].semantic, AtomCompoundSemantic::Single);
    }
}
