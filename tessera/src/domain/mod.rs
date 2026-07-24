pub mod atom;
pub mod container;
pub mod diagnostics;
pub mod flow;
pub mod flow_arrangement;
pub mod pattern_ir;
pub mod program;
#[cfg(feature = "bevy")]
pub mod reflect;
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
    PatternEvent, PatternIr, PatternNodeIr, PatternOutput, PatternProvenance, PatternStream,
    PatternStreamShape, Point3Ir, PriorityConflictIr, PriorityMergePolicyIr, Rational, ScalarEvent,
    ScalarStream, ScalarStreamNodeIr, SpatialMotionIr, WeightedPatternIr,
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
