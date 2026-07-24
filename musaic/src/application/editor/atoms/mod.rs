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
    Octave(i8),
    Accidental(Accidental),
    Operator(OperatorKind),
    Number(i32),
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
        if let AtomKind::Number(value) = atom.kind {
            members.push(atom.node.clone());
            display.push_str(&value.to_string());
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
        OperatorKind::At => "@",
        OperatorKind::Multiply => "*",
        OperatorKind::Divide => "/",
    }
}

fn atom_display(kind: &AtomKind) -> String {
    match kind {
        AtomKind::NoteName(note) => note_display(*note).to_string(),
        AtomKind::Octave(octave) => octave.to_string(),
        AtomKind::Accidental(accidental) => accidental_display(*accidental).to_string(),
        AtomKind::Operator(operator) => operator_display(*operator).to_string(),
        AtomKind::Number(value) => value.to_string(),
        AtomKind::Rest => "rest".to_string(),
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
