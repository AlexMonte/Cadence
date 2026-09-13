//! Complete effect operands. Times are seconds; normalized levels use 0..=1.
use super::Rational;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelayParameters {
    pub amount: Rational,
    pub time: Rational,
    pub feedback: Rational,
    pub damping: Rational,
}
impl Default for DelayParameters {
    fn default() -> Self {
        Self {
            amount: Rational::new(1, 4),
            time: Rational::new(1, 4),
            feedback: Rational::new(1, 4),
            damping: Rational::new(1, 2),
        }
    }
}
impl DelayParameters {
    pub fn validate(&self) -> Result<(), &'static str> {
        unit(self.amount)?;
        unit(self.feedback)?;
        unit(self.damping)?;
        bounded(
            self.time,
            Rational::new(1, 1000),
            Rational::from_integer(2),
            "Delay time must be 0.001 to 2 seconds.",
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReverbParameters {
    pub amount: Rational,
    pub decay: Rational,
    pub damping: Rational,
}
impl Default for ReverbParameters {
    fn default() -> Self {
        Self {
            amount: Rational::new(1, 4),
            decay: Rational::one(),
            damping: Rational::new(1, 2),
        }
    }
}
impl ReverbParameters {
    pub fn validate(&self) -> Result<(), &'static str> {
        unit(self.amount)?;
        unit(self.damping)?;
        bounded(
            self.decay,
            Rational::new(1, 1000),
            Rational::from_integer(60),
            "Reverb decay must be 0.001 to 60 seconds.",
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressorParameters {
    pub threshold: Rational,
    pub ratio: Rational,
    /// Soft-knee width in decibels.
    pub knee_db: Rational,
    pub attack: Rational,
    pub release: Rational,
}
impl Default for CompressorParameters {
    fn default() -> Self {
        Self {
            knee_db: Rational::zero(),
            threshold: Rational::new(1, 2),
            ratio: Rational::from_integer(4),
            attack: Rational::new(1, 100),
            release: Rational::new(1, 10),
        }
    }
}
impl CompressorParameters {
    pub fn validate(&self) -> Result<(), &'static str> {
        unit(self.threshold)?;
        bounded(
            self.knee_db,
            Rational::zero(),
            Rational::from_integer(40),
            "Compressor knee must be 0 to 40 dB.",
        )?;
        bounded(
            self.ratio,
            Rational::one(),
            Rational::from_integer(20),
            "Compressor ratio must be 1 to 20.",
        )?;
        for value in [self.attack, self.release] {
            bounded(
                value,
                Rational::new(1, 1000),
                Rational::from_integer(60),
                "Compressor times must be 0.001 to 60 seconds.",
            )?;
        }
        Ok(())
    }
}
fn unit(value: Rational) -> Result<(), &'static str> {
    bounded(
        value,
        Rational::zero(),
        Rational::one(),
        "Effect levels must be between zero and one.",
    )
}
fn bounded(
    value: Rational,
    min: Rational,
    max: Rational,
    error: &'static str,
) -> Result<(), &'static str> {
    if value.denominator > 0 && value >= min && value <= max {
        Ok(())
    } else {
        Err(error)
    }
}

/// A complete typed effect value, usable as one time slot in a control pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectValue {
    Modulation {
        parameter: super::ParameterKey,
        value: super::ModulationParameters,
    },
    Delay(DelayParameters),
    Reverb(ReverbParameters),
    Compressor(CompressorParameters),
}
impl EffectValue {
    pub fn control(self) -> (super::ControlKeyIr, super::ControlValueIr) {
        match self {
            Self::Modulation { parameter, value } => (
                parameter.control_key().expect("validated modulation lane"),
                super::ControlValueIr::Modulation { value },
            ),
            Self::Delay(value) => (
                super::ControlKeyIr::DelaySend,
                super::ControlValueIr::Delay { value },
            ),
            Self::Reverb(value) => (
                super::ControlKeyIr::ReverbSend,
                super::ControlValueIr::Reverb { value },
            ),
            Self::Compressor(value) => (
                super::ControlKeyIr::Compressor,
                super::ControlValueIr::Compressor { value },
            ),
        }
    }
}
