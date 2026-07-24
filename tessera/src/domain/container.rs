use serde::{Deserialize, Serialize};

use super::atom::{AtomExpr, AtomTile};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct ContainerId(pub String);

impl ContainerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum ContainerKind {
    Sequence,
    Alternate,
    Layer,
}

/// The axis a container's stacking unfolds along.
///
/// `Time` is the default and preserves the purely temporal behaviour: a
/// `Sequence` plays children one after another in time. The spatial axes
/// (`X`/`Y`/`Z`) keep that temporal structure but additionally place each
/// successive child at the next lattice cell along the axis, so structure
/// becomes a pattern of being over a temporal-spatial lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum ContainerAxis {
    #[default]
    Time,
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum ContainerSurfaceTile {
    Atom(AtomTile),
    NestedContainer(ContainerId),
    Transform,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct Container {
    pub kind: ContainerKind,
    #[serde(default)]
    pub axis: ContainerAxis,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack: Vec<ContainerSurfaceTile>,
}
impl Container {
    pub fn new(kind: ContainerKind, stack: Vec<ContainerSurfaceTile>) -> Self {
        Self {
            kind,
            axis: ContainerAxis::Time,
            stack,
        }
    }
    pub fn with_axis(mut self, axis: ContainerAxis) -> Self {
        self.axis = axis;
        self
    }
    pub fn kind(&self) -> ContainerKind {
        self.kind
    }
    pub fn axis(&self) -> ContainerAxis {
        self.axis
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct NormalizedContainer {
    pub id: ContainerId,
    pub kind: ContainerKind,
    #[serde(default)]
    pub axis: ContainerAxis,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exprs: Vec<AtomExpr>,
}

impl NormalizedContainer {
    pub fn new(id: ContainerId, kind: ContainerKind, exprs: Vec<AtomExpr>) -> Self {
        Self {
            id,
            kind,
            axis: ContainerAxis::Time,
            exprs,
        }
    }
    pub fn with_axis(mut self, axis: ContainerAxis) -> Self {
        self.axis = axis;
        self
    }
}

/// Preview grid variant on container tile meshes (not a cap on sequence length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerPreviewVariant {
    /// 3×3 socket grid on 1×1 footprint container mesh.
    OneByOne,
    /// 6×6 socket grid on 2×2 footprint container mesh.
    TwoByTwo,
}

impl ContainerPreviewVariant {
    pub fn grid_cols(&self) -> u32 {
        match self {
            Self::OneByOne => 3,
            Self::TwoByTwo => 6,
        }
    }

    pub fn grid_rows(&self) -> u32 {
        self.grid_cols()
    }

    pub fn capacity(&self) -> usize {
        (self.grid_cols() * self.grid_rows()) as usize
    }
}

/// Maps a sliding window of sequence indices onto preview grid cells (col, row).
pub fn sequence_preview_slots(
    variant: ContainerPreviewVariant,
    window_start: usize,
) -> impl Iterator<Item = (usize, u32, u32)> {
    let cols = variant.grid_cols();
    (0..variant.capacity()).map(move |offset| {
        let col = (offset as u32) % cols;
        let row = (offset as u32) / cols;
        (window_start + offset, col, row)
    })
}
