//! Host-owned sound assignments. Tessera keeps notes independent of their sound.

use crate::domain::project::samples::SampleId;
use serde::{Deserialize, Serialize};
use tessera::prelude::Rational;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Waveform {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
    Noise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SynthPreset {
    Bass,
    Pad,
    Percussion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentSource {
    Synth(Waveform),
    Preset(SynthPreset),
    Sample(SampleId),
    /// A single connected sequence may select any built-in named drum hit.
    Kit,
    /// Stable names from the built-in percussion kit.
    Drum(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentDefinition {
    pub source: InstrumentSource,
    pub rate: Rational,
    pub start: Rational,
    pub end: Rational,
    pub reverse: bool,
    pub sound: InstrumentSound,
}

impl Default for InstrumentDefinition {
    fn default() -> Self {
        Self::new(InstrumentSource::Synth(Waveform::Sine))
    }
}

impl InstrumentDefinition {
    pub fn new(source: InstrumentSource) -> Self {
        Self {
            source,
            rate: Rational::one(),
            start: Rational::zero(),
            end: Rational::one(),
            reverse: false,
            sound: InstrumentSound::default(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.sound.validate()?;
        let value = |r: Rational| -> Option<f64> {
            (r.denominator > 0).then_some(r.numerator as f64 / r.denominator as f64)
        };
        let rate = value(self.rate).ok_or("Rate needs a positive denominator")?;
        let start = value(self.start).ok_or("Start needs a positive denominator")?;
        let end = value(self.end).ok_or("End needs a positive denominator")?;
        if !(0.001..=64.0).contains(&rate) {
            return Err("Sample speed must be between 0.001 and 64".into());
        }
        if !(0.0..1.0).contains(&start) || end <= start || end > 1.0 {
            return Err("Sample region must satisfy 0 ≤ start < end ≤ 1".into());
        }
        if let InstrumentSource::Drum(name) = &self.source {
            if !["bd", "sd", "hh", "oh"].contains(&name.as_str()) {
                return Err("Unknown built-in drum".into());
            }
        }
        Ok(())
    }

    /// Effective envelope shown before the user makes a custom edit.
    pub fn effective_envelope(&self) -> InstrumentEnvelope {
        if let Some(envelope) = &self.sound.envelope {
            return envelope.clone();
        }
        match self.source {
            InstrumentSource::Preset(SynthPreset::Bass) => InstrumentEnvelope {
                attack: Rational::new(4, 1000),
                decay: Rational::new(12, 100),
                sustain: Rational::new(55, 100),
                release: Rational::new(6, 100),
            },
            InstrumentSource::Preset(SynthPreset::Pad) => InstrumentEnvelope {
                attack: Rational::new(15, 100),
                decay: Rational::new(2, 10),
                sustain: Rational::new(65, 100),
                release: Rational::new(4, 10),
            },
            InstrumentSource::Preset(SynthPreset::Percussion) => InstrumentEnvelope {
                attack: Rational::new(1, 1000),
                decay: Rational::new(12, 100),
                sustain: Rational::zero(),
                release: Rational::new(15, 1000),
            },
            _ => InstrumentEnvelope::default(),
        }
    }
    pub fn effective_gain(&self) -> Rational {
        self.sound.gain.unwrap_or(match self.source {
            InstrumentSource::Preset(SynthPreset::Bass | SynthPreset::Percussion) => {
                Rational::new(3, 10)
            }
            InstrumentSource::Preset(SynthPreset::Pad) => Rational::new(1, 4),
            _ => Rational::one(),
        })
    }

    pub fn intent(&self) -> Result<cadence::prelude::Intent, String> {
        use cadence::prelude::{BuiltInSynthSource, Intent, SampleIntent};
        self.validate()?;
        let mut intent = match &self.source {
            InstrumentSource::Synth(waveform) => Intent::synth(match waveform {
                Waveform::Sine => BuiltInSynthSource::Sine,
                Waveform::Triangle => BuiltInSynthSource::Triangle,
                Waveform::Saw => BuiltInSynthSource::Saw,
                Waveform::Square => BuiltInSynthSource::Square,
                Waveform::Noise => BuiltInSynthSource::Noise,
            }),
            InstrumentSource::Preset(preset) => Intent::synth_preset(match preset {
                SynthPreset::Bass => cadence::prelude::SynthPreset::Bass,
                SynthPreset::Pad => cadence::prelude::SynthPreset::Pad,
                SynthPreset::Percussion => cadence::prelude::SynthPreset::Percussion,
            }),
            source => {
                let name = match source {
                    InstrumentSource::Sample(id) => id.runtime_name(),
                    InstrumentSource::Kit => "bd".into(),
                    InstrumentSource::Drum(name) => name.clone(),
                    _ => unreachable!(),
                };
                Intent::Sample(
                    SampleIntent::new(name)
                        .rate(ratio(self.rate)?)
                        .region(ratio(self.start)?, ratio(self.end)?)
                        .reverse(self.reverse),
                )
            }
        };
        match &mut intent {
            Intent::Synth(source) => {
                source.inserts = self.sound.inserts()?;
                source.sound_defaults = self.sound.defaults()?;
            }
            Intent::Sample(source) => {
                source.inserts = self.sound.inserts()?;
                source.sound_defaults = self.sound.defaults()?;
            }
            _ => unreachable!(),
        }
        Ok(intent)
    }
}

/// Saved envelope times are seconds; sustain is a linear level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentEnvelope {
    pub attack: Rational,
    pub decay: Rational,
    pub sustain: Rational,
    pub release: Rational,
}
impl Default for InstrumentEnvelope {
    fn default() -> Self {
        Self {
            attack: Rational::zero(),
            decay: Rational::zero(),
            sustain: Rational::one(),
            release: Rational::zero(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentDelay {
    pub amount: Rational,
    pub time: Rational,
    pub feedback: Rational,
    pub damping: Rational,
}
impl Default for InstrumentDelay {
    fn default() -> Self {
        Self {
            amount: Rational::new(1, 4),
            time: Rational::new(1, 4),
            feedback: Rational::new(1, 3),
            damping: Rational::new(1, 2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentReverb {
    pub amount: Rational,
    pub decay: Rational,
    pub damping: Rational,
}
impl Default for InstrumentReverb {
    fn default() -> Self {
        Self {
            amount: Rational::new(1, 4),
            decay: Rational::new(3, 2),
            damping: Rational::new(1, 2),
        }
    }
}

/// Per-voice dynamics, before the ordered effect chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentCompressor {
    pub threshold: Rational,
    pub ratio: Rational,
    /// Soft-knee width in decibels.
    pub knee_db: Rational,
    pub attack: Rational,
    pub release: Rational,
}
impl Default for InstrumentCompressor {
    fn default() -> Self {
        Self {
            knee_db: Rational::zero(),
            threshold: Rational::new(1, 2),
            ratio: Rational::new(4, 1),
            attack: Rational::new(1, 100),
            release: Rational::new(1, 10),
        }
    }
}

/// Sequence order is audio processing order, preserved through save and undo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentEffect {
    LowPass {
        cutoff_hz: Rational,
        resonance: Rational,
    },
    HighPass {
        cutoff_hz: Rational,
        resonance: Rational,
    },
    Drive {
        amount: Rational,
        wet: Rational,
        output_gain: Rational,
    },
}
impl InstrumentEffect {
    pub fn intent(&self) -> Result<cadence::prelude::InsertEffect, String> {
        use cadence::prelude::InsertEffect;
        match self {
            Self::LowPass {
                cutoff_hz,
                resonance,
            } => InsertEffect::low_pass(ratio(*cutoff_hz)?, ratio(*resonance)?),
            Self::HighPass {
                cutoff_hz,
                resonance,
            } => InsertEffect::high_pass(ratio(*cutoff_hz)?, ratio(*resonance)?),
            Self::Drive {
                amount,
                wet,
                output_gain,
            } => InsertEffect::drive(ratio(*amount)?, ratio(*wet)?, ratio(*output_gain)?),
        }
        .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct InstrumentSound {
    pub envelope: Option<InstrumentEnvelope>,
    pub gain: Option<Rational>,
    pub delay: Option<InstrumentDelay>,
    pub reverb: Option<InstrumentReverb>,
    pub compressor: Option<InstrumentCompressor>,
    pub effects: Vec<InstrumentEffect>,
}
impl InstrumentSound {
    pub fn defaults(&self) -> Result<cadence::prelude::SoundDefaults, String> {
        use cadence::prelude::{
            CompressorSettings, DelaySettings, EnvelopeDefaults, ReverbSettings, SoundDefaults,
            UnitValue,
        };
        use std::time::Duration;
        let unit =
            |value| bounded(value, 0.0, 1.0, "Level").map(|value| UnitValue::new(value).unwrap());
        let duration = |value, min| bounded(value, min, 60.0, "Time").map(Duration::from_secs_f64);
        let mut defaults = SoundDefaults::default();
        if let Some(envelope) = &self.envelope {
            defaults = defaults.with_envelope(EnvelopeDefaults::new(
                duration(envelope.attack, 0.0)?,
                duration(envelope.decay, 0.0)?,
                unit(envelope.sustain)?,
                duration(envelope.release, 0.0)?,
            ));
        }
        if let Some(gain) = self.gain {
            defaults = defaults
                .with_gain(bounded(gain, 0.0, 4.0, "Gain")?)
                .map_err(str::to_string)?;
        }
        if let Some(delay) = &self.delay {
            defaults = defaults.with_delay(DelaySettings::new(
                unit(delay.amount)?,
                Duration::from_secs_f64(bounded(delay.time, 0.001, 2.0, "Delay time")?),
                unit(delay.feedback)?,
                unit(delay.damping)?,
            ));
        }
        if let Some(reverb) = &self.reverb {
            defaults = defaults.with_reverb(ReverbSettings::new(
                unit(reverb.amount)?,
                duration(reverb.decay, 0.001)?,
                unit(reverb.damping)?,
            ));
        }
        if let Some(compressor) = &self.compressor {
            defaults = defaults.with_compressor(
                CompressorSettings::new(
                    unit(compressor.threshold)?,
                    bounded(compressor.ratio, 1.0, 20.0, "Compression ratio")?,
                    duration(compressor.attack, 0.001)?,
                    duration(compressor.release, 0.001)?,
                )
                .with_knee_db(bounded(
                    compressor.knee_db,
                    0.0,
                    40.0,
                    "Compression knee",
                )?),
            );
        }
        Ok(defaults)
    }
    pub fn inserts(&self) -> Result<cadence::prelude::InsertChain, String> {
        cadence::prelude::InsertChain::new(
            self.effects
                .iter()
                .map(InstrumentEffect::intent)
                .collect::<Result<_, _>>()?,
        )
        .map_err(|error| error.to_string())
    }
    pub fn validate(&self) -> Result<(), String> {
        self.defaults()?;
        self.inserts()?;
        Ok(())
    }
}
fn ratio(value: Rational) -> Result<f64, String> {
    if value.denominator <= 0 {
        return Err("Sound values need a positive denominator".into());
    }
    Ok(value.numerator as f64 / value.denominator as f64)
}
fn bounded(value: Rational, min: f64, max: f64, label: &str) -> Result<f64, String> {
    let value = ratio(value)?;
    if !(min..=max).contains(&value) {
        return Err(format!("{label} must be between {min} and {max}"));
    }
    Ok(value)
}

/// Named reusable snapshots; applying one copies it to a Sound tile.
pub type SoundLibrary = std::collections::BTreeMap<String, InstrumentDefinition>;

pub fn sound_library_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err("Sound names need 1–128 bytes without control characters".into());
    }
    Ok(name.to_owned())
}

pub fn validate_sound_library(
    library: &SoundLibrary,
    samples: &crate::domain::project::samples::SampleManifest,
) -> Result<(), String> {
    if library.len() > 128 {
        return Err("A project supports at most 128 saved sounds".into());
    }
    let mut names = std::collections::BTreeSet::new();
    for (name, instrument) in library {
        if sound_library_name(name)? != *name || !names.insert(name.to_lowercase()) {
            return Err("Saved sound names must be unique and have no surrounding spaces".into());
        }
        instrument.validate()?;
        if let InstrumentSource::Sample(id) = instrument.source {
            if !samples.samples.contains_key(&id) {
                return Err(format!(
                    "Saved sound {name} refers to a sample absent from this project"
                ));
            }
        }
    }
    Ok(())
}
