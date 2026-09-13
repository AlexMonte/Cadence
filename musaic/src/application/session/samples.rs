//! Project-local sample state. The command dispatcher owns mutation; persistence
//! adapters prepare and validate imports before publishing a complete asset here.

use std::{collections::BTreeMap, sync::Arc};

use cadence::infrastructure::audio::SampleBuffer;

use crate::domain::project::samples::{SampleDiagnostic, SampleId, SampleManifest, SampleMetadata};

/// In-memory undo checkpoint. Decoded buffers and original WAV bytes are shared,
/// so relink undo never rereads a mutable external file.
#[derive(Debug, Clone)]
pub struct SampleAssetSnapshot {
    pub(crate) metadata: SampleMetadata,
    pub(crate) loaded: Option<LoadedProjectSample>,
    pub(crate) diagnostics: Vec<SampleDiagnostic>,
}

impl PartialEq for SampleAssetSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata
            && self.diagnostics == other.diagnostics
            && self.loaded.as_ref().map(|value| value.wav_bytes.as_ref())
                == other.loaded.as_ref().map(|value| value.wav_bytes.as_ref())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedProjectSample {
    pub wav_bytes: Arc<[u8]>,
    pub buffer: SampleBuffer,
    pub peaks: Arc<[u16]>,
}

impl LoadedProjectSample {
    pub(crate) fn new(wav_bytes: Arc<[u8]>, buffer: SampleBuffer) -> Self {
        const BINS: usize = 96;
        let count = buffer.len().min(BINS);
        let peaks = (0..count)
            .map(|index| {
                let start = index * buffer.len() / count;
                let end = (index + 1) * buffer.len() / count;
                let peak = buffer.frames()[start..end]
                    .iter()
                    .map(|frame| frame.left.abs().max(frame.right.abs()))
                    .fold(0.0_f32, f32::max);
                (peak.clamp(0.0, 1.0) * u16::MAX as f32).round() as u16
            })
            .collect::<Vec<_>>()
            .into();
        Self {
            wav_bytes,
            buffer,
            peaks,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectSamples {
    pub(crate) manifest: SampleManifest,
    pub(crate) loaded: BTreeMap<SampleId, LoadedProjectSample>,
    pub(crate) diagnostics: Vec<SampleDiagnostic>,
    pub(crate) revision: u64,
    /// Asset replacement tokens exclude metadata-only edits during decode.
    pub(crate) asset_revisions: BTreeMap<SampleId, u64>,
}

impl ProjectSamples {
    pub fn asset_revision(&self, id: SampleId) -> u64 {
        self.asset_revisions.get(&id).copied().unwrap_or(0)
    }

    pub(crate) fn asset_changed(&mut self, id: SampleId) {
        let revision = self.asset_revisions.entry(id).or_default();
        *revision = revision.wrapping_add(1);
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn manifest(&self) -> &SampleManifest {
        &self.manifest
    }

    /// Compare this and the manifest to avoid reloading audio for unrelated edits.
    /// A project replacement still resets the host's sample bindings.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn decoded(&self, id: SampleId) -> Option<&SampleBuffer> {
        self.loaded.get(&id).map(|sample| &sample.buffer)
    }

    pub fn diagnostics(&self) -> &[SampleDiagnostic] {
        &self.diagnostics
    }

    /// At most 96 amplitude bins, computed once when decoded audio is adopted.
    pub fn waveform_peaks(&self, id: SampleId) -> Option<&[u16]> {
        self.loaded.get(&id).map(|sample| sample.peaks.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadence::adapter::audio::Frame;

    #[test]
    fn waveform_cache_is_bounded_and_keeps_a_final_stereo_transient() {
        let mut frames = vec![Frame::ZERO; 1_001];
        frames[1_000].right = 1.0;
        let sample = LoadedProjectSample::new(Arc::from([]), SampleBuffer::new(8_000, frames));
        assert_eq!(sample.peaks.len(), 96);
        assert_eq!(sample.peaks[95], u16::MAX);
        assert!(sample.peaks[..95].iter().all(|peak| *peak == 0));
    }
}
