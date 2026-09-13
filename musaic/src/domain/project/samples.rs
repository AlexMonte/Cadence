//! Durable identities and metadata for project-owned WAV assets.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Identity within a project; independent of its folder, display name, or bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SampleId(pub u64);

impl SampleId {
    pub fn runtime_name(self) -> String {
        format!("project-sample-{:016x}", self.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SampleImportOptions {
    /// MIDI pitch of the original recording; absent for unpitched sounds.
    pub root_pitch: Option<f64>,
    /// Optional linear gain applied by the host when selecting this sample.
    pub default_gain: Option<f32>,
    /// Play the attack once, then repeat these source frames while the note sounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sustain_loop: Option<SampleLoopRegion>,
}

/// Half-open source-frame boundaries, independent of output rate and note pitch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleLoopRegion {
    pub start_frame: u64,
    pub end_frame: u64,
}

impl SampleImportOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .root_pitch
            .is_some_and(|pitch| !pitch.is_finite() || !(0.0..=127.0).contains(&pitch))
        {
            return Err("Sample root pitch must be a MIDI pitch from 0 to 127".into());
        }
        if self
            .default_gain
            .is_some_and(|gain| !gain.is_finite() || !(0.0..=16.0).contains(&gain))
        {
            return Err("Sample default gain must be finite and between 0 and 16".into());
        }
        if self
            .sustain_loop
            .is_some_and(|region| region.start_frame >= region.end_frame)
        {
            return Err("Sustain loop start must precede its end frame".into());
        }
        Ok(())
    }

    pub fn validate_frame_count(&self, frame_count: u64) -> Result<(), String> {
        self.validate()?;
        if self
            .sustain_loop
            .is_some_and(|region| region.end_frame > frame_count)
        {
            return Err("Sustain loop must lie inside the recording".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleMetadata {
    pub source_name: String,
    /// Forward-slash path relative to the project file's directory.
    pub relative_path: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_count: u64,
    pub byte_length: u64,
    /// FNV-1a checksum for accidental corruption detection, not authentication.
    pub checksum: String,
    #[serde(default)]
    pub options: SampleImportOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleManifest {
    /// Never reuse an ID after an asset has been removed.
    pub next_id: u64,
    pub samples: BTreeMap<SampleId, SampleMetadata>,
    /// Imported recordings grouped under the lead sample's existing identity.
    #[serde(default)]
    pub banks: BTreeMap<SampleId, SampleBankDefinition>,
}

impl Default for SampleManifest {
    fn default() -> Self {
        Self {
            next_id: 1,
            samples: BTreeMap::new(),
            banks: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplePitchZone {
    pub low: f64,
    pub high: f64,
}
// Validation admits finite MIDI pitches only.
impl Eq for SamplePitchZone {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleVariant {
    pub sample: SampleId,
    #[serde(default)]
    pub bank: Option<String>,
    #[serde(default)]
    pub pitch_zone: Option<SamplePitchZone>,
    #[serde(default)]
    pub playback_limit_ms: Option<u32>,
    #[serde(default)]
    pub hat_choke: bool,
}

impl SampleVariant {
    pub fn new(sample: SampleId) -> Self {
        Self {
            sample,
            bank: None,
            pitch_zone: None,
            playback_limit_ms: None,
            hat_choke: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleBankDefinition {
    /// Stable explicit ordering; the lead is first and each recording appears once.
    pub variants: Vec<SampleVariant>,
}

impl SampleBankDefinition {
    pub fn new(lead: SampleId) -> Self {
        Self {
            variants: vec![SampleVariant::new(lead)],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleDiagnosticKind {
    Unresolved,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleDiagnostic {
    pub sample: SampleId,
    pub source_name: String,
    pub relative_path: String,
    pub kind: SampleDiagnosticKind,
    pub message: String,
}
