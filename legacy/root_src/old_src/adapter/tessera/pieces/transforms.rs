use super::cadence_piece_def_with_tags;
use crate::adapter::tessera::cadence_schema::{pattern_port, pattern_schema};
use tessera::piece::{
    ParamDef, ParamInlineMode, ParamSchema, ParamTextSemantics, ParamValueKind, Piece, PieceDef,
};
use tessera::types::{PieceCategory, PortType, TileSide};

pub struct SpeedPiece {
    def: PieceDef,
}

impl SpeedPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.speed",
                "speed",
                "factor",
                "factor",
                2.0,
                Some(0.125),
                Some(32.0),
                "Query-time timing compression.",
                vec!["fast"],
            ),
        }
    }
}

impl Piece for SpeedPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct StretchPiece {
    def: PieceDef,
}

impl StretchPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.stretch",
                "stretch",
                "factor",
                "factor",
                2.0,
                Some(0.125),
                Some(32.0),
                "Query-time timing expansion.",
                vec!["slow"],
            ),
        }
    }
}

impl Piece for StretchPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct GainPiece {
    def: PieceDef,
}

impl GainPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.gain",
                "gain",
                "amount",
                "amount",
                0.8,
                Some(0.0),
                Some(8.0),
                "Event gain control.",
                vec!["gain", "amp", "level"],
            ),
        }
    }
}

impl Piece for GainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct MirrorPiece {
    def: PieceDef,
}

impl MirrorPiece {
    pub fn new() -> Self {
        Self {
            def: pattern_only_transform(
                "cadence.mirror",
                "mirror",
                "Reflect the pattern within the current cycle.",
                vec!["rev", "reverse_time"],
            ),
        }
    }
}

impl Piece for MirrorPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct MaskPiece {
    def: PieceDef,
}

impl MaskPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.mask",
                "mask",
                PieceCategory::Transform,
                vec![
                    required_pattern_param(),
                    ParamDef {
                        id: "by".into(),
                        label: "by".into(),
                        side: TileSide::BOTTOM,
                        schema: pattern_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Gate the incoming pattern with a second pattern.",
                vec!["mask", "struct"],
            ),
        }
    }
}

impl Piece for MaskPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct SampleBankPiece {
    def: PieceDef,
}

impl SampleBankPiece {
    pub fn new() -> Self {
        Self {
            def: unary_text_transform(
                "cadence.sample_bank",
                "sample bank",
                "value",
                "value",
                "default",
                "Choose the sample bank to resolve against.",
                vec!["sample_bank", "bank", "library"],
            ),
        }
    }
}

impl Piece for SampleBankPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PlaybackStartPiece {
    def: PieceDef,
}

impl PlaybackStartPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.playback_start",
                "playback start",
                "value",
                "value",
                0.0,
                Some(0.0),
                Some(1.0),
                "Normalized playback start within the sample.",
                vec!["playback_start", "start", "offset"],
            ),
        }
    }
}

impl Piece for PlaybackStartPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PlaybackEndPiece {
    def: PieceDef,
}

impl PlaybackEndPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.playback_end",
                "playback end",
                "value",
                "value",
                1.0,
                Some(0.0),
                Some(1.0),
                "Normalized playback end within the sample.",
                vec!["playback_end", "end"],
            ),
        }
    }
}

impl Piece for PlaybackEndPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PanPiece {
    def: PieceDef,
}

impl PanPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.pan",
                "pan",
                "value",
                "value",
                0.0,
                Some(-1.0),
                Some(1.0),
                "Stereo position control.",
                vec!["pan"],
            ),
        }
    }
}

impl Piece for PanPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PlaybackRatePiece {
    def: PieceDef,
}

impl PlaybackRatePiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.playback_rate",
                "playback rate",
                "value",
                "value",
                1.0,
                Some(0.125),
                Some(8.0),
                "Sample playback rate control.",
                vec!["playback_rate", "rate"],
            ),
        }
    }
}

impl Piece for PlaybackRatePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct ClipPiece {
    def: PieceDef,
}

impl ClipPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.clip",
                "clip",
                "value",
                "value",
                1.0,
                Some(0.0),
                Some(16.0),
                "Event-relative clip length multiplier.",
                vec!["clip"],
            ),
        }
    }
}

impl Piece for ClipPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct AddPiece {
    def: PieceDef,
}

impl AddPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.add",
                "add",
                PieceCategory::Transform,
                vec![
                    required_pattern_param(),
                    script_text_param(
                        "value",
                        "value",
                        TileSide::BOTTOM,
                        "0",
                        "cadence_note_script",
                    ),
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Transpose pitch controls by a Terrance offset pattern.",
                vec!["add", "transpose"],
            ),
        }
    }
}

impl Piece for AddPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct ScalePiece {
    def: PieceDef,
}

impl ScalePiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.scale",
                "scale",
                PieceCategory::Transform,
                vec![
                    required_pattern_param(),
                    text_param("value", "value", TileSide::BOTTOM, "c4:major"),
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Resolve degree-like pitch controls against a named scale root and mode.",
                vec!["scale"],
            ),
        }
    }
}

impl Piece for ScalePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct JuxByPiece {
    def: PieceDef,
}

impl JuxByPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.jux_by",
                "jux by",
                PieceCategory::Transform,
                vec![
                    required_pattern_param(),
                    number_param(
                        "amount",
                        "amount",
                        TileSide::BOTTOM,
                        1.0,
                        Some(0.0),
                        Some(1.0),
                    ),
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Stereo mirror the pattern by overlaying the original and a mirrored copy with opposite pan.",
                vec!["jux_by", "stereo_mirror"],
            ),
        }
    }
}

impl Piece for JuxByPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct ReversePiece {
    def: PieceDef,
}

impl ReversePiece {
    pub fn new() -> Self {
        Self {
            def: pattern_only_transform(
                "cadence.reverse",
                "reverse",
                "Reverse the sample playback direction.",
                vec!["reverse", "sample_reverse"],
            ),
        }
    }
}

impl Piece for ReversePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct VelocityPiece {
    def: PieceDef,
}

impl VelocityPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.velocity",
                "velocity",
                "value",
                "value",
                1.0,
                Some(0.0),
                Some(1.0),
                "Velocity control.",
                vec!["velocity"],
            ),
        }
    }
}

impl Piece for VelocityPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct LegatoPiece {
    def: PieceDef,
}

impl LegatoPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.legato",
                "legato",
                "value",
                "value",
                1.0,
                Some(0.01),
                Some(8.0),
                "Legato control.",
                vec!["legato"],
            ),
        }
    }
}

impl Piece for LegatoPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct AttackPiece {
    def: PieceDef,
}

impl AttackPiece {
    pub fn new() -> Self {
        Self {
            def: envelope_transform(
                "cadence.attack",
                "attack",
                0.01,
                "Envelope attack in seconds.",
                vec!["attack"],
            ),
        }
    }
}

impl Piece for AttackPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct DecayPiece {
    def: PieceDef,
}

impl DecayPiece {
    pub fn new() -> Self {
        Self {
            def: envelope_transform(
                "cadence.decay",
                "decay",
                0.05,
                "Envelope decay in seconds.",
                vec!["decay"],
            ),
        }
    }
}

impl Piece for DecayPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct SustainPiece {
    def: PieceDef,
}

impl SustainPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.sustain",
                "sustain",
                "value",
                "value",
                0.8,
                Some(0.0),
                Some(1.0),
                "Envelope sustain level.",
                vec!["sustain", "sustain_level"],
            ),
        }
    }
}

impl Piece for SustainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct ReleasePiece {
    def: PieceDef,
}

impl ReleasePiece {
    pub fn new() -> Self {
        Self {
            def: envelope_transform(
                "cadence.release",
                "release",
                0.2,
                "Envelope release in seconds.",
                vec!["release"],
            ),
        }
    }
}

impl Piece for ReleasePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct LowPassCutoffPiece {
    def: PieceDef,
}

impl LowPassCutoffPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.lowpass_cutoff",
                "lowpass cutoff",
                "value",
                "hz",
                1200.0,
                Some(20.0),
                Some(20_000.0),
                "Low-pass cutoff in hertz.",
                vec!["lowpass_cutoff", "cutoff", "lpf", "lowpass"],
            ),
        }
    }
}

impl Piece for LowPassCutoffPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct HighPassCutoffPiece {
    def: PieceDef,
}

impl HighPassCutoffPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.highpass_cutoff",
                "highpass cutoff",
                "value",
                "hz",
                180.0,
                Some(20.0),
                Some(20_000.0),
                "High-pass cutoff in hertz.",
                vec!["highpass_cutoff", "hpf", "highpass"],
            ),
        }
    }
}

impl Piece for HighPassCutoffPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct LowPassResonancePiece {
    def: PieceDef,
}

impl LowPassResonancePiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.lowpass_resonance",
                "lowpass resonance",
                "value",
                "amount",
                0.0,
                Some(0.0),
                Some(1.0),
                "Low-pass resonance amount.",
                vec!["lowpass_resonance", "resonance", "q", "lpq"],
            ),
        }
    }
}

impl Piece for LowPassResonancePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct HighPassResonancePiece {
    def: PieceDef,
}

impl HighPassResonancePiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.highpass_resonance",
                "highpass resonance",
                "value",
                "amount",
                0.0,
                Some(0.0),
                Some(1.0),
                "High-pass resonance amount.",
                vec!["highpass_resonance", "highpass_q", "hpq"],
            ),
        }
    }
}

impl Piece for HighPassResonancePiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct ReverbSendPiece {
    def: PieceDef,
}

impl ReverbSendPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.reverb_send",
                "reverb send",
                PieceCategory::Transform,
                vec![required_pattern_param(), config_bundle_param()],
                Some(pattern_port()),
                Some(TileSide::LEFT),
                "Send the voice to the shared reverb bus.",
                vec!["reverb_send", "reverb", "verb"],
            ),
        }
    }
}

impl Piece for ReverbSendPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct DelayPiece {
    def: PieceDef,
}

impl DelayPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.delay",
                "delay",
                PieceCategory::Transform,
                vec![required_pattern_param(), config_bundle_param()],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Send the voice to the shared delay bus.",
                vec!["delay", "echo_send"],
            ),
        }
    }
}

impl Piece for DelayPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct CompressorPiece {
    def: PieceDef,
}

impl CompressorPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.compressor",
                "compressor",
                PieceCategory::Transform,
                vec![required_pattern_param(), config_bundle_param()],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Apply a per-event compressor profile.",
                vec!["compressor", "comp"],
            ),
        }
    }
}

impl Piece for CompressorPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct PostGainPiece {
    def: PieceDef,
}

impl PostGainPiece {
    pub fn new() -> Self {
        Self {
            def: unary_number_transform(
                "cadence.postgain",
                "postgain",
                "value",
                "value",
                1.0,
                Some(0.0),
                Some(8.0),
                "Output trim applied after the event-local signal chain.",
                vec!["postgain"],
            ),
        }
    }
}

impl Piece for PostGainPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct FastPiece {
    def: PieceDef,
}

impl FastPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.fast",
                "fast",
                PieceCategory::Transform,
                vec![
                    required_pattern_param(),
                    number_param(
                        "factor",
                        "factor",
                        TileSide::BOTTOM,
                        2.0,
                        Some(0.125),
                        Some(32.0),
                    ),
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Legacy alias for speed.",
                Vec::new(),
            ),
        }
    }
}

impl Piece for FastPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct SlowPiece {
    def: PieceDef,
}

impl SlowPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.slow",
                "slow",
                PieceCategory::Transform,
                vec![
                    required_pattern_param(),
                    number_param(
                        "factor",
                        "factor",
                        TileSide::BOTTOM,
                        2.0,
                        Some(0.125),
                        Some(32.0),
                    ),
                ],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Legacy alias for stretch.",
                Vec::new(),
            ),
        }
    }
}

impl Piece for SlowPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

pub struct RevPiece {
    def: PieceDef,
}

impl RevPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cadence.rev",
                "rev",
                PieceCategory::Transform,
                vec![required_pattern_param()],
                Some(pattern_port()),
                Some(TileSide::RIGHT),
                "Legacy alias for mirror.",
                Vec::new(),
            ),
        }
    }
}

impl Piece for RevPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }
}

fn required_pattern_param() -> ParamDef {
    ParamDef {
        id: "pattern".into(),
        label: "pattern".into(),
        side: TileSide::LEFT,
        schema: pattern_schema(),
        text_semantics: Default::default(),
        variadic_group: None,
        required: true,
        role: Default::default(),
    }
}

fn config_bundle_param() -> ParamDef {
    ParamDef {
        id: "config".into(),
        label: "config".into(),
        side: TileSide::BOTTOM,
        schema: ParamSchema::Custom {
            port_type: PortType::new("control_bundle"),
            value_kind: ParamValueKind::None,
            default: None,
            can_inline: false,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
        text_semantics: Default::default(),
        variadic_group: None,
        required: false,
        role: Default::default(),
    }
}

fn number_param(
    id: &str,
    label: &str,
    side: TileSide,
    default: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: label.into(),
        side,
        schema: ParamSchema::Number {
            default,
            min,
            max,
            can_inline: true,
        },
        text_semantics: Default::default(),
        variadic_group: None,
        required: false,
        role: Default::default(),
    }
}

fn number_or_pattern_param(
    id: &str,
    label: &str,
    side: TileSide,
    default: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: label.into(),
        side,
        schema: ParamSchema::Custom {
            port_type: PortType::number(),
            value_kind: ParamValueKind::Number,
            default: Some(serde_json::json!(default)),
            can_inline: true,
            inline_mode: ParamInlineMode::Literal,
            min,
            max,
        },
        text_semantics: Default::default(),
        variadic_group: None,
        required: false,
        role: Default::default(),
    }
}

fn text_param(id: &str, label: &str, side: TileSide, default: &str) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: label.into(),
        side,
        schema: ParamSchema::Text {
            default: default.into(),
            can_inline: true,
        },
        text_semantics: Default::default(),
        variadic_group: None,
        required: false,
        role: Default::default(),
    }
}

fn script_text_param(
    id: &str,
    label: &str,
    side: TileSide,
    default: &str,
    semantics: &str,
) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: label.into(),
        side,
        schema: ParamSchema::Text {
            default: default.into(),
            can_inline: true,
        },
        text_semantics: ParamTextSemantics::new(semantics),
        variadic_group: None,
        required: false,
        role: Default::default(),
    }
}

fn unary_number_transform(
    id: &str,
    label: &str,
    param_id: &str,
    param_label: &str,
    default: f64,
    min: Option<f64>,
    max: Option<f64>,
    description: &str,
    tags: Vec<&str>,
) -> PieceDef {
    cadence_piece_def_with_tags(
        id,
        label,
        PieceCategory::Transform,
        vec![
            required_pattern_param(),
            number_or_pattern_param(param_id, param_label, TileSide::BOTTOM, default, min, max),
        ],
        Some(pattern_port()),
        Some(TileSide::RIGHT),
        description,
        tags,
    )
}

fn unary_text_transform(
    id: &str,
    label: &str,
    param_id: &str,
    param_label: &str,
    default: &str,
    description: &str,
    tags: Vec<&str>,
) -> PieceDef {
    cadence_piece_def_with_tags(
        id,
        label,
        PieceCategory::Transform,
        vec![
            required_pattern_param(),
            text_param(param_id, param_label, TileSide::BOTTOM, default),
        ],
        Some(pattern_port()),
        Some(TileSide::RIGHT),
        description,
        tags,
    )
}

fn pattern_only_transform(id: &str, label: &str, description: &str, tags: Vec<&str>) -> PieceDef {
    cadence_piece_def_with_tags(
        id,
        label,
        PieceCategory::Transform,
        vec![required_pattern_param()],
        Some(pattern_port()),
        Some(TileSide::RIGHT),
        description,
        tags,
    )
}

fn envelope_transform(
    id: &str,
    label: &str,
    default: f64,
    description: &str,
    tags: Vec<&str>,
) -> PieceDef {
    unary_number_transform(
        id,
        label,
        "value",
        "seconds",
        default,
        Some(0.0),
        Some(8.0),
        description,
        tags,
    )
}

macro_rules! impl_default_from_new {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Default for $ty {
                fn default() -> Self {
                    Self::new()
                }
            }
        )*
    };
}

impl_default_from_new!(
    SpeedPiece,
    StretchPiece,
    GainPiece,
    MirrorPiece,
    MaskPiece,
    SampleBankPiece,
    PlaybackStartPiece,
    PlaybackEndPiece,
    PanPiece,
    PlaybackRatePiece,
    ClipPiece,
    AddPiece,
    ScalePiece,
    JuxByPiece,
    ReversePiece,
    VelocityPiece,
    LegatoPiece,
    AttackPiece,
    DecayPiece,
    SustainPiece,
    ReleasePiece,
    LowPassCutoffPiece,
    LowPassResonancePiece,
    HighPassCutoffPiece,
    HighPassResonancePiece,
    ReverbSendPiece,
    DelayPiece,
    CompressorPiece,
    PostGainPiece,
    FastPiece,
    SlowPiece,
    RevPiece,
);

#[cfg(test)]
mod tests {
    use super::{
        AddPiece, CompressorPiece, HighPassResonancePiece, LowPassCutoffPiece,
        LowPassResonancePiece, Piece, PlaybackEndPiece, PlaybackStartPiece, ReverbSendPiece,
        ScalePiece, SpeedPiece,
    };
    use tessera::types::TileSide;

    #[test]
    fn canonical_tiles_expose_alias_tags() {
        let piece = SpeedPiece::new();
        assert!(piece.def().tags.iter().any(|tag| tag == "fast"));
        let add = AddPiece::new();
        assert!(
            add.def()
                .params
                .iter()
                .find(|param| param.id == "value")
                .is_some_and(|param| param.text_semantics.as_str() == "cadence_note_script")
        );
        let scale = ScalePiece::new();
        assert!(scale.def().tags.iter().any(|tag| tag == "scale"));
        let lowpass = LowPassCutoffPiece::new();
        assert!(lowpass.def().tags.iter().any(|tag| tag == "lpf"));
        let resonance = LowPassResonancePiece::new();
        assert!(resonance.def().tags.iter().any(|tag| tag == "q"));
        let highpass_resonance = HighPassResonancePiece::new();
        assert!(highpass_resonance.def().tags.iter().any(|tag| tag == "hpq"));
    }

    #[test]
    fn playback_boundary_pieces_expose_scalar_controls() {
        let start = PlaybackStartPiece::new();
        let end = PlaybackEndPiece::new();
        assert!(start.def().params.iter().any(|param| param.id == "value"));
        assert!(end.def().params.iter().any(|param| param.id == "value"));
    }

    #[test]
    fn reverb_and_compressor_tiles_expose_structured_controls() {
        let room = ReverbSendPiece::new();
        let compressor = CompressorPiece::new();
        let config_param = room
            .def()
            .params
            .iter()
            .find(|param| param.id == "config")
            .expect("reverb send should expose structured config");
        assert_eq!(config_param.side, TileSide::BOTTOM);
        assert!(
            room.def()
                .params
                .iter()
                .all(|param| !matches!(param.id.as_str(), "amount" | "decay" | "damping"))
        );
        assert!(
            compressor
                .def()
                .params
                .iter()
                .any(|param| param.id == "config")
        );
    }
}
