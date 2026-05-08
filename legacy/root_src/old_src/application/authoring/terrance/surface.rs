use crate::application::authoring::tile_pattern::{
    CueToken, SequenceMode, TileScript, TileScriptNode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceAtom {
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSequenceMode {
    Cycle,
    Chain,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceNode {
    Rest,
    Atom(SurfaceAtom),
    Sequence {
        mode: SurfaceSequenceMode,
        children: Vec<SurfaceNode>,
    },
    Stack(Vec<SurfaceNode>),
    Alternate {
        inner: Box<SurfaceNode>,
    },
    Repeat {
        inner: Box<SurfaceNode>,
        times: u32,
    },
    Fast {
        inner: Box<SurfaceNode>,
        factor: f64,
    },
    Slow {
        inner: Box<SurfaceNode>,
        factor: f64,
    },
    Elongate {
        inner: Box<SurfaceNode>,
        weight: u32,
    },
    Chord(Vec<SurfaceNode>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePattern {
    pub root: SurfaceNode,
}

impl From<TileScript> for SurfacePattern {
    fn from(value: TileScript) -> Self {
        Self {
            root: convert_node(value.root()),
        }
    }
}

fn convert_node(node: &TileScriptNode) -> SurfaceNode {
    match node {
        TileScriptNode::Cue(cue) => SurfaceNode::Atom(atom(cue)),
        TileScriptNode::Rest => SurfaceNode::Rest,
        TileScriptNode::Sequence { mode, children } => SurfaceNode::Sequence {
            mode: convert_mode(*mode),
            children: children.iter().map(convert_node).collect(),
        },
        TileScriptNode::Stack(children) => {
            SurfaceNode::Stack(children.iter().map(convert_node).collect())
        }
        TileScriptNode::Chord(children) => {
            SurfaceNode::Chord(children.iter().map(convert_node).collect())
        }
        TileScriptNode::Alternate { inner } => SurfaceNode::Alternate {
            inner: Box::new(convert_node(inner)),
        },
        TileScriptNode::Elongate { inner, weight } => SurfaceNode::Elongate {
            inner: Box::new(convert_node(inner)),
            weight: *weight,
        },
        TileScriptNode::Repeat { inner, times } => SurfaceNode::Repeat {
            inner: Box::new(convert_node(inner)),
            times: *times,
        },
        TileScriptNode::Slow { inner, factor } => SurfaceNode::Slow {
            inner: Box::new(convert_node(inner)),
            factor: factor.value(),
        },
        TileScriptNode::Fast { inner, factor } => SurfaceNode::Fast {
            inner: Box::new(convert_node(inner)),
            factor: factor.value(),
        },
    }
}

fn atom(cue: &CueToken) -> SurfaceAtom {
    SurfaceAtom {
        text: cue.as_str().to_string(),
    }
}

fn convert_mode(mode: SequenceMode) -> SurfaceSequenceMode {
    match mode {
        SequenceMode::Cycle => SurfaceSequenceMode::Cycle,
        SequenceMode::Chain => SurfaceSequenceMode::Chain,
    }
}
