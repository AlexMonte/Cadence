//! Signal sources evaluated as pure functions of musical time.
//!
//! A [`Signal`] is a small, serializable-friendly descriptor for a continuous
//! modulation source, including periodic shapes and seeded noise. Use
//! [`Signal::eval_at`] for exact fractional note onsets and [`Signal::eval`]
//! for continuous rendering. The same clock and descriptor always produce
//! the same value. Signals carry exact clock ratios, amplitudes and a seed so the
//! `Score`/`ControlScore` trees that embed them keep their
//! `Clone + PartialEq + Debug + Send + Sync` invariants.

use std::f64::consts::TAU;

use crate::domain::rational::Rational;

/// Continuous waveform shape used by a [`Signal`].
///
/// Periodic shapes complete one oscillation per unit of (rate-scaled) cycle
/// time. The noise shapes are deterministic and seeded so they never allocate
/// and never depend on external state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Waveform {
    /// Non-repeating linear ramp, clamped to [0, 1] in phase space.
    Ramp,
    /// Sine wave in the range `[-1.0, 1.0]`.
    Sine,
    /// Rising sawtooth wave in the range `[-1.0, 1.0)`.
    Saw,
    /// Triangle wave in the range `[-1.0, 1.0]`.
    Tri,
    /// Square wave alternating between `-1.0` and `1.0`.
    Square,
    /// Smooth seeded value (perlin-style) noise in the range `[-1.0, 1.0]`.
    Perlin {
        /// Deterministic seed.
        seed: u64,
    },
    /// Sample-and-hold seeded white noise in the range `[-1.0, 1.0]`.
    Rand {
        /// Deterministic seed.
        seed: u64,
    },
    /// Independent seeded noise at every distinct musical time. Note controls
    /// use [`Signal::eval_at`] to hash the normalized exact fractional onset.
    Random {
        /// Deterministic seed.
        seed: u64,
    },
}

/// Continuous modulation source evaluated as a pure function of cycle time.
///
/// The evaluated value is `bias + depth * shape(rate * cycle_time + phase)`,
/// where `shape` is the selected [`Waveform`].
///
/// `rate` and `phase` are exact, cycle-relative [`Rational`]s so signals stay
/// coherent with musical time and with each other (harmonic ratios are exact).
/// `depth` and `bias` remain `f64` because they are amplitudes, not time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signal {
    waveform: Waveform,
    rate: Rational,
    depth: f64,
    bias: f64,
    phase: Rational,
}

impl Signal {
    /// Creates a signal with the given waveform and default shaping.
    ///
    /// Defaults are `rate = 1`, `depth = 1.0`, `bias = 0.0`, and `phase = 0`.
    #[must_use]
    pub fn new(waveform: Waveform) -> Self {
        Self {
            waveform,
            rate: Rational::ONE,
            depth: 1.0,
            bias: 0.0,
            phase: Rational::ZERO,
        }
    }

    /// Creates a non-repeating ramp over an absolute musical interval.
    #[must_use]
    pub fn linear_ramp(start: Rational, end: Rational, from: f64, to: f64) -> Self {
        assert!(end > start, "ramp interval must be positive");
        Self::new(Waveform::Ramp)
            .with_rate(Rational::ONE / (end - start))
            .with_phase((Rational::ZERO - start) / (end - start))
            .with_depth(to - from)
            .with_bias(from)
    }

    /// Creates a sine signal.
    #[must_use]
    pub fn sine() -> Self {
        Self::new(Waveform::Sine)
    }

    /// Creates a sawtooth signal.
    #[must_use]
    pub fn saw() -> Self {
        Self::new(Waveform::Saw)
    }

    /// Creates a triangle signal.
    #[must_use]
    pub fn tri() -> Self {
        Self::new(Waveform::Tri)
    }

    /// Creates a square signal.
    #[must_use]
    pub fn square() -> Self {
        Self::new(Waveform::Square)
    }

    /// Creates a smooth seeded perlin-style noise signal.
    #[must_use]
    pub fn perlin(seed: u64) -> Self {
        Self::new(Waveform::Perlin { seed })
    }

    /// Creates a sample-and-hold seeded random-noise signal.
    #[must_use]
    pub fn rand(seed: u64) -> Self {
        Self::new(Waveform::Rand { seed })
    }

    /// Creates independent seeded noise for every distinct fractional onset.
    /// Unlike [`Self::rand`], this does not hold one value for an entire step.
    #[must_use]
    pub fn random(seed: u64) -> Self {
        Self::new(Waveform::Random { seed })
    }

    /// Sets the oscillation rate in cycles.
    ///
    /// A rate of `1` completes one full oscillation per transport cycle.
    #[must_use]
    pub fn with_rate(mut self, rate: Rational) -> Self {
        self.rate = rate;
        self
    }

    /// Sets the modulation depth (amplitude) multiplier.
    #[must_use]
    pub fn with_depth(mut self, depth: f64) -> Self {
        self.depth = depth;
        self
    }

    /// Sets the additive bias (center value).
    #[must_use]
    pub fn with_bias(mut self, bias: f64) -> Self {
        self.bias = bias;
        self
    }

    /// Sets the phase offset in cycles.
    #[must_use]
    pub fn with_phase(mut self, phase: Rational) -> Self {
        self.phase = phase;
        self
    }

    /// Returns the waveform shape.
    #[must_use]
    pub fn waveform(&self) -> Waveform {
        self.waveform
    }

    /// Returns the oscillation rate in cycles.
    #[must_use]
    pub fn rate(&self) -> Rational {
        self.rate
    }

    /// Returns the modulation depth.
    #[must_use]
    pub fn depth(&self) -> f64 {
        self.depth
    }

    /// Returns the additive bias.
    #[must_use]
    pub fn bias(&self) -> f64 {
        self.bias
    }

    /// Returns the phase offset in cycles.
    #[must_use]
    pub fn phase(&self) -> Rational {
        self.phase
    }

    /// Evaluates the signal at the given cycle time.
    ///
    /// This is pure and deterministic: identical inputs always produce
    /// identical outputs, with no allocation and no external state.
    #[must_use]
    pub fn eval(&self, cycle_time: f64) -> f64 {
        let total_phase = self.rate.value() * cycle_time + self.phase.value();
        self.bias + self.depth * shape(self.waveform, total_phase)
    }

    /// Evaluates an onset using its exact musical clock. Random noise hashes
    /// the reduced numerator and denominator after rate and phase are applied,
    /// so equivalent fractions, seeks and query boundaries choose one value.
    /// If the exact total exceeds checked 128-bit arithmetic, a deterministic
    /// hash of the normalized input ratios and time replaces that calculation.
    /// Continuous rendering uses [`Self::eval`] with its floating-point clock.
    #[must_use]
    pub fn eval_at(&self, cycle_time: Rational) -> f64 {
        if let Waveform::Random { seed } = self.waveform {
            let value = match exact_phase(self.rate, cycle_time, self.phase) {
                Some((numerator, denominator)) => hash_exact_phase(seed, numerator, denominator),
                // Extremely large rational combinations can exceed even the
                // wide temporary representation. Preserve determinism without
                // panicking, wrapping, or approximating all late notes to the
                // same floating-point timestamp.
                None => hash_words(
                    seed ^ 0x5241_4E44_4641_4C4C,
                    &[
                        self.rate.numerator() as u64,
                        self.rate.denominator() as u64,
                        cycle_time.numerator() as u64,
                        cycle_time.denominator() as u64,
                        self.phase.numerator() as u64,
                        self.phase.denominator() as u64,
                    ],
                ),
            };
            let unit = (value >> 11) as f64 / ((1u64 << 53) as f64);
            self.bias + self.depth * (unit * 2.0 - 1.0)
        } else {
            self.eval(cycle_time.value())
        }
    }
}

// Keep the widened, checked calculation local to random onset sampling. Global
// musical-time arithmetic and the continuous floating-point path are unchanged.
fn exact_phase(rate: Rational, time: Rational, phase: Rational) -> Option<(i128, i128)> {
    let (a, b) = (i128::from(rate.numerator()), i128::from(rate.denominator()));
    let (c, d) = (i128::from(time.numerator()), i128::from(time.denominator()));
    let ad = wide_gcd(a.unsigned_abs(), d as u128) as i128;
    let cb = wide_gcd(c.unsigned_abs(), b as u128) as i128;
    let numerator = (a / ad).checked_mul(c / cb)?;
    let denominator = (b / cb).checked_mul(d / ad)?;
    let (numerator, denominator) = reduce_wide(numerator, denominator);
    let phase_denominator = i128::from(phase.denominator());
    let common = wide_gcd(denominator as u128, phase_denominator as u128) as i128;
    let left_scale = phase_denominator / common;
    let right_scale = denominator / common;
    let left = numerator.checked_mul(left_scale)?;
    let right = i128::from(phase.numerator()).checked_mul(right_scale)?;
    Some(reduce_wide(
        left.checked_add(right)?,
        denominator.checked_mul(left_scale)?,
    ))
}

fn reduce_wide(numerator: i128, denominator: i128) -> (i128, i128) {
    let common = wide_gcd(numerator.unsigned_abs(), denominator as u128) as i128;
    (numerator / common, denominator / common)
}

fn wide_gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn hash_exact_phase(seed: u64, numerator: i128, denominator: i128) -> u64 {
    if let (Ok(numerator), Ok(denominator)) = (i64::try_from(numerator), i64::try_from(denominator))
    {
        // Preserve the original hash for ordinary musical timestamps.
        let key = splitmix64(seed ^ splitmix64(numerator as u64));
        splitmix64(key ^ denominator as u64)
    } else {
        hash_words(
            seed ^ 0x5241_4E44_5749_4445,
            &[
                numerator as u64,
                (numerator >> 64) as u64,
                denominator as u64,
                (denominator >> 64) as u64,
            ],
        )
    }
}

fn hash_words(seed: u64, words: &[u64]) -> u64 {
    words
        .iter()
        .fold(splitmix64(seed), |key, word| splitmix64(key ^ word))
}

fn shape(waveform: Waveform, phase: f64) -> f64 {
    match waveform {
        Waveform::Ramp => phase.clamp(0.0, 1.0),
        Waveform::Sine => (TAU * phase).sin(),
        Waveform::Saw => {
            let wrapped = phase.rem_euclid(1.0);
            2.0 * wrapped - 1.0
        }
        Waveform::Tri => {
            let wrapped = phase.rem_euclid(1.0);
            if wrapped < 0.5 {
                4.0 * wrapped - 1.0
            } else {
                3.0 - 4.0 * wrapped
            }
        }
        Waveform::Square => {
            let wrapped = phase.rem_euclid(1.0);
            if wrapped < 0.5 { 1.0 } else { -1.0 }
        }
        Waveform::Perlin { seed } => perlin_noise(seed, phase),
        Waveform::Rand { seed } => rand_noise(seed, phase),
        Waveform::Random { seed } => hash_signed(seed, phase.to_bits() as i64),
    }
}

fn rand_noise(seed: u64, phase: f64) -> f64 {
    let cell = phase.floor() as i64;
    hash_signed(seed, cell)
}

fn perlin_noise(seed: u64, phase: f64) -> f64 {
    let lower = phase.floor();
    let cell = lower as i64;
    let frac = phase - lower;
    let a = hash_signed(seed, cell);
    let b = hash_signed(seed, cell.wrapping_add(1));
    // Smoothstep interpolation between adjacent lattice values.
    let t = frac * frac * (3.0 - 2.0 * frac);
    a + (b - a) * t
}

fn hash_signed(seed: u64, cell: i64) -> f64 {
    hash_unit(seed, cell) * 2.0 - 1.0
}

fn hash_unit(seed: u64, cell: i64) -> f64 {
    let mixed = splitmix64(seed ^ splitmix64(cell as u64));
    // Use the top 53 bits to build a uniform value in [0, 1).
    (mixed >> 11) as f64 / ((1u64 << 53) as f64)
}

fn splitmix64(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_is_deterministic_and_bounded() {
        let signal = Signal::sine();
        for step in 0..1000 {
            let t = step as f64 / 97.0;
            let value = signal.eval(t);
            assert_eq!(value, signal.eval(t));
            assert!((-1.0..=1.0).contains(&value));
        }
    }

    #[test]
    fn sine_completes_one_oscillation_per_cycle() {
        let signal = Signal::sine();
        assert!(signal.eval(0.0).abs() < 1e-9);
        assert!((signal.eval(0.25) - 1.0).abs() < 1e-9);
        assert!(signal.eval(0.5).abs() < 1e-9);
    }

    #[test]
    fn bias_and_depth_shift_range() {
        let signal = Signal::sine().with_bias(0.5).with_depth(0.5);
        for step in 0..1000 {
            let value = signal.eval(step as f64 / 41.0);
            assert!((-1e-9..=1.0 + 1e-9).contains(&value));
        }
    }

    #[test]
    fn saw_tri_square_stay_in_unit_range() {
        for signal in [Signal::saw(), Signal::tri(), Signal::square()] {
            for step in 0..500 {
                let value = signal.eval(step as f64 / 13.0);
                assert!((-1.0..=1.0).contains(&value), "{value}");
            }
        }
    }

    #[test]
    fn noise_is_deterministic_bounded_and_seed_sensitive() {
        let a = Signal::perlin(7);
        let b = Signal::perlin(8);
        let r = Signal::rand(7);
        let mut differs = false;
        for step in 0..500 {
            let t = step as f64 / 7.0;
            assert_eq!(a.eval(t), a.eval(t));
            assert!((-1.0..=1.0).contains(&a.eval(t)));
            assert!((-1.0..=1.0).contains(&r.eval(t)));
            if (a.eval(t) - b.eval(t)).abs() > 1e-9 {
                differs = true;
            }
        }
        assert!(differs, "different seeds must produce different noise");
    }

    #[test]
    fn rand_holds_value_within_an_integer_step() {
        let signal = Signal::rand(3);
        assert_eq!(signal.eval(2.1), signal.eval(2.9));
        assert_ne!(signal.eval(2.1), signal.eval(3.1));
    }

    #[test]
    fn exact_random_phase_uses_wide_checked_math_and_a_stable_overflow_fallback() {
        let huge = Rational::whole_number(i64::MAX);
        let random = Signal::random(73)
            .with_rate(huge)
            .with_bias(0.85)
            .with_depth(0.15);
        let (numerator, denominator) = exact_phase(huge, huge, Rational::ZERO).unwrap();
        assert_eq!(numerator, i128::from(i64::MAX) * i128::from(i64::MAX));
        assert_eq!(denominator, 1);
        let wide_value = random.eval_at(huge);
        assert!((0.7..=1.0).contains(&wide_value));
        assert_eq!(wide_value, random.eval_at(huge));
        assert_ne!(
            wide_value,
            random.eval_at(Rational::whole_number(i64::MAX - 1))
        );

        // The exact common numerator needs about 189 bits. No unchecked Time
        // multiplication/addition is attempted, and nearby late onsets differ.
        let phase = Rational::new(1, i64::MAX);
        assert!(exact_phase(huge, huge, phase).is_none());
        let fallback = random.with_phase(phase);
        let value = fallback.eval_at(huge);
        assert!((0.7..=1.0).contains(&value));
        assert_eq!(value, fallback.eval_at(huge));
        assert_ne!(
            value,
            fallback.eval_at(Rational::whole_number(i64::MAX - 1))
        );
        assert!(
            Signal::random(73)
                .with_rate(Rational::whole_number(i64::MIN))
                .eval_at(Rational::whole_number(i64::MAX))
                .is_finite()
        );
    }
}
