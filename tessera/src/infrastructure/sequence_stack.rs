use crate::domain::{AtomModifier, ContainerSurfaceTile, Rational, SignedAccidental};

use super::stack::{accidental, modifier, note, octave, rest, scalar};

#[derive(Debug, Clone, Default)]
pub struct SequenceStack {
    items: Vec<ContainerSurfaceTile>,
}

impl SequenceStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn note(mut self, value: impl AsRef<str>) -> Self {
        self.items.push(note(value));
        self
    }

    pub fn notes(mut self, values: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        for value in values {
            self.items.push(note(value));
        }
        self
    }

    pub fn scalar(mut self, value: i64) -> Self {
        self.items.push(scalar(value));
        self
    }

    pub fn octave(mut self, value: i64) -> Self {
        self.items.push(octave(value));
        self
    }

    pub fn accidental(mut self, value: SignedAccidental) -> Self {
        self.items.push(accidental(value));
        self
    }

    pub fn modifier(mut self, value: AtomModifier) -> Self {
        self.items.push(modifier(value));
        self
    }

    pub fn rest(mut self) -> Self {
        self.items.push(rest());
        self
    }

    pub fn elongate(mut self, value: i64) -> Self {
        self.items
            .push(modifier(AtomModifier::Elongate(Rational::from_integer(
                value,
            ))));
        self
    }

    pub fn fast(mut self, value: i64) -> Self {
        self.items
            .push(modifier(AtomModifier::Fast(Rational::from_integer(value))));
        self
    }

    pub fn slow(mut self, value: i64) -> Self {
        self.items
            .push(modifier(AtomModifier::Slow(Rational::from_integer(value))));
        self
    }

    pub fn push(mut self, item: ContainerSurfaceTile) -> Self {
        self.items.push(item);
        self
    }

    pub fn extend(mut self, other: SequenceStack) -> Self {
        self.items.extend(other.items);
        self
    }

    pub fn build(self) -> Vec<ContainerSurfaceTile> {
        self.items
    }
}
