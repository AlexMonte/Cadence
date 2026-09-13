pub mod atom;
pub mod container;
pub mod diagnostics;
pub mod flow;
pub mod flow_arrangement;
mod flow_query;
pub mod parameter;
pub mod pattern_ir;
pub mod program;
mod query_ir;
pub mod relations;
pub mod spatial_defaults;
pub mod stack;
pub mod surface;

pub use atom::{
    AtomExpr, AtomExprKind, AtomModifier, AtomOperatorToken, AtomTile, MusicalValue, NoteAtom,
    NoteValue, ScalarAtom, try_parse_note_value,
};
pub use container::{
    Container, ContainerAxis, ContainerId, ContainerKind, ContainerPreviewVariant,
    ContainerSurfaceTile, NormalizedContainer, sequence_preview_slots,
};
pub use diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticKind, DiagnosticLocation};
pub use flow::{
    ConnectionRule, DefaultStreamBehavior, FlowControlKind, FlowControlNode, FlowControlPolicy,
    GroupMembers, InputGroupSpec, InputPort, InputSocketSpec, NodeInputRole, NodeSignature,
    OutputGroupSpec, OutputNode, OutputPort, OutputSocketSpec, PortCountRule, PortGroupId,
    PortMemberId, RootSurfaceNodeKind, Side, StreamShape, TransformKind, TransformNode,
};
pub use flow_arrangement::{
    Arrangement, ArrangementReject, ArrangementSegment, FlowComposer, FlowList, FlowRef, LowerCtx,
};
pub use pattern_ir::{
    AxisIr, ControlEvent, ControlKeyIr, ControlStream, ControlStreamNodeIr, ControlValueIr,
    CycleDuration, CycleSpan, CycleTime, DeduplicateKeyIr, DeduplicatePolicyIr,
    DeduplicateWinnerIr, EventField, EventStreamNodeIr, EventValue, FieldValue, FlatPatternOutput,
    FlowInputIr, PatternEvent, PatternIr, PatternNodeIr, PatternOutput, PatternProvenance,
    PatternStream, PatternStreamShape, Point3Ir, PriorityConflictIr, PriorityMergePolicyIr,
    Rational, ScalarEvent, ScalarStream, ScalarStreamNodeIr, SpatialMotionIr, TimedPatternIr,
    WeightedPatternIr,
};
pub use program::{NodeId, NormalizedProgram, TesseraProgram};
pub use relations::{InputEndpoint, OutputEndpoint, RootRelation, StreamSource, StreamTarget};
pub use spatial_defaults::{apply_flow_member_default_bindings, default_spatial_bindings};
pub use stack::{
    InputStackHost, InputStackPiece, InputStackReject, InputStackSurface, SignedAccidental,
    StackCompound, StackDisplay, StackDisplayPart, StackPiece, StackReject, StackSequence,
    StackSlotHint, StackSurfaceLayout, StackSurfaceRect, stack_compound_from_tiles,
    stack_layout_for_signature,
};
pub use surface::{
    AuthoredTesseraProgram, BoardSlot, NodeSpatialBindings, RootPlacement, RootSurface,
    SpatialSide, TileFootprint,
};

pub use parameter::{
    ParameterDomain, ParameterKey, ParameterMerge, ParameterSource, ParameterSpec, ParameterTiming,
    ParameterUnit,
};

mod effect_parameters;
pub use effect_parameters::{CompressorParameters, DelayParameters, EffectValue, ReverbParameters};

mod modulation;
pub use modulation::{ModulationParameters, ModulationWaveform};

mod musical_patterns;
pub use musical_patterns::{EuclidPatternParameters, ScaleMode, ScaleParameters};
