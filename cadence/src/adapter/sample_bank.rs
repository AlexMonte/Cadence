//! In-memory decoded sample bank for the active runtime path.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    adapter::audio::SampleBuffer, application::sample::SampleTrigger, domain::control::Symbol,
};

/// Runtime choke behavior that the active audio adapter understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChokeGroup {
    /// Shared choke group for hat-style samples.
    Hat,
}

/// Optional playback hints carried alongside a decoded sample.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SampleLoadOptions {
    /// Optional playback limit to apply after decoding.
    pub playback_limit: Option<Duration>,
    /// Optional choke group for mixer behavior.
    pub choke_group: Option<ChokeGroup>,
    /// Optional named bank for variant selection.
    pub bank: Option<Symbol>,
    /// Optional root pitch used for pitch-aware variant selection.
    pub root_pitch: Option<f64>,
    /// Optional pitch zone for this decoded sample.
    pub pitch_range: Option<SamplePitchRange>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Inclusive pitch zone for a decoded sample variant.
pub struct SamplePitchRange {
    low: f64,
    high: f64,
}

impl SamplePitchRange {
    /// Creates a pitch range.
    ///
    /// Returns `None` when either bound is non-finite or `low > high`.
    #[must_use]
    pub fn new(low: f64, high: f64) -> Option<Self> {
        (low.is_finite() && high.is_finite() && low <= high).then_some(Self { low, high })
    }

    /// Returns `true` when `pitch` lies inside the inclusive range.
    #[must_use]
    pub fn contains(self, pitch: f64) -> bool {
        (self.low..=self.high).contains(&pitch)
    }
}

impl Eq for SamplePitchRange {}

/// Decoded sample entry stored in the active runtime bank.
#[derive(Debug, Clone)]
pub struct LoadedSample {
    /// Exact sample key used to retrieve this variant.
    pub sample_key: String,
    /// Decoded audio payload.
    pub sample: Arc<SampleBuffer>,
    /// Optional playback limit.
    pub playback_limit: Option<Duration>,
    /// Optional choke group.
    pub choke_group: Option<ChokeGroup>,
    /// Optional named bank.
    pub bank: Option<Symbol>,
    /// Optional root pitch used for pitch-aware matching.
    pub root_pitch: Option<f64>,
    /// Optional pitch zone.
    pub pitch_range: Option<SamplePitchRange>,
}

/// Typed trigger plus resolved decoded sample payload for the active audio path.
#[derive(Debug, Clone)]
pub struct LoadedSampleTrigger {
    /// Trigger settings after resolution.
    pub trigger: SampleTrigger,
    /// Exact sample key that was resolved.
    pub sample_key: String,
    /// Decoded audio payload.
    pub sample: Arc<SampleBuffer>,
    /// Optional playback limit.
    pub playback_limit: Option<Duration>,
    /// Optional choke group for mixer behavior.
    pub choke_group: Option<ChokeGroup>,
}

#[derive(Debug, Clone, Default)]
/// Thread-safe decoded sample store used by the active runtime path.
pub struct SampleBank {
    samples: Arc<RwLock<HashMap<String, Vec<LoadedSample>>>>,
}

impl SampleBank {
    /// Creates an empty decoded-sample store for the active runtime path.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or overwrites a decoded sample by exact key.
    pub fn load(&self, name: impl Into<String>, audio: SampleBuffer) {
        self.load_with_options(name, audio, SampleLoadOptions::default());
    }

    /// Inserts or overwrites a decoded sample with extra playback hints.
    pub fn load_with_options(
        &self,
        name: impl Into<String>,
        audio: SampleBuffer,
        options: SampleLoadOptions,
    ) {
        let key = name.into();
        let loaded = LoadedSample {
            sample_key: key.clone(),
            sample: Arc::new(audio),
            playback_limit: options.playback_limit,
            choke_group: options.choke_group,
            bank: options.bank,
            root_pitch: options.root_pitch,
            pitch_range: options.pitch_range,
        };

        let mut samples = self
            .samples
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entries = samples.entry(key).or_default();
        entries.retain(|entry| {
            !(entry.bank == loaded.bank
                && entry.root_pitch == loaded.root_pitch
                && entry.pitch_range == loaded.pitch_range)
        });
        entries.push(loaded);
    }

    /// Returns `true` when a decoded sample exists for the exact key.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        let samples = self
            .samples
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        samples.get(name).is_some_and(|entries| !entries.is_empty())
    }

    #[cfg(test)]
    pub(crate) fn get(&self, name: &str) -> Option<LoadedSample> {
        let samples = self
            .samples
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        samples.get(name)?.first().cloned()
    }

    pub(crate) fn resolve_trigger(&self, trigger: &SampleTrigger) -> Option<LoadedSampleTrigger> {
        let samples = self
            .samples
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let variants = samples.get(&trigger.sample)?;
        let loaded = select_variant(variants, trigger)?;
        let mut resolved_trigger = trigger.clone();

        if let (Some(target_pitch), Some(root_pitch)) = (trigger.pitch, loaded.root_pitch) {
            resolved_trigger.playback_rate *= pitch_ratio(target_pitch - root_pitch);
        }

        Some(LoadedSampleTrigger {
            trigger: resolved_trigger,
            sample_key: loaded.sample_key.clone(),
            sample: Arc::clone(&loaded.sample),
            playback_limit: loaded.playback_limit,
            choke_group: loaded.choke_group,
        })
    }
}

fn select_variant<'a>(
    variants: &'a [LoadedSample],
    trigger: &SampleTrigger,
) -> Option<&'a LoadedSample> {
    let bank_candidates: Vec<&LoadedSample> = match &trigger.sample_bank {
        Some(bank) => variants
            .iter()
            .filter(|sample| sample.bank.as_ref() == Some(bank))
            .collect(),
        None => variants.iter().collect(),
    };

    if bank_candidates.is_empty() {
        return None;
    }

    if let Some(sample_variant) = trigger.sample_variant {
        return bank_candidates
            .get(sample_variant % bank_candidates.len())
            .copied();
    }

    let selected = if let Some(pitch) = trigger.pitch {
        let zoned: Vec<&LoadedSample> = bank_candidates
            .iter()
            .copied()
            .filter(|sample| {
                sample
                    .pitch_range
                    .is_some_and(|range| range.contains(pitch))
            })
            .collect();
        let candidates = if zoned.is_empty() {
            bank_candidates
        } else {
            zoned
        };

        candidates.into_iter().min_by(|left, right| {
            pitch_distance(left, pitch)
                .partial_cmp(&pitch_distance(right, pitch))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    } else {
        bank_candidates
            .into_iter()
            .find(|sample| sample.bank.is_none() && sample.root_pitch.is_none())
            .or_else(|| variants.first())
    };

    selected
}

fn pitch_distance(sample: &LoadedSample, target_pitch: f64) -> f64 {
    sample
        .root_pitch
        .map(|root| (target_pitch - root).abs())
        .unwrap_or(f64::INFINITY)
}

fn pitch_ratio(delta_semitones: f64) -> f64 {
    2_f64.powf(delta_semitones / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::audio::Frame;

    fn sample(values: &[f32], sample_rate: u32) -> SampleBuffer {
        SampleBuffer::new(
            sample_rate,
            values
                .iter()
                .copied()
                .map(Frame::from_mono)
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn load_registers_sample_by_exact_name() {
        let bank = SampleBank::new();

        bank.load("bd", sample(&[0.25, 0.5], 4));

        assert!(bank.contains("bd"));
        assert_eq!(bank.get("bd").unwrap().sample.len(), 2);
    }

    #[test]
    fn load_overwrites_existing_sample() {
        let bank = SampleBank::new();

        bank.load("bd", sample(&[0.25], 4));
        bank.load("bd", sample(&[0.1, 0.2, 0.3], 4));

        assert_eq!(bank.get("bd").unwrap().sample.len(), 3);
    }

    #[test]
    fn load_with_same_bank_and_zone_overwrites_matching_variant() {
        let bank = SampleBank::new();

        bank.load_with_options(
            "bd",
            sample(&[0.25], 4),
            SampleLoadOptions {
                bank: Some(Symbol::from("808")),
                root_pitch: Some(36.0),
                pitch_range: Some(SamplePitchRange::new(30.0, 42.0).unwrap()),
                ..SampleLoadOptions::default()
            },
        );
        bank.load_with_options(
            "bd",
            sample(&[0.5, 0.75], 4),
            SampleLoadOptions {
                bank: Some(Symbol::from("808")),
                root_pitch: Some(36.0),
                pitch_range: Some(SamplePitchRange::new(30.0, 42.0).unwrap()),
                ..SampleLoadOptions::default()
            },
        );

        let samples = bank
            .samples
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(samples.get("bd").unwrap().len(), 1);
    }

    #[test]
    fn load_with_options_preserves_playback_hints() {
        let bank = SampleBank::new();

        bank.load_with_options(
            "hat",
            sample(&[0.25], 4),
            SampleLoadOptions {
                playback_limit: Some(Duration::from_millis(110)),
                choke_group: Some(ChokeGroup::Hat),
                ..SampleLoadOptions::default()
            },
        );

        let loaded = bank.get("hat").unwrap();
        assert_eq!(loaded.playback_limit, Some(Duration::from_millis(110)));
        assert_eq!(loaded.choke_group, Some(ChokeGroup::Hat));
    }

    #[test]
    fn resolve_trigger_uses_exact_sample_name() {
        let bank = SampleBank::new();
        bank.load("kick", sample(&[0.25], 4));
        let trigger = SampleTrigger::builder()
            .sample("kick")
            .play_for(Duration::ZERO)
            .build()
            .unwrap();

        let resolved = bank.resolve_trigger(&trigger).unwrap();

        assert_eq!(resolved.sample_key, "kick");
        assert_eq!(resolved.sample.len(), 1);
    }

    #[test]
    fn resolve_trigger_prefers_matching_bank_and_pitch_zone() {
        let bank = SampleBank::new();
        bank.load_with_options(
            "kalimba",
            sample(&[0.1], 4),
            SampleLoadOptions {
                bank: Some(Symbol::from("gm")),
                root_pitch: Some(60.0),
                pitch_range: Some(SamplePitchRange::new(57.0, 63.0).unwrap()),
                ..SampleLoadOptions::default()
            },
        );
        bank.load_with_options(
            "kalimba",
            sample(&[0.2], 4),
            SampleLoadOptions {
                bank: Some(Symbol::from("gm")),
                root_pitch: Some(72.0),
                pitch_range: Some(SamplePitchRange::new(69.0, 75.0).unwrap()),
                ..SampleLoadOptions::default()
            },
        );

        let trigger = SampleTrigger::builder()
            .sample("kalimba")
            .sample_bank(Symbol::from("gm"))
            .pitch(61.0)
            .play_for(Duration::ZERO)
            .build()
            .unwrap();

        let resolved = bank.resolve_trigger(&trigger).unwrap();

        assert_eq!(resolved.sample.frame(0), Some(Frame::from_mono(0.1)));
    }

    #[test]
    fn resolve_trigger_transposes_from_closest_root_pitch_when_zone_missing() {
        let bank = SampleBank::new();
        bank.load_with_options(
            "bass",
            sample(&[0.25], 4),
            SampleLoadOptions {
                root_pitch: Some(48.0),
                ..SampleLoadOptions::default()
            },
        );

        let trigger = SampleTrigger::builder()
            .sample("bass")
            .pitch(60.0)
            .play_for(Duration::ZERO)
            .build()
            .unwrap();

        let resolved = bank.resolve_trigger(&trigger).unwrap();

        assert!(resolved.trigger.playback_rate > 1.9);
    }

    #[test]
    fn resolve_trigger_wraps_explicit_sample_variant_within_bank_matches() {
        let bank = SampleBank::new();
        bank.load_with_options(
            "kalimba",
            sample(&[0.1], 4),
            SampleLoadOptions {
                bank: Some(Symbol::from("gm")),
                ..SampleLoadOptions::default()
            },
        );
        bank.load_with_options(
            "kalimba",
            sample(&[0.2], 4),
            SampleLoadOptions {
                bank: Some(Symbol::from("gm")),
                ..SampleLoadOptions::default()
            },
        );

        let trigger = SampleTrigger::builder()
            .sample("kalimba")
            .sample_bank(Symbol::from("gm"))
            .sample_variant(3)
            .play_for(Duration::ZERO)
            .build()
            .unwrap();

        let resolved = bank.resolve_trigger(&trigger).unwrap();

        assert_eq!(resolved.sample.frame(0), Some(Frame::from_mono(0.2)));
    }
}
