use tessera::piece_registry::PieceRegistry;

use crate::adapter::tessera::pieces::{
    AddPiece, ArgsConnectorPiece, ArrangePiece, AttackPiece, ClipPiece, CompressorPiece,
    ConnectorPiece, ControlInputPiece, DecayPiece, DelayPiece, FastPiece, GainPiece,
    HighPassCutoffPiece, HighPassResonancePiece, JuxByPiece, LayerPiece, LegatoPiece,
    LowPassCutoffPiece, LowPassResonancePiece, MaskPiece, MirrorPiece, NotePiece, OutputPiece,
    OverlayPiece, PanPiece, PatternAtomElongationPiece, PatternAtomFastPiece, PatternAtomNotePiece,
    PatternAtomPitchShiftPiece, PatternAtomRestPiece, PatternAtomScalarPiece, PatternAtomSlowPiece,
    PatternContainerAlternatePiece, PatternContainerBasicPiece, PatternContainerParallelPiece,
    PatternContainerSubdividePiece, PlaybackEndPiece, PlaybackRatePiece, PlaybackStartPiece,
    PolymeterPiece, PostGainPiece, ReleasePiece, RevPiece, ReverbSendPiece, ReversePiece,
    SampleBankPiece, ScalePiece, SilencePiece, SlowPiece, SoundPiece, SpeedPiece, StretchPiece,
    SustainPiece, VelocityPiece,
};

pub(crate) fn register_cadence_pieces(registry: &mut PieceRegistry) {
    registry.register(OverlayPiece::new());
    registry.register(LayerPiece::new());
    registry.register(ArrangePiece::new());
    registry.register(PolymeterPiece::new());
    registry.register(SilencePiece::new());
    registry.register(PatternContainerBasicPiece::new());
    registry.register(PatternContainerSubdividePiece::new());
    registry.register(PatternContainerAlternatePiece::new());
    registry.register(PatternContainerParallelPiece::new());
    registry.register(PatternAtomNotePiece::new());
    registry.register(PatternAtomScalarPiece::new());
    registry.register(PatternAtomRestPiece::new());
    registry.register(PatternAtomElongationPiece::new());
    registry.register(PatternAtomPitchShiftPiece::new());
    registry.register(PatternAtomSlowPiece::new());
    registry.register(PatternAtomFastPiece::new());
    registry.register(SoundPiece::new());
    registry.register(NotePiece::new());
    registry.register(SpeedPiece::new());
    registry.register(StretchPiece::new());
    registry.register(FastPiece::new());
    registry.register(GainPiece::new());
    registry.register(MirrorPiece::new());
    registry.register(RevPiece::new());
    registry.register(MaskPiece::new());
    registry.register(SampleBankPiece::new());
    registry.register(AddPiece::new());
    registry.register(ScalePiece::new());
    registry.register(ClipPiece::new());
    registry.register(JuxByPiece::new());
    registry.register(PlaybackStartPiece::new());
    registry.register(PlaybackEndPiece::new());
    registry.register(PanPiece::new());
    registry.register(PlaybackRatePiece::new());
    registry.register(ReversePiece::new());
    registry.register(VelocityPiece::new());
    registry.register(LegatoPiece::new());
    registry.register(AttackPiece::new());
    registry.register(DecayPiece::new());
    registry.register(SustainPiece::new());
    registry.register(ReleasePiece::new());
    registry.register(LowPassCutoffPiece::new());
    registry.register(LowPassResonancePiece::new());
    registry.register(HighPassCutoffPiece::new());
    registry.register(HighPassResonancePiece::new());
    registry.register(ReverbSendPiece::new());
    registry.register(DelayPiece::new());
    registry.register(CompressorPiece::new());
    registry.register(PostGainPiece::new());
    registry.register(SlowPiece::new());
    registry.register(ControlInputPiece::new());
    registry.register(OutputPiece::new());
    registry.register(ConnectorPiece::new());
    registry.register(ArgsConnectorPiece::new());
}

#[cfg(test)]
pub(crate) fn default_cadence_registry() -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_cadence_pieces(&mut registry);
    registry
}
