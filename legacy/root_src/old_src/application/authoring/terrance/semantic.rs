use crate::{
    application::authoring::tile_pattern::TileScriptError, domain::script::parse_pitch_text,
};

use super::surface::{SurfaceNode, SurfacePattern, SurfaceSequenceMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Control,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchResolveMode {
    Absolute,
    Degree,
    Offset,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticPattern<A> {
    pub root: SemanticNode<A>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticNode<A> {
    Rest,
    Atom(A),
    Sequence {
        mode: SurfaceSequenceMode,
        children: Vec<SemanticNode<A>>,
    },
    Stack(Vec<SemanticNode<A>>),
    Alternate {
        inner: Box<SemanticNode<A>>,
    },
    Repeat {
        inner: Box<SemanticNode<A>>,
        times: u32,
    },
    Fast {
        inner: Box<SemanticNode<A>>,
        factor: f64,
    },
    Slow {
        inner: Box<SemanticNode<A>>,
        factor: f64,
    },
    Elongate {
        inner: Box<SemanticNode<A>>,
        weight: u32,
    },
    Chord(Vec<SemanticNode<A>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PitchAtom {
    Absolute { midi: f64, label: String },
    Degree(i32),
    Offset(f64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternValueAtom {
    Scalar(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaleSpec {
    pub root_midi: i32,
    pub intervals: [i32; 7],
}

impl ScaleSpec {
    pub fn parse(text: &str) -> Result<Self, TileScriptError> {
        let trimmed = text.trim();
        let Some((root_text, mode_text)) = trimmed.split_once(':') else {
            return Err(TileScriptError::InvalidCueForContext {
                cue: trimmed.to_string(),
                context: "scale pattern",
            });
        };
        let root = parse_pitch_text(root_text.trim())
            .and_then(|value| {
                let rounded = value.round();
                ((value - rounded).abs() < 1e-9).then_some(rounded as i32)
            })
            .ok_or_else(|| TileScriptError::InvalidCueForContext {
                cue: root_text.trim().to_string(),
                context: "scale root",
            })?;
        let intervals = match mode_text.trim().to_ascii_lowercase().as_str() {
            "major" | "ionian" => [0, 2, 4, 5, 7, 9, 11],
            "minor" | "aeolian" => [0, 2, 3, 5, 7, 8, 10],
            _ => {
                return Err(TileScriptError::InvalidCueForContext {
                    cue: mode_text.trim().to_string(),
                    context: "scale mode",
                });
            }
        };
        Ok(Self {
            root_midi: root,
            intervals,
        })
    }
}

pub fn resolve_pitch_pattern(
    pattern: &SurfacePattern,
    mode: PitchResolveMode,
) -> Result<SemanticPattern<PitchAtom>, TileScriptError> {
    Ok(SemanticPattern {
        root: resolve_pitch_node(&pattern.root, mode)?,
    })
}

pub fn resolve_pattern_values(
    pattern: &SurfacePattern,
    stream_kind: StreamKind,
) -> Result<SemanticPattern<PatternValueAtom>, TileScriptError> {
    Ok(SemanticPattern {
        root: resolve_pattern_value_node(&pattern.root, stream_kind),
    })
}

fn resolve_pitch_node(
    node: &SurfaceNode,
    mode: PitchResolveMode,
) -> Result<SemanticNode<PitchAtom>, TileScriptError> {
    map_surface(node, &mut |text| match mode {
        PitchResolveMode::Absolute => {
            let midi =
                parse_pitch_text(text).ok_or_else(|| TileScriptError::InvalidCueForContext {
                    cue: text.to_string(),
                    context: "note pattern",
                })?;
            Ok(PitchAtom::Absolute {
                midi,
                label: text.to_string(),
            })
        }
        PitchResolveMode::Degree => {
            let degree =
                text.trim()
                    .parse::<i32>()
                    .map_err(|_| TileScriptError::InvalidCueForContext {
                        cue: text.to_string(),
                        context: "degree pattern",
                    })?;
            Ok(PitchAtom::Degree(degree))
        }
        PitchResolveMode::Offset => {
            let offset = text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| TileScriptError::InvalidCueForContext {
                    cue: text.to_string(),
                    context: "pitch offset pattern",
                })?;
            Ok(PitchAtom::Offset(offset))
        }
    })
}

fn resolve_pattern_value_node(
    node: &SurfaceNode,
    stream_kind: StreamKind,
) -> SemanticNode<PatternValueAtom> {
    map_surface_infallible(node, &mut |text| match stream_kind {
        StreamKind::Control => PatternValueAtom::Scalar(text.to_string()),
    })
}

fn map_surface<A>(
    node: &SurfaceNode,
    resolver: &mut impl FnMut(&str) -> Result<A, TileScriptError>,
) -> Result<SemanticNode<A>, TileScriptError> {
    Ok(match node {
        SurfaceNode::Rest => SemanticNode::Rest,
        SurfaceNode::Atom(atom) => SemanticNode::Atom(resolver(atom.text.as_str())?),
        SurfaceNode::Sequence { mode, children } => SemanticNode::Sequence {
            mode: *mode,
            children: children
                .iter()
                .map(|child| map_surface(child, resolver))
                .collect::<Result<Vec<_>, _>>()?,
        },
        SurfaceNode::Stack(children) => SemanticNode::Stack(
            children
                .iter()
                .map(|child| map_surface(child, resolver))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        SurfaceNode::Alternate { inner } => SemanticNode::Alternate {
            inner: Box::new(map_surface(inner, resolver)?),
        },
        SurfaceNode::Repeat { inner, times } => SemanticNode::Repeat {
            inner: Box::new(map_surface(inner, resolver)?),
            times: *times,
        },
        SurfaceNode::Fast { inner, factor } => SemanticNode::Fast {
            inner: Box::new(map_surface(inner, resolver)?),
            factor: *factor,
        },
        SurfaceNode::Slow { inner, factor } => SemanticNode::Slow {
            inner: Box::new(map_surface(inner, resolver)?),
            factor: *factor,
        },
        SurfaceNode::Elongate { inner, weight } => SemanticNode::Elongate {
            inner: Box::new(map_surface(inner, resolver)?),
            weight: *weight,
        },
        SurfaceNode::Chord(children) => SemanticNode::Chord(
            children
                .iter()
                .map(|child| map_surface(child, resolver))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    })
}

fn map_surface_infallible<A>(
    node: &SurfaceNode,
    resolver: &mut impl FnMut(&str) -> A,
) -> SemanticNode<A> {
    match node {
        SurfaceNode::Rest => SemanticNode::Rest,
        SurfaceNode::Atom(atom) => SemanticNode::Atom(resolver(atom.text.as_str())),
        SurfaceNode::Sequence { mode, children } => SemanticNode::Sequence {
            mode: *mode,
            children: children
                .iter()
                .map(|child| map_surface_infallible(child, resolver))
                .collect(),
        },
        SurfaceNode::Stack(children) => SemanticNode::Stack(
            children
                .iter()
                .map(|child| map_surface_infallible(child, resolver))
                .collect(),
        ),
        SurfaceNode::Alternate { inner } => SemanticNode::Alternate {
            inner: Box::new(map_surface_infallible(inner, resolver)),
        },
        SurfaceNode::Repeat { inner, times } => SemanticNode::Repeat {
            inner: Box::new(map_surface_infallible(inner, resolver)),
            times: *times,
        },
        SurfaceNode::Fast { inner, factor } => SemanticNode::Fast {
            inner: Box::new(map_surface_infallible(inner, resolver)),
            factor: *factor,
        },
        SurfaceNode::Slow { inner, factor } => SemanticNode::Slow {
            inner: Box::new(map_surface_infallible(inner, resolver)),
            factor: *factor,
        },
        SurfaceNode::Elongate { inner, weight } => SemanticNode::Elongate {
            inner: Box::new(map_surface_infallible(inner, resolver)),
            weight: *weight,
        },
        SurfaceNode::Chord(children) => SemanticNode::Chord(
            children
                .iter()
                .map(|child| map_surface_infallible(child, resolver))
                .collect(),
        ),
    }
}
