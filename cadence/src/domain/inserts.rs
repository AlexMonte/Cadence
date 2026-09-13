//! Immutable, ordered per-voice sound processing descriptions.
use super::control::UnitValue;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Invalid authoring parameters. Construction never truncates an effect chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InsertError {
    /// More than eight effects were requested.
    #[error("an insert chain supports at most eight effects")]
    TooManyEffects,
    /// A filter cutoff is outside the supported finite range.
    #[error("insert filter cutoff must be finite and between 20 and 20000 Hz")]
    Cutoff,
    /// A normalized parameter is outside [0, 1].
    #[error("insert resonance, drive amount and wet mix must be finite and within [0, 1]")]
    UnitRange,
    /// Drive output gain is outside [0, 2].
    #[error("drive output gain must be finite and within [0, 2]")]
    OutputGain,
}

/// One low- or high-pass biquad. Device-rate preparation limits cutoff to 45%
/// of the actual sample rate, matching the existing voice filter behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsertFilter {
    cutoff_hz: u64,
    resonance: u64,
}
impl InsertFilter {
    fn new(cutoff_hz: f64, resonance: f64) -> Result<Self, InsertError> {
        if !cutoff_hz.is_finite() || !(20.0..=20_000.0).contains(&cutoff_hz) {
            return Err(InsertError::Cutoff);
        }
        let resonance = unit(resonance)?;
        Ok(Self {
            cutoff_hz: cutoff_hz.to_bits(),
            resonance: resonance.to_bits(),
        })
    }
    /// Returns the authored cutoff in hertz.
    pub fn cutoff_hz(self) -> f64 {
        f64::from_bits(self.cutoff_hz)
    }
    /// Returns normalized resonance.
    pub fn resonance(self) -> UnitValue {
        UnitValue::new(f64::from_bits(self.resonance)).unwrap()
    }
}
impl Hash for InsertFilter {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cutoff_hz.hash(state);
        self.resonance.hash(state);
    }
}

/// Stateless soft saturation. Amount and wet mix are unit-range; output gain
/// is linear [0, 2]. Amount zero or wet zero bypasses saturation, retaining the
/// output gain. The fully neutral setting (0, any wet, 1) is exactly identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InsertDrive {
    amount: u64,
    wet: u64,
    output_gain: u64,
}
impl InsertDrive {
    fn new(amount: f64, wet: f64, output_gain: f64) -> Result<Self, InsertError> {
        let amount = unit(amount)?;
        let wet = unit(wet)?;
        if !output_gain.is_finite() || !(0.0..=2.0).contains(&output_gain) {
            return Err(InsertError::OutputGain);
        }
        Ok(Self {
            amount: amount.to_bits(),
            wet: wet.to_bits(),
            output_gain: positive_zero(output_gain).to_bits(),
        })
    }
    /// Returns normalized saturation amount.
    pub fn amount(self) -> f64 {
        f64::from_bits(self.amount)
    }
    /// Returns the processed fraction of the output.
    pub fn wet(self) -> f64 {
        f64::from_bits(self.wet)
    }
    /// Returns the final linear gain.
    pub fn output_gain(self) -> f64 {
        f64::from_bits(self.output_gain)
    }
}

fn positive_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}
fn unit(value: f64) -> Result<f64, InsertError> {
    UnitValue::new(value)
        .map(|value| positive_zero(value.value()))
        .ok_or(InsertError::UnitRange)
}

/// A validated insert. Its position in the chain is its processing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsertEffect {
    /// Remove frequencies above the cutoff.
    LowPass(InsertFilter),
    /// Remove frequencies below the cutoff.
    HighPass(InsertFilter),
    /// Apply soft saturation and wet/dry mixing.
    Drive(InsertDrive),
}
impl InsertEffect {
    /// Creates a low-pass insert with cutoff 20–20000 Hz and resonance [0, 1].
    pub fn low_pass(cutoff_hz: f64, resonance: f64) -> Result<Self, InsertError> {
        InsertFilter::new(cutoff_hz, resonance).map(Self::LowPass)
    }
    /// Creates a high-pass insert with cutoff 20–20000 Hz and resonance [0, 1].
    pub fn high_pass(cutoff_hz: f64, resonance: f64) -> Result<Self, InsertError> {
        InsertFilter::new(cutoff_hz, resonance).map(Self::HighPass)
    }
    /// Creates saturation with amount/wet [0, 1] and output gain [0, 2].
    pub fn drive(amount: f64, wet: f64, output_gain: f64) -> Result<Self, InsertError> {
        InsertDrive::new(amount, wet, output_gain).map(Self::Drive)
    }
}

/// Shared immutable topology. Hosts persist `effects()` in order and rebuild
/// through these validating constructors; DSP history is never authoring data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct InsertChain(Arc<[InsertEffect]>);
impl InsertChain {
    /// Maximum immutable topology and per-voice state length.
    pub const MAX_EFFECTS: usize = 8;
    /// Validates the count without dropping or reordering any effect.
    pub fn new(effects: Vec<InsertEffect>) -> Result<Self, InsertError> {
        if effects.len() > Self::MAX_EFFECTS {
            return Err(InsertError::TooManyEffects);
        }
        Ok(Self(effects.into()))
    }
    /// Returns effects in processing order.
    pub fn effects(&self) -> &[InsertEffect] {
        &self.0
    }
    /// Whether the chain leaves the existing processing path unchanged.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
