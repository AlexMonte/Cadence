use bevy_app::App;
use bevy_ecs::reflect::AppTypeRegistry;
use bevy_reflect::{Reflect, TypePath};

use crate::domain::{
    AtomExpr, AtomExprKind, AtomModifier, AtomOperatorToken, AtomTile, AuthoredTesseraProgram,
    AxisIr, BoardSlot, Container, ContainerAxis, ContainerId, ContainerKind, ContainerSurfaceTile,
    CycleSpan, Diagnostic, DiagnosticCategory, DiagnosticKind, DiagnosticLocation, EventField,
    EventValue, FlowControlKind, FlowControlNode, FlowControlPolicy, InputEndpoint, InputPort,
    MusicalValue, NodeId, NodeSpatialBindings, NormalizedProgram, NoteAtom, OutputEndpoint,
    OutputNode, OutputPort, PatternEvent, PatternStream, Point3Ir, PortGroupId, PortMemberId,
    Rational, RootPlacement, RootRelation, RootSurface, RootSurfaceNodeKind, ScalarAtom,
    SpatialMotionIr, SpatialSide, StreamSource, StreamTarget, TesseraProgram, TileFootprint,
    TransformKind, TransformNode,
};
use crate::infrastructure::CompileOptions;
use crate::infrastructure::{CompileReport, PreviewReport, ValidationReport};

macro_rules! register {
    ($app:expr, $($ty:ty),+ $(,)?) => {
        $( $app.register_type::<$ty>(); )+
    };
}

pub fn register_tessera_types(app: &mut App) {
    register!(
        app,
        AuthoredTesseraProgram,
        RootSurface,
        BoardSlot,
        TileFootprint,
        RootPlacement,
        SpatialSide,
        NodeSpatialBindings,
        NodeId,
        TesseraProgram,
        NormalizedProgram,
        ContainerId,
        ContainerKind,
        ContainerAxis,
        ContainerSurfaceTile,
        Container,
        TransformKind,
        TransformNode,
        FlowControlKind,
        FlowControlNode,
        FlowControlPolicy,
        OutputNode,
        RootSurfaceNodeKind,
        InputEndpoint,
        OutputEndpoint,
        StreamSource,
        StreamTarget,
        RootRelation,
        InputPort,
        OutputPort,
        PortGroupId,
        PortMemberId,
        Diagnostic,
        DiagnosticCategory,
        DiagnosticKind,
        DiagnosticLocation,
        AtomTile,
        AtomExpr,
        AtomExprKind,
        AtomModifier,
        AtomOperatorToken,
        MusicalValue,
        NoteAtom,
        ScalarAtom,
        Rational,
        CycleSpan,
        PatternEvent,
        PatternStream,
        EventValue,
        EventField,
        AxisIr,
        Point3Ir,
        SpatialMotionIr,
        CompileOptions,
        CompileReport,
        PreviewReport,
        ValidationReport,
    );
}

pub fn type_registry_contains<T: Reflect + TypePath>(app: &App) -> bool {
    let registry = app.world().resource::<AppTypeRegistry>();
    registry.read().contains(std::any::TypeId::of::<T>())
}
