use tessera::prelude::{
    AtomOperatorToken, ContainerId, InputStackPiece, NodeId, NoteValue, Rational, SignedAccidental,
    StackPiece,
};

use super::document::{Accidental, AtomValue, NoteName, OperatorValue, TileSpawnKind};

pub fn stack_piece_from_atom(atom: AtomValue) -> Option<StackPiece> {
    match atom {
        AtomValue::NoteName(note) => Some(StackPiece::note(note_value(note), note_letter(note))),
        AtomValue::Rest => Some(StackPiece::Rest),
        AtomValue::Number(value) => Some(StackPiece::Scalar(Rational::from_integer(value as i64))),
        AtomValue::Octave(value) => Some(StackPiece::Scalar(Rational::from_integer(value as i64))),
        AtomValue::Accidental(accidental) => {
            Some(StackPiece::Accidental(signed_accidental(accidental)))
        }
        AtomValue::Operator(operator) => Some(StackPiece::Operator(operator_token(operator))),
    }
}

pub fn input_stack_piece_from_spawn(
    node: NodeId,
    tile: &TileSpawnKind,
    container: Option<ContainerId>,
) -> Option<InputStackPiece> {
    match tile {
        TileSpawnKind::Container { .. } => {
            container.map(|container| InputStackPiece::Container { node, container })
        }
        TileSpawnKind::Atom { atom } => match atom {
            AtomValue::Number(value) => Some(InputStackPiece::Scalar(Rational::from_integer(
                *value as i64,
            ))),
            AtomValue::Octave(value) => Some(InputStackPiece::Scalar(Rational::from_integer(
                *value as i64,
            ))),
            _ => None,
        },
        _ => None,
    }
}

fn note_letter(note: NoteName) -> &'static str {
    match note {
        NoteName::A => "a",
        NoteName::B => "b",
        NoteName::C => "c",
        NoteName::D => "d",
        NoteName::E => "e",
        NoteName::F => "f",
        NoteName::G => "g",
    }
}

fn note_value(note: NoteName) -> NoteValue {
    match note {
        NoteName::A => NoteValue::A,
        NoteName::B => NoteValue::B,
        NoteName::C => NoteValue::C,
        NoteName::D => NoteValue::D,
        NoteName::E => NoteValue::E,
        NoteName::F => NoteValue::F,
        NoteName::G => NoteValue::G,
    }
}

fn signed_accidental(accidental: Accidental) -> SignedAccidental {
    match accidental {
        Accidental::Sharp => SignedAccidental::Sharp,
        Accidental::Flat => SignedAccidental::Flat,
        Accidental::Natural => SignedAccidental::Natural,
    }
}

fn operator_token(operator: OperatorValue) -> AtomOperatorToken {
    match operator {
        OperatorValue::Power => AtomOperatorToken::Replicate,
        OperatorValue::At => AtomOperatorToken::Elongate,
        OperatorValue::Multiply => AtomOperatorToken::Fast,
        OperatorValue::Divide => AtomOperatorToken::Slow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_piece_from_atom_maps_note_and_operator() {
        assert!(matches!(
            stack_piece_from_atom(AtomValue::NoteName(NoteName::E)),
            Some(StackPiece::Note { .. })
        ));
        assert_eq!(
            stack_piece_from_atom(AtomValue::Operator(OperatorValue::At)),
            Some(StackPiece::Operator(AtomOperatorToken::Elongate))
        );
    }

    #[test]
    fn input_stack_piece_from_spawn_maps_container_and_scalar() {
        assert_eq!(
            input_stack_piece_from_spawn(
                NodeId::new("phrase"),
                &TileSpawnKind::Container {
                    kind: crate::domain::document::ContainerKind::Sequence
                },
                Some(ContainerId::new("phrase"))
            ),
            Some(InputStackPiece::Container {
                node: NodeId::new("phrase"),
                container: ContainerId::new("phrase"),
            })
        );
        assert_eq!(
            input_stack_piece_from_spawn(
                NodeId::new("rate"),
                &TileSpawnKind::Atom {
                    atom: AtomValue::Number(2)
                },
                None
            ),
            Some(InputStackPiece::Scalar(Rational::from_integer(2)))
        );
    }
}
