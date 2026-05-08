pub mod combinators;
pub mod connector;
pub mod constants;
pub mod generators;
pub mod terminal;
pub mod transforms;

use tessera::piece::PieceDef;
use tessera::types::{PieceCategory, PieceSemanticKind, PortType, TileSide};

pub use combinators::{ArrangePiece, LayerPiece, OverlayPiece, PolymeterPiece, SilencePiece};
pub use connector::{ArgsConnectorPiece, ConnectorPiece};
pub use constants::{
    ControlInputPiece, PatternAtomElongationPiece, PatternAtomFastPiece, PatternAtomNotePiece,
    PatternAtomPitchShiftPiece, PatternAtomRestPiece, PatternAtomScalarPiece, PatternAtomSlowPiece,
    PatternContainerAlternatePiece, PatternContainerBasicPiece, PatternContainerParallelPiece,
    PatternContainerSubdividePiece,
};
pub use generators::{NotePiece, SoundPiece};
pub use terminal::OutputPiece;
pub use transforms::{
    AddPiece, AttackPiece, ClipPiece, CompressorPiece, DecayPiece, DelayPiece, FastPiece,
    GainPiece, HighPassCutoffPiece, HighPassResonancePiece, JuxByPiece, LegatoPiece,
    LowPassCutoffPiece, LowPassResonancePiece, MaskPiece, MirrorPiece, PanPiece, PlaybackEndPiece,
    PlaybackRatePiece, PlaybackStartPiece, PostGainPiece, ReleasePiece, RevPiece, ReverbSendPiece,
    ReversePiece, SampleBankPiece, ScalePiece, SlowPiece, SpeedPiece, StretchPiece, SustainPiece,
    VelocityPiece,
};
pub(super) fn cadence_piece_def_with_tags(
    id: &str,
    label: &str,
    category: PieceCategory,
    params: Vec<tessera::piece::ParamDef>,
    output_type: Option<PortType>,
    output_side: Option<TileSide>,
    description: &str,
    tags: Vec<&str>,
) -> PieceDef {
    PieceDef {
        id: id.into(),
        label: label.into(),
        category,
        semantic_kind: cadence_semantic_kind(category),
        namespace: "cadence".into(),
        params,
        output_type,
        output_side,
        output_role: Default::default(),
        temporal_kind: Default::default(),
        fan_in: Default::default(),
        fan_out: Default::default(),
        description: Some(description.into()),
        tags: tags.into_iter().map(str::to_string).collect(),
    }
}

fn cadence_semantic_kind(category: PieceCategory) -> PieceSemanticKind {
    match category {
        PieceCategory::Source => PieceSemanticKind::Intrinsic,
        PieceCategory::Cadence => PieceSemanticKind::Construct,
        PieceCategory::Flow => PieceSemanticKind::Route,
        PieceCategory::State => PieceSemanticKind::State,
        PieceCategory::Constant => PieceSemanticKind::Literal,
        PieceCategory::Output => PieceSemanticKind::Output,
        PieceCategory::Observe => PieceSemanticKind::Observe,
        PieceCategory::Boundary => PieceSemanticKind::Output,
        PieceCategory::Connector => PieceSemanticKind::Connector,
        PieceCategory::Generator | PieceCategory::Transform => PieceSemanticKind::Intrinsic,
        PieceCategory::Structure | PieceCategory::Control => PieceSemanticKind::Construct,
        PieceCategory::Trick => PieceSemanticKind::Trick,
    }
}
