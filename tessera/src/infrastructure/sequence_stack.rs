use crate::domain::{AtomOperatorToken, ContainerSurfaceTile};

use super::stack::{note, op, rest, scalar};

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

    pub fn rest(mut self) -> Self {
        self.items.push(rest());
        self
    }

    pub fn elongate(mut self, value: i64) -> Self {
        self.items.push(op(AtomOperatorToken::Elongate));
        self.items.push(scalar(value));
        self
    }

    pub fn fast(mut self, value: i64) -> Self {
        self.items.push(op(AtomOperatorToken::Fast));
        self.items.push(scalar(value));
        self
    }

    pub fn slow(mut self, value: i64) -> Self {
        self.items.push(op(AtomOperatorToken::Slow));
        self.items.push(scalar(value));
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
