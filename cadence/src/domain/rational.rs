//! Exact rational values used for musical time.

use std::{ops, time::Duration};

use crate::domain::prelude::*;

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
/// Normalized rational number with a non-zero denominator.
///
/// Most of the crate uses the [`Time`] alias instead of naming `Rational`
/// directly, because these values usually represent musical time.
pub struct Rational {
    numerator: i64,
    denominator: i64,
}

/// Exact musical time represented as a normalized rational number.
pub type Time = Rational;

impl Time {
    /// Zero cycles.
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    /// One whole cycle.
    pub const ONE: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    /// Creates a normalized rational number.
    ///
    /// The fraction is reduced to lowest terms and the sign is kept on the
    /// numerator.
    ///
    /// # Panics
    ///
    /// Panics if `denominator == 0`.
    ///
    /// ```
    /// use cadence::domain::rational::Time;
    ///
    /// let beat = Time::new(2, 4);
    ///
    /// assert_eq!(beat, Time::new(1, 2));
    /// ```
    pub fn new(numerator: i64, denominator: i64) -> Self {
        // we crash because if this is ever true, we made a wrong assumption somewhere else in the code.
        if denominator == 0 {
            panic!("Zero is an invalid denominator!");
        }
        let gcd = gcd(numerator.abs(), denominator.abs());
        let mut denominator = denominator / gcd;
        let mut numerator = numerator / gcd;
        if denominator < 0 {
            denominator *= -1;
            numerator *= -1;
        }

        Self {
            numerator,
            denominator,
        }
    }

    /// Converts a wall-clock duration into exact seconds as a rational value.
    ///
    /// This is useful when bridging between transport math and `Duration`.
    ///
    /// # Panics
    ///
    /// Panics if the duration exceeds the supported `i64` range.
    pub fn from_duration(duration: Duration) -> Self {
        const NANOS_PER_SEC: i64 = 1_000_000_000;

        let secs = i64::try_from(duration.as_secs())
            .expect("duration seconds exceed the supported Time range");
        let nanos = duration.subsec_nanos() as i64;

        let total_nanos = secs
            .checked_mul(NANOS_PER_SEC)
            .and_then(|secs_as_nanos| secs_as_nanos.checked_add(nanos))
            .expect("duration exceeds the supported Time range");

        Self::new(total_nanos, NANOS_PER_SEC)
    }

    /// Returns the normalized numerator.
    pub fn numerator(&self) -> i64 {
        self.numerator
    }

    /// Returns the normalized denominator.
    pub fn denominator(&self) -> i64 {
        self.denominator
    }

    /// Returns an exact integer value with denominator `1`.
    pub fn whole_number(boundary: i64) -> Self {
        Self {
            numerator: boundary,
            denominator: 1,
        }
    }

    /// Returns the approximate floating-point value.
    ///
    /// Use this only when an external API requires floats. Keep exact `Time`
    /// values as long as possible for symbolic timing logic.
    pub fn value(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    /// Returns the mathematical floor of the rational value.
    pub fn floor(&self) -> i64 {
        let quotient = self.numerator / self.denominator;
        let remainder = self.numerator % self.denominator;

        if self.numerator < 0 && remainder != 0 {
            quotient - 1
        } else {
            quotient
        }
    }

    /// Returns the next whole-number boundary strictly above this value.
    pub fn next_boundary(&self) -> Time {
        Time::new(self.floor() + 1, 1)
    }
}

impl ops::Add for Time {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let g = gcd(self.denominator, rhs.denominator);
        let num = self.numerator * (rhs.denominator / g) + rhs.numerator * (self.denominator / g);
        let dem = (self.denominator / g) * rhs.denominator;

        Self::new(num, dem)
    }
}

impl ops::Sub for Time {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        let g = gcd(self.denominator, rhs.denominator);
        let num = self.numerator * (rhs.denominator / g) - rhs.numerator * (self.denominator / g);
        let dem = (self.denominator / g) * rhs.denominator;

        Self::new(num, dem)
    }
}

impl ops::Mul for Time {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let g1 = gcd(self.numerator.abs(), rhs.denominator);
        let g2 = gcd(rhs.numerator.abs(), self.denominator);

        let num = (self.numerator / g1) * (rhs.numerator / g2);
        let dem = (self.denominator / g2) * (rhs.denominator / g1);

        Self::new(num, dem)
    }
}

impl ops::Div for Time {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        if rhs.numerator == 0 {
            panic!("Cannot divide by zero");
        }

        let g1 = gcd(self.numerator.abs(), rhs.numerator.abs());
        let g2 = gcd(rhs.denominator, self.denominator);

        let num = (self.numerator / g1) * (rhs.denominator / g2);
        let dem = (self.denominator / g2) * (rhs.numerator / g1);

        Self::new(num, dem)
    }
}

impl Ord for Time {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        let g = gcd(self.denominator, rhs.denominator);
        let lhs = self.numerator * (rhs.denominator / g);
        let rhs = rhs.numerator * (self.denominator / g);
        lhs.cmp(&rhs)
    }
}
impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Exact coordinate on one spatial lattice axis.
///
/// `Coord` shares the same normalized rational core as [`Time`], so spatial
/// math stays drift-free in exactly the same way musical time does. It is a
/// distinct newtype so spatial and temporal values can never be added or
/// compared by accident. The integer part is the grid cell; the fractional
/// part is the sub-cell offset.
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub struct Coord(Rational);

impl Default for Coord {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Coord {
    /// Origin coordinate (the lattice center).
    pub const ZERO: Self = Self(Rational::ZERO);

    /// One whole cell along the axis.
    pub const ONE: Self = Self(Rational::ONE);

    /// Creates a normalized coordinate from a numerator and denominator.
    ///
    /// # Panics
    ///
    /// Panics if `denominator == 0`.
    #[must_use]
    pub fn new(numerator: i64, denominator: i64) -> Self {
        Self(Rational::new(numerator, denominator))
    }

    /// Wraps an existing rational value as a coordinate.
    #[must_use]
    pub fn from_rational(value: Rational) -> Self {
        Self(value)
    }

    /// Returns an exact integer coordinate (a whole grid cell).
    #[must_use]
    pub fn whole_number(cell: i64) -> Self {
        Self(Rational::whole_number(cell))
    }

    /// Returns the underlying rational value.
    #[must_use]
    pub fn rational(self) -> Rational {
        self.0
    }

    /// Returns the approximate floating-point value.
    ///
    /// Use this only at the render/audio boundary; keep exact `Coord` values
    /// for symbolic spatial logic.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0.value()
    }

    /// Returns the integer grid cell (mathematical floor).
    #[must_use]
    pub fn cell(self) -> i64 {
        self.0.floor()
    }
}

impl ops::Add for Coord {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl ops::Sub for Coord {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl ops::Neg for Coord {
    type Output = Self;
    fn neg(self) -> Self {
        Self(Rational::ZERO - self.0)
    }
}

impl ops::Mul for Coord {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl ops::Mul<Rational> for Coord {
    type Output = Self;
    fn mul(self, rhs: Rational) -> Self {
        Self(self.0 * rhs)
    }
}

impl Ord for Coord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Coord {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // expect a panic
    #[test]
    #[should_panic(expected = "Zero is an invalid denominator")]
    fn zero_denom_panics() {
        Time::new(1, 0);
    }
    #[test]
    fn div_one_eighth_by_four() {
        let result = Time::new(1, 8) / Time::new(4, 1);
        assert_eq!(result, Time::new(1, 32));
    }
    #[test]
    fn rational_arithmetic_is_functional() {
        assert_eq!(Time::ZERO + Time::new(1, 4), Time::new(1, 4));
        assert_eq!(Time::new(1, 2) - Time::new(1, 4), Time::new(1, 4));
        assert_eq!(Time::new(1, 8) * Time::new(4, 1), Time::new(1, 2));
        assert_eq!(Time::new(1, 8) / Time::new(4, 1), Time::new(1, 32));
        assert_eq!(Time::new(4, 1) / Time::new(1, 4), Time::new(16, 1));
    }
}
