//! Shared Cadence-owned graph and type primitives.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridPos {
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct EdgeId(pub String);

impl EdgeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TileSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DomainBridgeKind {
    ControlToAudio,
    AudioToControl,
    EventToControl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PortType {
    Plain(String),
    Detailed {
        kind: String,
        #[serde(default)]
        domain: Option<String>,
    },
}

impl PortType {
    pub fn new(kind: impl Into<String>) -> Self {
        Self::Plain(kind.into())
    }

    pub fn kind(&self) -> &str {
        match self {
            Self::Plain(kind) | Self::Detailed { kind, .. } => kind.as_str(),
        }
    }

    pub fn domain(&self) -> Option<&str> {
        match self {
            Self::Plain(_) => Some("control"),
            Self::Detailed { domain, .. } => domain.as_deref(),
        }
    }
}

impl From<tessera::types::GridPos> for GridPos {
    fn from(value: tessera::types::GridPos) -> Self {
        Self {
            col: value.col,
            row: value.row,
        }
    }
}

impl From<GridPos> for tessera::types::GridPos {
    fn from(value: GridPos) -> Self {
        Self {
            col: value.col,
            row: value.row,
        }
    }
}

impl From<tessera::types::EdgeId> for EdgeId {
    fn from(value: tessera::types::EdgeId) -> Self {
        Self(value.0.to_string())
    }
}

impl From<tessera::types::TileSide> for TileSide {
    fn from(value: tessera::types::TileSide) -> Self {
        match value {
            tessera::types::TileSide::TOP => Self::Top,
            tessera::types::TileSide::RIGHT => Self::Right,
            tessera::types::TileSide::BOTTOM => Self::Bottom,
            tessera::types::TileSide::LEFT => Self::Left,
        }
    }
}

impl From<tessera::types::DomainBridgeKind> for DomainBridgeKind {
    fn from(value: tessera::types::DomainBridgeKind) -> Self {
        match value {
            tessera::types::DomainBridgeKind::ControlToAudio => Self::ControlToAudio,
            tessera::types::DomainBridgeKind::AudioToControl => Self::AudioToControl,
            tessera::types::DomainBridgeKind::EventToControl => Self::EventToControl,
        }
    }
}

impl From<tessera::types::PortType> for PortType {
    fn from(value: tessera::types::PortType) -> Self {
        match value.domain() {
            Some(domain) => Self::Detailed {
                kind: value.as_str().to_string(),
                domain: Some(match domain {
                    tessera::types::ExecutionDomain::Audio => "audio".to_string(),
                    tessera::types::ExecutionDomain::Control => "control".to_string(),
                    tessera::types::ExecutionDomain::Event => "event".to_string(),
                }),
            },
            None => Self::Plain(value.as_str().to_string()),
        }
    }
}
