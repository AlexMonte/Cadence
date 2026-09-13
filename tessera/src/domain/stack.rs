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
pub enum SignedAccidental {
    Sharp,
    Flat,
    Natural,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StackPiece {
    Note { value: NoteValue, label: String },
    Sound(String),
    Accidental(SignedAccidental),
    Scalar(Rational),
    Operator(AtomOperatorToken),
    Octave(i64),
    Modifier(AtomModifier),
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
            StackPiece::Note { .. }
            | StackPiece::Sound(_)
            | StackPiece::Rest
            | StackPiece::Nested(_) => Err(StackReject::InvalidRoot),
            StackPiece::Scalar(_) | StackPiece::Modifier(_) => Ok(()),
            StackPiece::Octave(_) => {
                if matches!(self.layers.first(), Some(StackPiece::Note { .. })) {
                    Ok(())
                } else {
                    Err(StackReject::InvalidRoot)
                }
            }
            StackPiece::Accidental(_) => {
                if !matches!(self.layers.first(), Some(StackPiece::Note { .. }))
                    || self
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

        let mut value = match root {
            StackPiece::Note { value, label } => {
                let value = if label.is_empty() {
                    value.clone()
                } else {
                    try_parse_note_value(label).ok_or(StackReject::InvalidRoot)?
                };
                MusicalValue::Note(NoteAtom {
                    value,
                    label: label.clone(),
                    octave: None,
                    accidental: None,
                })
            }
            StackPiece::Modifier(modifier) => {
                validate_modifier(modifier)?;
                MusicalValue::Effect(modifier.effect_value().ok_or(StackReject::InvalidRoot)?)
            }
            StackPiece::Sound(value) => MusicalValue::Sound(value.clone()),
            StackPiece::Rest => MusicalValue::Rest,
            StackPiece::Scalar(value) => MusicalValue::Scalar(ScalarAtom { value: *value }),
            StackPiece::Nested(id) => MusicalValue::NestedContainer(id.clone()),
            _ => return Err(StackReject::InvalidRoot),
        };
        let mut modifiers = Vec::new();
        let mut index = 1;
        while let Some(piece) = self.layers.get(index) {
            match piece {
                StackPiece::Accidental(accidental) => {
                    let MusicalValue::Note(note) = &mut value else {
                        return Err(StackReject::InvalidRoot);
                    };
                    if note.accidental.replace(*accidental).is_some() {
                        return Err(StackReject::InvalidRoot);
                    }
                }
                StackPiece::Octave(octave) => {
                    let MusicalValue::Note(note) = &mut value else {
                        return Err(StackReject::InvalidRoot);
                    };
                    if note.octave.replace(*octave).is_some() {
                        return Err(StackReject::DuplicateOctave);
                    }
                }
                StackPiece::Scalar(scalar) => {
                    return Err(StackReject::UnassignedScalar { value: *scalar });
                }
                StackPiece::Modifier(modifier) => {
                    if matches!(value, MusicalValue::Effect(_))
                        && modifier.parameter_key().is_some()
                    {
                        return Err(StackReject::InvalidRoot);
                    }
                    validate_modifier(modifier)?;
                    modifiers.push(modifier.clone());
                }
                StackPiece::Operator(operator) => {
                    let count = match operator {
                        AtomOperatorToken::Euclid => 2,
                        AtomOperatorToken::EuclidRot => 3,
                        AtomOperatorToken::Choice | AtomOperatorToken::Parallel => {
                            return Err(StackReject::InvalidRoot);
                        }
                        AtomOperatorToken::Degrade => usize::from(matches!(
                            self.layers.get(index + 1),
                            Some(StackPiece::Scalar(_))
                        )),
                        _ => 1,
                    };
                    let mut arguments = Vec::new();
                    for offset in 1..=count {
                        let Some(StackPiece::Scalar(value)) = self.layers.get(index + offset)
                        else {
                            return Err(StackReject::MissingModifierArgument {
                                operator: *operator,
                            });
                        };
                        arguments.push(*value);
                    }
                    modifiers.extend(pair_modifiers(&[*operator], arguments)?);
                    index += count;
                }
                _ => return Err(StackReject::InvalidRoot),
            }
            index += 1;
        }
        Ok(AtomExpr {
            source_node: None,
            kind: AtomExprKind::Value(value),
            modifiers,
        })
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
        StackPiece::Note { .. }
            | StackPiece::Sound(_)
            | StackPiece::Rest
            | StackPiece::Scalar(_)
            | StackPiece::Nested(_)
    ) || matches!(piece, StackPiece::Modifier(modifier) if modifier.effect_value().is_some())
}

impl StackPiece {
    fn as_operator(&self) -> Option<AtomOperatorToken> {
        match self {
            Self::Operator(token) => Some(*token),
            _ => None,
        }
    }
}

/// Validation shared by the spatial stack and the normalizer's bound groups.
pub fn validate_modifier(modifier: &AtomModifier) -> Result<(), StackReject> {
    let parameter = modifier.parameter_key().zip(modifier.parameter_value());
    if let Some((parameter, value)) = parameter {
        return parameter
            .spec()
            .validate(&value)
            .map_err(|_| StackReject::InvalidParameter { parameter, value });
    }
    match modifier {
        AtomModifier::Scale(value) => {
            value
                .validate()
                .map_err(|message| StackReject::InvalidMusicalPattern {
                    message: message.into(),
                })?
        }
        AtomModifier::EuclidPattern(value) => {
            value
                .validate()
                .map_err(|message| StackReject::InvalidMusicalPattern {
                    message: message.into(),
                })?
        }
        AtomModifier::Fast(value) => {
            build_scalar_modifier(AtomOperatorToken::Fast, *value)?;
        }
        AtomModifier::Slow(value) => {
            build_scalar_modifier(AtomOperatorToken::Slow, *value)?;
        }
        AtomModifier::Elongate(value) => {
            build_scalar_modifier(AtomOperatorToken::Elongate, *value)?;
        }
        AtomModifier::Replicate(count) => {
            build_scalar_modifier(
                AtomOperatorToken::Replicate,
                Rational::from_integer(i64::from(*count)),
            )?;
        }
        AtomModifier::Degrade(Some(value))
            if *value < Rational::zero() || *value > Rational::one() =>
        {
            return Err(StackReject::InvalidModifierArgument {
                operator: AtomOperatorToken::Degrade,
                value: *value,
            });
        }
        AtomModifier::Euclid { pulses, steps } | AtomModifier::EuclidRot { pulses, steps, .. } => {
            validate_euclid(
                Rational::from_integer(i64::from(*pulses)),
                Rational::from_integer(i64::from(*steps)),
            )?
        }
        AtomModifier::Rev | AtomModifier::Degrade(_) => {}
        AtomModifier::Late(value) => {
            if value.denominator <= 0
                || *value < Rational::from_integer(-1024)
                || *value > Rational::from_integer(1024)
            {
                return Err(StackReject::InvalidMusicalPattern {
                    message: "Timing offset must be within 1024 cycles".into(),
                });
            }
        }
        AtomModifier::Modulation { .. }
        | AtomModifier::Gain(_)
        | AtomModifier::Attack(_)
        | AtomModifier::Decay(_)
        | AtomModifier::Release(_)
        | AtomModifier::Transpose(_)
        | AtomModifier::Pan(_)
        | AtomModifier::Delay(_)
        | AtomModifier::Reverb(_)
        | AtomModifier::Compressor(_)
        | AtomModifier::Velocity(_)
        | AtomModifier::ClipLength(_)
        | AtomModifier::PostGain(_)
        | AtomModifier::PitchBend(_)
        | AtomModifier::Expression(_)
        | AtomModifier::HighPassCutoff(_)
        | AtomModifier::HighPassResonance(_)
        | AtomModifier::Gate(_)
        | AtomModifier::Legato(_)
        | AtomModifier::Sustain(_)
        | AtomModifier::LowPassCutoff(_)
        | AtomModifier::LowPassResonance(_)
        | AtomModifier::SampleBank(_)
        | AtomModifier::SampleVariant(_)
        | AtomModifier::PlaybackRate(_)
        | AtomModifier::PlaybackStart(_)
        | AtomModifier::PlaybackEnd(_)
        | AtomModifier::Reverse(_)
        | AtomModifier::Fit(_)
        | AtomModifier::Loop(_)
        | AtomModifier::Slice { .. } => unreachable!("validated parameter above"),
    }
    Ok(())
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
                    if probability.denominator <= 0
                        || probability < Rational::zero()
                        || probability > Rational::one()
                    {
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
            if value.denominator <= 0 || value <= Rational::zero() {
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
pub enum StackReject {
    InvalidMusicalPattern {
        message: String,
    },
    InvalidParameter {
        parameter: super::ParameterKey,
        value: super::FieldValue,
    },
    InvalidRoot,
    MissingModifierArgument {
        operator: AtomOperatorToken,
    },
    UnassignedScalar {
        value: Rational,
    },
    DuplicateOctave,
    InvalidModifierArgument {
        operator: AtomOperatorToken,
        value: Rational,
    },
    EmptyCompound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackSlotHint {
    NeedsRoot,
    AcceptsLayer,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StackDisplay {
    pub parts: Vec<StackDisplayPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StackDisplayPart {
    Note { label: String },
    Sound(String),
    Accidental(SignedAccidental),
    Scalar(Rational),
    Operator(AtomOperatorToken),
    Octave(i64),
    Modifier(AtomModifier),
    Rest,
    Nested(ContainerId),
}

impl StackDisplayPart {
    fn from_piece(piece: &StackPiece) -> Self {
        match piece {
            StackPiece::Note { label, .. } => Self::Note {
                label: label.clone(),
            },
            StackPiece::Sound(value) => Self::Sound(value.clone()),
            StackPiece::Accidental(accidental) => Self::Accidental(*accidental),
            StackPiece::Scalar(value) => Self::Scalar(*value),
            StackPiece::Operator(token) => Self::Operator(*token),
            StackPiece::Octave(value) => Self::Octave(*value),
            StackPiece::Modifier(modifier) => Self::Modifier(modifier.clone()),
            StackPiece::Rest => Self::Rest,
            StackPiece::Nested(container) => Self::Nested(container.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputStackPiece {
    Container {
        node: NodeId,
        container: ContainerId,
    },
    Scalar(Rational),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputStackSurface {
    pub port: InputPort,
    pub role: super::flow::NodeInputRole,
    pub shape: StreamShape,
    pub connection: ConnectionRule,
    pub stack: Option<InputStackPiece>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct StackSurfaceRect {
    pub port: InputPort,
    pub slot: BoardSlot,
    pub footprint: TileFootprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    if let Some(ContainerSurfaceTile::Atom(AtomTile::Note(note))) = tiles.get(start) {
        if let Some(octave) = note.octave {
            let _ = compound.try_push(StackPiece::Octave(octave));
        }
        if let Some(accidental) = note.accidental {
            let _ = compound.try_push(StackPiece::Accidental(accidental));
        }
    }
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
        ContainerSurfaceTile::Atom(AtomTile::Sound(value)) => {
            Some(StackPiece::Sound(value.clone()))
        }
        ContainerSurfaceTile::Atom(AtomTile::Rest) => Some(StackPiece::Rest),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(scalar)) => {
            Some(StackPiece::Scalar(scalar.value))
        }
        ContainerSurfaceTile::Atom(AtomTile::Operator(token)) => Some(StackPiece::Operator(*token)),
        ContainerSurfaceTile::Atom(AtomTile::Accidental(value)) => {
            Some(StackPiece::Accidental(*value))
        }
        ContainerSurfaceTile::Atom(AtomTile::Octave(value)) => Some(StackPiece::Octave(*value)),
        ContainerSurfaceTile::Atom(AtomTile::Modifier(modifier)) => {
            Some(StackPiece::Modifier(modifier.clone()))
        }
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
    fn stack_compound_bound_groups_are_reorderable() {
        let groups = [
            StackPiece::Octave(4),
            StackPiece::Modifier(AtomModifier::Elongate(Rational::from_integer(2))),
            StackPiece::Modifier(AtomModifier::Fast(Rational::from_integer(3))),
        ];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut layers = vec![StackPiece::note(NoteValue::C, "c")];
            layers.extend(order.into_iter().map(|i| groups[i].clone()));
            let compound = compound_from_pieces(layers);
            let decoded: StackCompound =
                serde_json::from_str(&serde_json::to_string(&compound).unwrap()).unwrap();
            assert_eq!(decoded, compound);
            let expr = decoded.resolve().unwrap();
            assert!(matches!(
                expr.kind,
                AtomExprKind::Value(MusicalValue::Note(NoteAtom {
                    octave: Some(4),
                    ..
                }))
            ));
            assert!(
                expr.modifiers
                    .contains(&AtomModifier::Elongate(Rational::from_integer(2)))
            );
            assert!(
                expr.modifiers
                    .contains(&AtomModifier::Fast(Rational::from_integer(3)))
            );
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
            StackPiece::Octave(4),
        ]);
        let expr = compound.resolve().expect("resolves");
        match expr.kind {
            AtomExprKind::Value(MusicalValue::Note(note)) => {
                assert_eq!(note.value, NoteValue::C);
                assert_eq!(note.accidental, Some(SignedAccidental::Sharp));
                assert_eq!(note.pitch_label(), "c#");
                assert_eq!(note.semitone(4), 61);
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
                node: NodeId::new("pattern"),
                container: ContainerId::new("pattern"),
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
            } if *node == NodeId::new("pattern")
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
