use serde::{Deserialize, Serialize};

use super::{ContainerId, InputEndpoint, NodeId, OutputEndpoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum DiagnosticCategory {
    Placement,
    LocalGrammar,
    RootRelation,
    TransformTopology,
    TransformArgument,
    FlowControlTopology,
    StreamShape,
    Cycle,
    Compile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum DiagnosticKind {
    MissingContainer,
    MissingPlacement,
    UnknownPlacedNode,
    OverlappingPlacement,
    UnknownBindingNode,
    TransformInsideContainer,
    OutputInsideContainer,
    AmbiguousOctaveBinding,
    OperatorWithoutLeftValue,
    OperatorWithoutRightScalar,
    InvalidModifierArgument,
    InvalidNote,
    InvalidFlowSource,
    InvalidFlowTarget,
    UnknownInputSocket,
    UnknownInputGroup,
    UnknownInputGroupMember,
    UnknownOutputSocket,
    UnknownOutputGroupMember,
    EndpointShapeMismatch,
    InvalidChainSource,
    InvalidChainTarget,
    TransformMissingMainInput,
    RequiredSocketMissing,
    OptionalSocketMultiplyBound,
    PortCountViolation,
    InvalidStreamShape,
    OutputMissingInput,
    OutputCannotProduceStream,
    RootCycle,
    InvalidTransformArgument,
    FlowControlCannotStartComposition,
    CompileFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum DiagnosticLocation {
    RootNode(NodeId),
    RootRelation {
        index: usize,
    },
    ContainerStack {
        container: ContainerId,
        index: usize,
    },
    InputEndpoint {
        node: NodeId,
        endpoint: InputEndpoint,
    },
    OutputEndpoint {
        node: NodeId,
        endpoint: OutputEndpoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct Diagnostic {
    pub category: DiagnosticCategory,
    pub kind: DiagnosticKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<DiagnosticLocation>,
}

impl Diagnostic {
    pub fn new(
        category: DiagnosticCategory,
        kind: DiagnosticKind,
        message: impl Into<String>,
        location: Option<DiagnosticLocation>,
    ) -> Self {
        Self {
            category,
            kind,
            message: message.into(),
            location,
        }
    }
}
