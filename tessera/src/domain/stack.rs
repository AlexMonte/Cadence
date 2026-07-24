//! Tile vocabulary for Tessera's spatial authoring surface.
//!
//! | Term | Meaning | Authoring | Lowers to |
//! | --- | --- | --- | --- |
//! | **Nesting** | Containers inside containers | Nested container in interior sequence | `MusicalValue::NestedContainer` |
//! | **Stacking** | Vertical tile-on-tile | Drag onto tile (highlight valid targets) | Atom: `AtomExpr` · Input: `FlowsTo` |
//! | **Chaining** | Pattern containers linked in flow order | Connectors / flow between outputs → inputs | `ChainedTo` + `Concat` |
//! | **Transform** | Modulates flow into new output shape | Grey tile; stack container/scalar onto inputs | `TransformNode` + `FlowsTo` |
//! | **Connector** | Explicit lateral flow route | Connector tile or edge | `FlowsTo` / `RootRelation` |
//! | **Arrangement** | Song form — timed `[bars, flow]` segments | Arrangement tile | `Concat` of segments + inner `Parallel`/`Polymeter`/`Layer` |
//! | **Output** | Single flow sink per playable branch | Red output tile | `RootSurfaceNodeKind::Output` |
//!
//! **Naming:** vertical composition uses **Stack** (`StackPiece`, `StackCompound`). Strudel's
//! `stack(...)` maps to [`FlowComposer::Parallel`](crate::domain::flow_arrangement::FlowComposer),
//! not to atom stacking.

use serde::{Deserialize, Serialize};

use super::atom::{
    AtomExpr, AtomExprKind, AtomModifier, AtomOperatorToken, AtomTile, MusicalValue, NoteAtom,
    NoteValue, ScalarAtom, try_parse_note_value,
};
use super::container::{ContainerId, ContainerSurfaceTile};
use super::flow::{
    ConnectionRule, InputPort, NodeSignature, StreamShape, TransformKind, TransformNode,
};
use super::pattern_ir::Rational;
use super::program::NodeId;
use super::relations::{InputEndpoint, RootRelation, StreamSource, StreamTarget};
use super::surface::{BoardSlot, TileFootprint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum SignedAccidental {
    Sharp,
    Flat,
    Natural,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum StackPiece {
    Note { value: NoteValue, label: String },
    Accidental(SignedAccidental),
    Scalar(Rational),
    Operator(AtomOperatorToken),
    Rest,
    Nested(ContainerId),
}

impl StackPiece {
    pub fn note(value: NoteValue, label: impl Into<String>) -> Self {
        Self::Note {
            value,
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct StackCompound {
    layers: Vec<StackPiece>,
}

impl StackCompound {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_layers(layers: Vec<StackPiece>) -> Self {
        Self { layers }
    }

    pub fn layers(&self) -> &[StackPiece] {
        &self.layers
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn try_push(&mut self, piece: StackPiece) -> Result<(), StackReject> {
        self.push_reject_reason(&piece)?;
        self.layers.push(piece);
        Ok(())
    }

    pub fn can_push(&self, piece: &StackPiece) -> bool {
        self.push_reject_reason(piece).is_ok()
    }

    fn push_reject_reason(&self, piece: &StackPiece) -> Result<(), StackReject> {
        if self.layers.is_empty() {
            return if is_valid_root(piece) {
                Ok(())
            } else {
                Err(StackReject::InvalidRoot)
            };
        }

        match piece {
            StackPiece::Note { .. } | StackPiece::Rest | StackPiece::Nested(_) => {
                Err(StackReject::InvalidRoot)
            }
            StackPiece::Scalar(_) => Ok(()),
            StackPiece::Accidental(_) => {
                if !matches!(self.layers.first(), Some(StackPiece::Note { .. })) {
                    Err(StackReject::InvalidRoot)
                } else if self
                    .layers
                    .iter()
                    .any(|layer| matches!(layer, StackPiece::Accidental(_)))
                {
                    Err(StackReject::InvalidRoot)
                } else {
                    Ok(())
                }
            }
            StackPiece::Operator(AtomOperatorToken::Choice | AtomOperatorToken::Parallel) => {
                Err(StackReject::InvalidModifierArgument {
                    operator: piece.as_operator().expect("checked above"),
                    value: Rational::zero(),
                })
            }
            StackPiece::Operator(_) => Ok(()),
        }
    }

    pub fn hint(&self) -> StackSlotHint {
        if self.layers.is_empty() {
            StackSlotHint::NeedsRoot
        } else if self.resolve().is_ok() {
            StackSlotHint::Closed
        } else {
            StackSlotHint::AcceptsLayer
        }
    }

    pub fn resolve(&self) -> Result<AtomExpr, StackReject> {
        if self.layers.is_empty() {
            return Err(StackReject::EmptyCompound);
        }

        let root = self.layers.first().expect("non-empty");
        if !is_valid_root(root) {
            return Err(StackReject::InvalidRoot);
        }

        let mut accidentals = Vec::new();
        let mut scalars = Vec::new();
        let mut operators = Vec::new();

        for piece in self.layers.iter().skip(1) {
            match piece {
                StackPiece::Accidental(accidental) => accidentals.push(*accidental),
                StackPiece::Scalar(value) => scalars.push(*value),
                StackPiece::Operator(token) => {
                    if matches!(
                        token,
                        AtomOperatorToken::Choice | AtomOperatorToken::Parallel
                    ) {
                        return Err(StackReject::InvalidModifierArgument {
                            operator: *token,
                            value: Rational::zero(),
                        });
                    }
                    operators.push(*token);
                }
                StackPiece::Note { .. } | StackPiece::Rest | StackPiece::Nested(_) => {
                    return Err(StackReject::InvalidRoot);
                }
            }
        }

        match root {
            StackPiece::Note { value, label } => {
                let mut note = NoteAtom {
                    value: value.clone(),
                    label: label.clone(),
                    octave: None,
                };
                if !note.label.is_empty() {
                    if try_parse_note_value(&note.label).is_err() {
                        return Err(StackReject::InvalidRoot);
                    }
                    note.value = try_parse_note_value(&note.label).expect("validated above");
                }
                apply_accidentals(&mut note, &accidentals);
                let (octave_scalar, modifier_scalars) = partition_octave_scalars(scalars);
                if let Some(octave) = octave_scalar {
                    if octave.denominator != 1 {
                        return Err(StackReject::InvalidOctave { value: octave });
                    }
                    note.octave = Some(octave.numerator);
                }
                let modifiers = pair_modifiers(&operators, modifier_scalars)?;
                Ok(AtomExpr {
                    kind: AtomExprKind::Value(MusicalValue::Note(note)),
                    modifiers,
                })
            }
            StackPiece::Rest => {
                let modifiers = pair_modifiers(&operators, scalars)?;
                Ok(AtomExpr {
                    kind: AtomExprKind::Value(MusicalValue::Rest),
                    modifiers,
                })
            }
            StackPiece::Scalar(scalar) => {
                let modifiers = pair_modifiers(&operators, scalars)?;
                Ok(AtomExpr {
                    kind: AtomExprKind::Value(MusicalValue::Scalar(ScalarAtom { value: *scalar })),
                    modifiers,
                })
            }
            StackPiece::Nested(container) => {
                let modifiers = pair_modifiers(&operators, scalars)?;
                Ok(AtomExpr {
                    kind: AtomExprKind::Value(MusicalValue::NestedContainer(container.clone())),
                    modifiers,
                })
            }
            StackPiece::Accidental(_) | StackPiece::Operator(_) => Err(StackReject::InvalidRoot),
        }
    }

    pub fn display_parts(&self) -> StackDisplay {
        StackDisplay {
            parts: self
                .layers
                .iter()
                .map(StackDisplayPart::from_piece)
                .collect(),
        }
    }
}

fn is_valid_root(piece: &StackPiece) -> bool {
    matches!(
        piece,
        StackPiece::Note { .. } | StackPiece::Rest | StackPiece::Scalar(_) | StackPiece::Nested(_)
    )
}

impl StackPiece {
    fn as_operator(&self) -> Option<AtomOperatorToken> {
        match self {
            Self::Operator(token) => Some(*token),
            _ => None,
        }
    }
}

fn apply_accidentals(note: &mut NoteAtom, accidentals: &[SignedAccidental]) {
    for accidental in accidentals {
        match accidental {
            SignedAccidental::Sharp => match note.value {
                NoteValue::A => note.value = NoteValue::B,
                NoteValue::B => note.value = NoteValue::C,
                NoteValue::C => note.value = NoteValue::D,
                NoteValue::D => note.value = NoteValue::E,
                NoteValue::E => note.value = NoteValue::F,
                NoteValue::F => note.value = NoteValue::G,
                NoteValue::G => note.value = NoteValue::A,
            },
            SignedAccidental::Flat => match note.value {
                NoteValue::A => note.value = NoteValue::G,
                NoteValue::B => note.value = NoteValue::A,
                NoteValue::C => note.value = NoteValue::B,
                NoteValue::D => note.value = NoteValue::C,
                NoteValue::E => note.value = NoteValue::D,
                NoteValue::F => note.value = NoteValue::E,
                NoteValue::G => note.value = NoteValue::F,
            },
            SignedAccidental::Natural => {}
        }
    }
}

fn partition_octave_scalars(scalars: Vec<Rational>) -> (Option<Rational>, Vec<Rational>) {
    if let Some((index, _octave)) = scalars
        .iter()
        .enumerate()
        .find(|(_, scalar)| scalar.denominator == 1)
    {
        let mut rest = scalars;
        let octave = rest.remove(index);
        (Some(octave), rest)
    } else {
        (None, scalars)
    }
}

fn pair_modifiers(
    operators: &[AtomOperatorToken],
    mut scalars: Vec<Rational>,
) -> Result<Vec<AtomModifier>, StackReject> {
    let mut modifiers = Vec::new();
    let mut scalar_iter = scalars.drain(..);

    for operator in operators.iter().copied() {
        match operator {
            AtomOperatorToken::Degrade => {
                if let Some(probability) = scalar_iter.next() {
                    if probability < Rational::zero() || probability > Rational::one() {
                        return Err(StackReject::InvalidModifierArgument {
                            operator,
                            value: probability,
                        });
                    }
                    modifiers.push(AtomModifier::Degrade(Some(probability)));
                } else {
                    modifiers.push(AtomModifier::Degrade(None));
                }
            }
            AtomOperatorToken::Euclid => {
                let pulses = scalar_iter
                    .next()
                    .ok_or(StackReject::MissingModifierArgument { operator })?;
                let steps = scalar_iter
                    .next()
                    .ok_or(StackReject::MissingModifierArgument { operator })?;
                validate_euclid(pulses, steps)?;
                modifiers.push(AtomModifier::Euclid {
                    pulses: pulses.numerator.max(0) as u32,
                    steps: steps.numerator.max(1) as u32,
                });
            }
            AtomOperatorToken::EuclidRot => {
                let pulses = scalar_iter
                    .next()
                    .ok_or(StackReject::MissingModifierArgument { operator })?;
                let steps = scalar_iter
                    .next()
                    .ok_or(StackReject::MissingModifierArgument { operator })?;
                let rotation = scalar_iter
                    .next()
                    .ok_or(StackReject::MissingModifierArgument { operator })?;
                validate_euclid(pulses, steps)?;
                if rotation.denominator != 1 {
                    return Err(StackReject::InvalidModifierArgument {
                        operator,
                        value: rotation,
                    });
                }
                modifiers.push(AtomModifier::EuclidRot {
                    pulses: pulses.numerator.max(0) as u32,
                    steps: steps.numerator.max(1) as u32,
                    rotation: rotation.numerator as i32,
                });
            }
            AtomOperatorToken::Fast
            | AtomOperatorToken::Slow
            | AtomOperatorToken::Elongate
            | AtomOperatorToken::Replicate => {
                let value = scalar_iter
                    .next()
                    .ok_or(StackReject::MissingModifierArgument { operator })?;
                modifiers.push(build_scalar_modifier(operator, value)?);
            }
            AtomOperatorToken::Choice | AtomOperatorToken::Parallel => {
                return Err(StackReject::InvalidModifierArgument {
                    operator,
                    value: Rational::zero(),
                });
            }
        }
    }

    if let Some(leftover) = scalar_iter.next() {
        return Err(StackReject::UnassignedScalar { value: leftover });
    }

    Ok(modifiers)
}

fn validate_euclid(pulses: Rational, steps: Rational) -> Result<(), StackReject> {
    if pulses.denominator != 1
        || steps.denominator != 1
        || pulses.numerator < 0
        || steps.numerator <= 0
        || pulses.numerator > steps.numerator
    {
        return Err(StackReject::InvalidModifierArgument {
            operator: AtomOperatorToken::Euclid,
            value: steps,
        });
    }
    Ok(())
}

fn build_scalar_modifier(
    operator: AtomOperatorToken,
    value: Rational,
) -> Result<AtomModifier, StackReject> {
    match operator {
        AtomOperatorToken::Fast | AtomOperatorToken::Slow | AtomOperatorToken::Elongate => {
            if value <= Rational::zero() {
                return Err(StackReject::InvalidModifierArgument { operator, value });
            }
            Ok(match operator {
                AtomOperatorToken::Fast => AtomModifier::Fast(value),
                AtomOperatorToken::Slow => AtomModifier::Slow(value),
                AtomOperatorToken::Elongate => AtomModifier::Elongate(value),
                _ => unreachable!(),
            })
        }
        AtomOperatorToken::Replicate => {
            if value.denominator != 1 || value.numerator < 1 {
                return Err(StackReject::InvalidModifierArgument { operator, value });
            }
            Ok(AtomModifier::Replicate(value.numerator as u32))
        }
        _ => Err(StackReject::InvalidModifierArgument { operator, value }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct StackSequence {
    pub compounds: Vec<StackCompound>,
}

impl StackSequence {
    pub fn new(compounds: Vec<StackCompound>) -> Self {
        Self { compounds }
    }

    pub fn resolve(&self) -> Result<Vec<AtomExpr>, StackReject> {
        self.compounds.iter().map(StackCompound::resolve).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum StackReject {
    InvalidRoot,
    MissingModifierArgument {
        operator: AtomOperatorToken,
    },
    UnassignedScalar {
        value: Rational,
    },
    InvalidOctave {
        value: Rational,
    },
    InvalidModifierArgument {
        operator: AtomOperatorToken,
        value: Rational,
    },
    EmptyCompound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum StackSlotHint {
    NeedsRoot,
    AcceptsLayer,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct StackDisplay {
    pub parts: Vec<StackDisplayPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum StackDisplayPart {
    Note { label: String },
    Accidental(SignedAccidental),
    Scalar(Rational),
    Operator(AtomOperatorToken),
    Rest,
    Nested(ContainerId),
}

impl StackDisplayPart {
    fn from_piece(piece: &StackPiece) -> Self {
        match piece {
            StackPiece::Note { label, .. } => Self::Note {
                label: label.clone(),
            },
            StackPiece::Accidental(accidental) => Self::Accidental(*accidental),
            StackPiece::Scalar(value) => Self::Scalar(*value),
            StackPiece::Operator(token) => Self::Operator(*token),
            StackPiece::Rest => Self::Rest,
            StackPiece::Nested(container) => Self::Nested(container.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum InputStackPiece {
    Container {
        node: NodeId,
        container: ContainerId,
    },
    Scalar(Rational),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct InputStackSurface {
    pub port: InputPort,
    pub role: super::flow::NodeInputRole,
    pub shape: StreamShape,
    pub connection: ConnectionRule,
    pub stack: Option<InputStackPiece>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct InputStackHost {
    pub node: NodeId,
    pub surfaces: Vec<InputStackSurface>,
}

impl InputStackHost {
    pub fn from_transform(node: NodeId, kind: TransformKind) -> Self {
        let signature = TransformNode::new(kind).signature;
        Self::from_signature(node, signature)
    }

    pub fn from_signature(node: NodeId, signature: NodeSignature) -> Self {
        Self {
            node,
            surfaces: signature
                .input_sockets
                .into_iter()
                .map(|socket| InputStackSurface {
                    port: socket.port,
                    role: socket.role,
                    shape: socket.shape,
                    connection: socket.connection,
                    stack: None,
                })
                .collect(),
        }
    }

    pub fn try_stack(
        &mut self,
        port: InputPort,
        piece: InputStackPiece,
    ) -> Result<(), InputStackReject> {
        self.stack_reject_reason(&port, &piece)?;
        let surface = self
            .surface_mut(&port)
            .ok_or(InputStackReject::UnknownInputPort { port: port.clone() })?;
        if surface.stack.is_some() {
            return Err(InputStackReject::DuplicateStack { port });
        }
        surface.stack = Some(piece);
        Ok(())
    }

    pub fn can_stack(&self, port: &InputPort, piece: &InputStackPiece) -> bool {
        self.stack_reject_reason(port, piece).is_ok()
    }

    fn stack_reject_reason(
        &self,
        port: &InputPort,
        piece: &InputStackPiece,
    ) -> Result<(), InputStackReject> {
        let Some(surface) = self.surfaces.iter().find(|surface| &surface.port == port) else {
            return Err(InputStackReject::UnknownInputPort { port: port.clone() });
        };
        if surface.stack.is_some() {
            return Err(InputStackReject::DuplicateStack { port: port.clone() });
        }
        let expected = surface.shape;
        let matches = match piece {
            InputStackPiece::Container { .. } => {
                matches!(
                    expected,
                    StreamShape::Any | StreamShape::EventPattern | StreamShape::ControlPattern
                )
            }
            InputStackPiece::Scalar(_) => {
                matches!(
                    expected,
                    StreamShape::Any | StreamShape::ScalarPattern | StreamShape::ControlPattern
                )
            }
        };
        if !matches {
            return Err(InputStackReject::ShapeMismatch {
                expected,
                got: piece.clone(),
            });
        }
        Ok(())
    }

    pub fn resolve_relations(&self) -> Result<Vec<RootRelation>, InputStackReject> {
        let mut relations = Vec::new();
        for surface in &self.surfaces {
            match (&surface.connection, &surface.stack) {
                (ConnectionRule::Required, None) => {
                    return Err(InputStackReject::RequiredInputEmpty {
                        port: surface.port.clone(),
                    });
                }
                (_, Some(InputStackPiece::Container { node, .. })) => {
                    relations.push(RootRelation::FlowsTo {
                        from: StreamSource::node(node.clone()),
                        to: StreamTarget::TransformInput {
                            node: self.node.clone(),
                            endpoint: InputEndpoint::Socket(surface.port.clone()),
                        },
                    });
                }
                (_, Some(InputStackPiece::Scalar(_))) | (_, None) => {}
            }
        }
        Ok(relations)
    }

    fn surface_mut(&mut self, port: &InputPort) -> Option<&mut InputStackSurface> {
        self.surfaces
            .iter_mut()
            .find(|surface| &surface.port == port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum InputStackReject {
    UnknownHost,
    UnknownInputPort {
        port: InputPort,
    },
    ShapeMismatch {
        expected: StreamShape,
        got: InputStackPiece,
    },
    RequiredInputEmpty {
        port: InputPort,
    },
    DuplicateStack {
        port: InputPort,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct StackSurfaceRect {
    pub port: InputPort,
    pub slot: BoardSlot,
    pub footprint: TileFootprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct StackSurfaceLayout {
    pub host_footprint: TileFootprint,
    pub surfaces: Vec<StackSurfaceRect>,
}

pub fn stack_layout_for_signature(signature: &NodeSignature) -> StackSurfaceLayout {
    let aux_sockets = signature
        .input_sockets
        .iter()
        .filter(|socket| socket.role != super::flow::NodeInputRole::Main)
        .collect::<Vec<_>>();
    let host_height = 1u32.saturating_add(aux_sockets.len() as u32);
    let host_footprint = TileFootprint::new(1, host_height);
    let mut surfaces = Vec::new();

    let mut aux_row = 0i32;
    for socket in &signature.input_sockets {
        let y = if socket.role == super::flow::NodeInputRole::Main {
            host_height.saturating_sub(1) as i32
        } else {
            let row = aux_row;
            aux_row += 1;
            row
        };
        let _ = socket.side;
        surfaces.push(StackSurfaceRect {
            port: socket.port.clone(),
            slot: BoardSlot::new(0, y),
            footprint: TileFootprint::unit(),
        });
    }

    let _ = aux_sockets;
    StackSurfaceLayout {
        host_footprint,
        surfaces,
    }
}

/// Build a [`StackCompound`] from a linear container stack slice starting at `start`.
pub fn stack_compound_from_tiles(tiles: &[ContainerSurfaceTile], start: usize) -> StackCompound {
    let mut compound = StackCompound::new();
    let Some(first) = stack_piece_from_tile(tiles.get(start)) else {
        return compound;
    };
    let _ = compound.try_push(first);
    let mut index = start + 1;
    while index < tiles.len() {
        let Some(piece) = stack_piece_from_tile(tiles.get(index)) else {
            break;
        };
        if !compound.can_push(&piece) {
            break;
        }
        let _ = compound.try_push(piece);
        index += 1;
    }
    compound
}

fn stack_piece_from_tile(tile: Option<&ContainerSurfaceTile>) -> Option<StackPiece> {
    match tile? {
        ContainerSurfaceTile::Atom(AtomTile::Note(note)) => Some(StackPiece::note(
            note.value.clone(),
            if note.label.is_empty() {
                note.value.to_string()
            } else {
                note.label.clone()
            },
        )),
        ContainerSurfaceTile::Atom(AtomTile::Rest) => Some(StackPiece::Rest),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(scalar)) => {
            Some(StackPiece::Scalar(scalar.value))
        }
        ContainerSurfaceTile::Atom(AtomTile::Operator(token)) => Some(StackPiece::Operator(*token)),
        ContainerSurfaceTile::NestedContainer(container) => {
            Some(StackPiece::Nested(container.clone()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AtomOperatorToken;

    fn compound_from_pieces(pieces: Vec<StackPiece>) -> StackCompound {
        let mut compound = StackCompound::new();
        for piece in pieces {
            compound.try_push(piece).expect("valid stack piece");
        }
        compound
    }

    #[test]
    fn stack_compound_e2_at_2_order_independent() {
        let left = compound_from_pieces(vec![
            StackPiece::note(NoteValue::E, "e"),
            StackPiece::Scalar(Rational::from_integer(2)),
            StackPiece::Operator(AtomOperatorToken::Elongate),
            StackPiece::Scalar(Rational::from_integer(2)),
        ]);
        let right = compound_from_pieces(vec![
            StackPiece::note(NoteValue::E, "e"),
            StackPiece::Operator(AtomOperatorToken::Elongate),
            StackPiece::Scalar(Rational::from_integer(2)),
            StackPiece::Scalar(Rational::from_integer(2)),
        ]);

        let left_expr = left.resolve().expect("left resolves");
        let right_expr = right.resolve().expect("right resolves");
        assert_eq!(left_expr, right_expr);
        assert!(matches!(
            left_expr.modifiers.as_slice(),
            [AtomModifier::Elongate(value)] if *value == Rational::from_integer(2)
        ));
        match left_expr.kind {
            AtomExprKind::Value(MusicalValue::Note(note)) => {
                assert_eq!(note.octave, Some(2));
            }
            other => panic!("unexpected expr kind: {other:?}"),
        }
    }

    #[test]
    fn stack_compound_rejects_unassigned_scalar() {
        let compound = compound_from_pieces(vec![
            StackPiece::note(NoteValue::E, "e"),
            StackPiece::Scalar(Rational::from_integer(2)),
            StackPiece::Scalar(Rational::from_integer(3)),
        ]);
        assert!(matches!(
            compound.resolve(),
            Err(StackReject::UnassignedScalar { .. })
        ));
    }

    #[test]
    fn stack_compound_rejects_bare_elongate() {
        let compound = compound_from_pieces(vec![
            StackPiece::note(NoteValue::E, "e"),
            StackPiece::Operator(AtomOperatorToken::Elongate),
        ]);
        assert!(matches!(
            compound.resolve(),
            Err(StackReject::MissingModifierArgument {
                operator: AtomOperatorToken::Elongate
            })
        ));
    }

    #[test]
    fn stack_compound_applies_accidental() {
        let compound = compound_from_pieces(vec![
            StackPiece::note(NoteValue::C, "c"),
            StackPiece::Accidental(SignedAccidental::Sharp),
            StackPiece::Scalar(Rational::from_integer(4)),
        ]);
        let expr = compound.resolve().expect("resolves");
        match expr.kind {
            AtomExprKind::Value(MusicalValue::Note(note)) => {
                assert_eq!(note.value, NoteValue::D);
                assert_eq!(note.octave, Some(4));
            }
            other => panic!("unexpected expr kind: {other:?}"),
        }
    }

    #[test]
    fn input_stack_host_slow_main_and_factor() {
        let mut host = InputStackHost::from_transform(NodeId::new("slow"), TransformKind::Slow);
        host.try_stack(
            InputPort::new("main"),
            InputStackPiece::Container {
                node: NodeId::new("phrase"),
                container: ContainerId::new("phrase"),
            },
        )
        .expect("main accepts container");
        host.try_stack(
            InputPort::new("factor"),
            InputStackPiece::Scalar(Rational::from_integer(2)),
        )
        .expect("factor accepts scalar");

        let relations = host.resolve_relations().expect("relations resolve");
        assert_eq!(relations.len(), 1);
        assert!(matches!(
            &relations[0],
            RootRelation::FlowsTo {
                from: StreamSource { node, .. },
                ..
            } if *node == NodeId::new("phrase")
        ));
    }

    #[test]
    fn stack_layout_for_slow_transform() {
        let signature = TransformNode::new(TransformKind::Slow).signature;
        let layout = stack_layout_for_signature(&signature);
        assert_eq!(layout.host_footprint.height, 2);
        assert_eq!(layout.surfaces.len(), 2);
    }
}
