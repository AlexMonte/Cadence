//! Complete continuous-control values. Clock settings and ranges are rational.
use super::{ParameterKey, Rational};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModulationWaveform {
    Sine,
    Saw,
    Triangle,
    Square,
    SmoothNoise,
    SteppedNoise,
    Random,
    Ramp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulationParameters {
    pub waveform: ModulationWaveform,
    pub rate: Rational,
    pub phase: Rational,
    pub minimum: Rational,
    pub maximum: Rational,
    pub seed: u64,
}
impl ModulationParameters {
    pub fn for_parameter(parameter: ParameterKey) -> Self {
        let (minimum, maximum) = match parameter {
            ParameterKey::PlaybackRate => (Rational::new(1, 2), Rational::from_integer(2)),
            ParameterKey::LowPassCutoff => {
                (Rational::from_integer(200), Rational::from_integer(4000))
            }
            ParameterKey::Transpose => (Rational::from_integer(-12), Rational::from_integer(12)),
            _ => (Rational::zero(), Rational::one()),
        };
        Self {
            waveform: ModulationWaveform::Sine,
            rate: Rational::one(),
            phase: Rational::zero(),
            minimum,
            maximum,
            seed: 0,
        }
    }
    pub fn validate_for(&self, parameter: ParameterKey) -> Result<(), &'static str> {
        for value in [self.rate, self.phase, self.minimum, self.maximum] {
            if value.denominator <= 0 {
                return Err("Modulation values must be valid rational numbers.");
            }
        }
        if self.minimum > self.maximum {
            return Err("The minimum must not exceed the maximum.");
        }
        match parameter {
            ParameterKey::Velocity
                if self.minimum >= Rational::zero() && self.maximum <= Rational::one() =>
            {
                Ok(())
            }
            ParameterKey::Gain if self.minimum >= Rational::zero() => Ok(()),
            ParameterKey::PlaybackRate | ParameterKey::LowPassCutoff
                if self.minimum > Rational::zero() =>
            {
                Ok(())
            }
            ParameterKey::Transpose
                if self.minimum >= Rational::from_integer(-127)
                    && self.maximum <= Rational::from_integer(127) =>
            {
                Ok(())
            }
            ParameterKey::Gain => Err("Gain modulation must stay at or above zero."),
            ParameterKey::Velocity => Err("Velocity modulation must stay between zero and one."),
            ParameterKey::PlaybackRate | ParameterKey::LowPassCutoff => {
                Err("Rate and cutoff modulation must stay above zero.")
            }
            ParameterKey::Transpose => {
                Err("Transpose modulation must stay within -127 to 127 semitones.")
            }
            _ => Err("This control does not support signal modulation."),
        }
    }
}
impl Default for ModulationParameters {
    fn default() -> Self {
        Self::for_parameter(ParameterKey::Gain)
    }
}
