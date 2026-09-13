//! Immutable instrument defaults, below ambient and authored note controls.
use super::control::{
    CompressorSettings, ControlKey as K, ControlMap, ControlValue as V, DelaySettings,
    ReverbSettings, UnitValue,
};
use std::{
    hash::{Hash, Hasher},
    time::Duration,
};

/// Envelope values belonging to a sound definition rather than a note pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeDefaults {
    attack: Duration,
    decay: Duration,
    sustain: UnitValue,
    release: Duration,
}
impl EnvelopeDefaults {
    /// Times are wall-clock durations; sustain is a linear unit level.
    pub fn new(attack: Duration, decay: Duration, sustain: UnitValue, release: Duration) -> Self {
        Self {
            attack,
            decay,
            sustain,
            release,
        }
    }
}
impl Hash for EnvelopeDefaults {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.attack.hash(state);
        self.decay.hash(state);
        bits(self.sustain.value()).hash(state);
        self.release.hash(state);
    }
}

/// Sound defaults survive transport, instrument assignment, and live audition.
/// An absent value keeps the source or preset default. Authored note controls
/// override these values without creating conflicting control lanes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SoundDefaults {
    envelope: Option<EnvelopeDefaults>,
    gain: Option<u64>,
    delay: Option<DelaySettings>,
    reverb: Option<ReverbSettings>,
    compressor: Option<CompressorSettings>,
}
impl SoundDefaults {
    /// Replaces the instrument envelope.
    pub fn with_envelope(mut self, envelope: EnvelopeDefaults) -> Self {
        self.envelope = Some(envelope);
        self
    }
    /// Replaces source/preset gain. The value must be finite and nonnegative.
    pub fn with_gain(mut self, gain: f64) -> Result<Self, &'static str> {
        if !gain.is_finite() || gain < 0.0 {
            return Err("sound gain must be finite and nonnegative");
        }
        self.gain = Some(bits(gain));
        Ok(self)
    }
    /// Replaces the delay send defaults.
    pub fn with_delay(mut self, delay: DelaySettings) -> Self {
        self.delay = Some(delay);
        self
    }
    /// Replaces the reverb send defaults.
    pub fn with_reverb(mut self, reverb: ReverbSettings) -> Self {
        self.reverb = Some(reverb);
        self
    }
    /// Replaces per-voice compression, processed before ordered inserts.
    pub fn with_compressor(mut self, compressor: CompressorSettings) -> Self {
        self.compressor = Some(compressor);
        self
    }
    /// Builds the control-thread map; never called by the per-frame renderer.
    pub fn controls(&self) -> ControlMap {
        let mut controls = ControlMap::new();
        if let Some(envelope) = &self.envelope {
            controls.extend([
                (K::Attack, V::Scalar(envelope.attack.as_secs_f64())),
                (K::Decay, V::Scalar(envelope.decay.as_secs_f64())),
                (K::Sustain, V::Scalar(envelope.sustain.value())),
                (K::Release, V::Scalar(envelope.release.as_secs_f64())),
            ]);
        }
        if let Some(gain) = self.gain {
            controls.insert(K::Gain, V::Scalar(f64::from_bits(gain)));
        }
        if let Some(delay) = &self.delay {
            controls.insert(K::DelaySend, V::Delay(*delay));
        }
        if let Some(reverb) = &self.reverb {
            controls.insert(K::ReverbSend, V::Reverb(*reverb));
        }
        if let Some(compressor) = &self.compressor {
            controls.insert(K::Compressor, V::Compressor(*compressor));
        }
        controls
    }
}
fn bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0f64.to_bits()
    } else {
        value.to_bits()
    }
}
impl Hash for SoundDefaults {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.envelope.hash(state);
        self.gain.hash(state);
        self.delay.is_some().hash(state);
        if let Some(delay) = &self.delay {
            bits(delay.amount().value()).hash(state);
            delay.time().hash(state);
            bits(delay.feedback().value()).hash(state);
            bits(delay.damping().value()).hash(state);
        }
        self.compressor.is_some().hash(state);
        if let Some(compressor) = &self.compressor {
            bits(compressor.threshold().value()).hash(state);
            bits(compressor.ratio()).hash(state);
            bits(compressor.knee_db()).hash(state);
            compressor.attack().hash(state);
            compressor.release().hash(state);
        }
        self.reverb.is_some().hash(state);
        if let Some(reverb) = &self.reverb {
            bits(reverb.amount().value()).hash(state);
            reverb.decay().hash(state);
            bits(reverb.damping().value()).hash(state);
        }
    }
}
