use std::{
    cmp::Ordering,
    ops::{Add, AddAssign, Sub, SubAssign},
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;

use crate::domain::rational::Time;

/// Canonical application transport clock.
///
/// The underlying ideas are still useful:
/// - the app schedules in **cycles**
/// - internally, time is still stored as **whole ticks + fractional progress**
///
/// Keeping the `tick` concept inside [`ClockTime`] is valuable even in a
/// cycle-oriented app because it gives us a stable whole+fraction transport
/// representation that can also be reused for lower-level timing concerns
/// that are not strictly musical phrase/cycle semantics.

/// Converts a wall-clock duration into musical cycles at the given transport rate.
///
/// # Panics
///
/// Panics if `cycles_per_second <= 0`.
pub fn map_duration_to_cycles(duration: Duration, cycles_per_second: Time) -> Time {
    if cycles_per_second <= Time::ZERO {
        panic!("cycles_per_second must be positive");
    }
    if duration.is_zero() {
        return Time::ZERO;
    }

    Time::from_duration(duration) * cycles_per_second
}

/// A high-fidelity point in transport time.
///
/// The app thinks in cycles, but this type stores time as:
/// - whole ticks
/// - a fractional tick
///
/// For this project, a "tick" can represent one musical cycle, but we keep the
/// term because the representation is useful even if we later apply the same
/// clock to lower-level transport or rendering logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockTime {
    ticks: u64,
    fraction: Time,
}

impl ClockTime {
    /// The zero point of the transport.
    pub const ZERO: Self = Self {
        ticks: 0,
        fraction: Time::ZERO,
    };

    /// Creates a new clock time from whole ticks and a fractional tick.
    ///
    /// The fraction must be in the half-open range `[0, 1)`.
    pub fn new(ticks: u64, fraction: Time) -> Self {
        if fraction < Time::ZERO || fraction >= Time::ONE {
            panic!("clock fraction must be in [0, 1)");
        }

        Self { ticks, fraction }
    }

    /// Creates a clock time from a whole number of ticks.
    pub fn from_ticks_u64(ticks: u64) -> Self {
        Self::new(ticks, Time::ZERO)
    }

    /// Creates a clock time from an absolute musical time.
    ///
    /// This expects a non-negative time value.
    pub fn from_time(time: Time) -> Self {
        if time < Time::ZERO {
            panic!("clock time cannot be created from a negative musical time");
        }

        let whole_ticks = time.floor();
        let whole_time = Time::whole_number(whole_ticks);
        let fraction = time - whole_time;

        Self::new(
            u64::try_from(whole_ticks).expect("clock time exceeds the supported tick range"),
            fraction,
        )
    }

    /// Returns the whole-tick component.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Returns the fractional-tick component.
    pub fn fraction(&self) -> Time {
        self.fraction
    }

    /// Returns this clock time as the app's musical `Time`.
    pub fn as_time(&self) -> Time {
        Time::whole_number(
            i64::try_from(self.ticks).expect("clock time exceeds the supported musical range"),
        ) + self.fraction
    }

    /// Returns `true` if this is the zero point.
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    /// Adds a musical duration to this clock time.
    ///
    /// Returns `None` if the result would become negative.
    pub fn checked_add(self, rhs: Time) -> Option<Self> {
        let sum = self.as_time() + rhs;
        (sum >= Time::ZERO).then(|| Self::from_time(sum))
    }

    /// Subtracts a musical duration from this clock time.
    ///
    /// Returns `None` if the result would become negative.
    pub fn checked_sub(self, rhs: Time) -> Option<Self> {
        let difference = self.as_time() - rhs;
        (difference >= Time::ZERO).then(|| Self::from_time(difference))
    }

    /// Subtracts a musical duration, saturating at zero.
    pub fn saturating_sub(self, rhs: Time) -> Self {
        self.checked_sub(rhs).unwrap_or(Self::ZERO)
    }
}

impl Add<Time> for ClockTime {
    type Output = Self;

    fn add(self, rhs: Time) -> Self::Output {
        self.checked_add(rhs)
            .expect("adding that musical duration would produce a negative clock time")
    }
}

impl AddAssign<Time> for ClockTime {
    fn add_assign(&mut self, rhs: Time) {
        *self = *self + rhs;
    }
}

impl Sub<Time> for ClockTime {
    type Output = Self;

    fn sub(self, rhs: Time) -> Self::Output {
        self.checked_sub(rhs)
            .expect("subtracting that musical duration would produce a negative clock time")
    }
}

impl SubAssign<Time> for ClockTime {
    fn sub_assign(&mut self, rhs: Time) {
        *self = *self - rhs;
    }
}

impl PartialOrd for ClockTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ClockTime {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ticks
            .cmp(&other.ticks)
            .then(self.fraction.cmp(&other.fraction))
    }
}

/// A snapshot of the transport at a particular wall-clock instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSnapshot {
    /// Wall-clock instant that was sampled.
    pub instant: Instant,
    /// Musical time at that instant.
    pub time: ClockTime,
}

/// Snapshot information about the transport clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockInfo {
    /// Whether the transport is actively ticking.
    pub ticking: bool,
    /// Current clock time if known.
    pub time: ClockTime,
}

/// Describes whether something tied to a clock time should start now, later,
/// or never.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WhenToStart {
    /// The action should start immediately.
    Now,
    /// The action should wait for a later time.
    Later,
    /// The action cannot start because no transport is available.
    Never,
}

/// Determines whether a clock-tied action should start now, later, or never.
#[must_use]
pub fn when_to_start(transport: Option<ClockInfo>, time: ClockTime) -> WhenToStart {
    match transport {
        Some(transport) if transport.ticking && transport.time >= time => WhenToStart::Now,
        Some(_) => WhenToStart::Later,
        None => WhenToStart::Never,
    }
}

/// The rate that an application clock advances.
///
/// This enum keeps `tick` wording intentionally:
/// the project schedules in cycles, but "tick" is still a useful lower-level
/// notion for transport bookkeeping and non-musical timing layers.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClockSpeed {
    /// The clock advances one tick every `x` seconds.
    SecondsPerTick(f64),
    /// The clock advances `x` ticks per second.
    TicksPerSecond(f64),
    /// The clock advances `x` ticks per minute.
    TicksPerMinute(f64),
}

impl ClockSpeed {
    /// Returns the speed as seconds per tick.
    #[must_use]
    pub fn as_seconds_per_tick(self) -> f64 {
        match self {
            Self::SecondsPerTick(seconds_per_tick) => seconds_per_tick,
            Self::TicksPerSecond(ticks_per_second) => 1.0 / ticks_per_second,
            Self::TicksPerMinute(ticks_per_minute) => 60.0 / ticks_per_minute,
        }
    }

    /// Returns the speed as ticks per second.
    #[must_use]
    pub fn as_ticks_per_second(self) -> f64 {
        match self {
            Self::SecondsPerTick(seconds_per_tick) => 1.0 / seconds_per_tick,
            Self::TicksPerSecond(ticks_per_second) => ticks_per_second,
            Self::TicksPerMinute(ticks_per_minute) => ticks_per_minute / 60.0,
        }
    }

    /// Returns the speed as ticks per minute.
    #[must_use]
    pub fn as_ticks_per_minute(self) -> f64 {
        match self {
            Self::SecondsPerTick(seconds_per_tick) => 60.0 / seconds_per_tick,
            Self::TicksPerSecond(ticks_per_second) => ticks_per_second * 60.0,
            Self::TicksPerMinute(ticks_per_minute) => ticks_per_minute,
        }
    }

    /// Returns the speed as cycles per second.
    #[must_use]
    pub fn as_cycles_per_second(self) -> f64 {
        self.as_ticks_per_second()
    }

    /// Returns the speed as cycles per minute.
    #[must_use]
    pub fn as_cycles_per_minute(self) -> f64 {
        self.as_ticks_per_minute()
    }

    /// Creates a clock speed from cycles per second.
    #[must_use]
    pub fn from_cycles_per_second(cycles_per_second: f64) -> Self {
        Self::TicksPerSecond(cycles_per_second)
    }

    /// Creates a clock speed from cycles per minute.
    #[must_use]
    pub fn from_cycles_per_minute(cycles_per_minute: f64) -> Self {
        Self::TicksPerMinute(cycles_per_minute)
    }
}

/// Describes when an action should occur.
///
/// This lives next to the canonical clock types because it directly depends on
/// [`ClockTime`], even if higher-level modules use it for audio or parameter
/// transitions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum StartTime {
    /// The action should occur immediately.
    #[default]
    Immediate,
    /// The action should occur a certain amount of time from now.
    Delayed(Duration),
    /// The action should occur when the transport reaches a specific clock time.
    ClockTime(ClockTime),
}

impl From<Duration> for StartTime {
    fn from(value: Duration) -> Self {
        Self::Delayed(value)
    }
}

impl From<ClockTime> for StartTime {
    fn from(value: ClockTime) -> Self {
        Self::ClockTime(value)
    }
}

impl StartTime {
    /// Advances the start condition by wall-clock delta `dt`.
    ///
    /// Returns `true` when the action should never start because its required
    /// transport is unavailable.
    pub fn update(&mut self, dt: f64, transport: Option<ClockInfo>) -> bool {
        match self {
            Self::Immediate => {}
            Self::Delayed(time_remaining) => {
                *time_remaining =
                    time_remaining.saturating_sub(Duration::from_secs_f64(dt.max(0.0)));
                if time_remaining.is_zero() {
                    *self = Self::Immediate;
                }
            }
            Self::ClockTime(clock_time) => match when_to_start(transport, *clock_time) {
                WhenToStart::Now => *self = Self::Immediate,
                WhenToStart::Later => {}
                WhenToStart::Never => return true,
            },
        }

        false
    }
}

/// Transport clock used by the scheduler and playback layers.
///
/// This keeps:
/// - a wall-clock anchor (`Instant`)
/// - a musical-time anchor (`ClockTime`)
/// - the transport rate in cycles per second (`Time`)
///
/// That gives the app a precise bridge between musical time and wall time.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    started_at: Instant,
    started_at_time: ClockTime,
    cycles_per_second: Time,
}

impl Clock {
    /// Creates a clock starting at the given wall-clock instant and musical zero.
    pub fn new(cycles_per_second: Time, started_at: Instant) -> Self {
        Self::starting_at(cycles_per_second, started_at, ClockTime::ZERO)
    }

    /// Creates a clock starting at the given wall-clock instant and musical time.
    ///
    /// # Panics
    ///
    /// Panics if `cycles_per_second <= 0`.
    pub fn starting_at(
        cycles_per_second: Time,
        started_at: Instant,
        started_at_time: ClockTime,
    ) -> Self {
        if cycles_per_second <= Time::ZERO {
            panic!("cycles_per_second must be positive");
        }

        Self {
            started_at,
            started_at_time,
            cycles_per_second,
        }
    }

    /// Changes the rate using the current wall-clock instant as the pivot.
    pub fn set_cycles_per_second(&mut self, cps: Time) {
        self.set_cycles_per_second_at(cps, Instant::now());
    }

    /// Changes the rate while preserving the musical time at `instant`.
    ///
    /// # Panics
    ///
    /// Panics if `cps <= 0`.
    pub fn set_cycles_per_second_at(&mut self, cps: Time, instant: Instant) {
        if cps <= Time::ZERO {
            panic!("cycles_per_second must be positive");
        }

        let current_time = self.snapshot_at(instant).time;
        self.started_at = instant;
        self.started_at_time = current_time;
        self.cycles_per_second = cps;
    }

    /// Changes the rate and re-anchors the clock at an explicit musical time.
    ///
    /// # Panics
    ///
    /// Panics if `cps <= 0`.
    pub fn set_cycles_per_second_from_time(&mut self, cps: Time, time: ClockTime) {
        if cps <= Time::ZERO {
            panic!("cycles_per_second must be positive");
        }

        self.cycles_per_second = cps;
        self.reset_at(time);
    }
    /// Returns the wall-clock instant where this transport was anchored.
    pub fn started_at(&self) -> Instant {
        self.started_at
    }

    /// Returns the musical time where this transport was anchored.
    pub fn started_at_time(&self) -> ClockTime {
        self.started_at_time
    }

    /// Resets the clock to zero at the current wall-clock instant.
    pub fn reset(&mut self) {
        self.reset_at(ClockTime::ZERO);
    }

    /// Resets the clock to `at` at the current wall-clock instant.
    pub fn reset_at(&mut self, at: ClockTime) {
        self.reset_at_instant(at, Instant::now());
    }

    /// Resets the clock to `at` anchored at `instant`.
    pub fn reset_at_instant(&mut self, at: ClockTime, instant: Instant) {
        self.started_at = instant;
        self.started_at_time = at;
    }

    /// Returns the transport rate in cycles per second.
    pub fn cycles_per_second(&self) -> Time {
        self.cycles_per_second
    }

    /// Returns the elapsed wall-clock time since the transport start.
    pub fn elapsed(&self) -> Duration {
        self.elapsed_at(Instant::now())
    }

    /// Returns the elapsed wall-clock time from the transport start to `instant`.
    ///
    /// If `instant` is before the start, this returns zero.
    pub fn elapsed_at(&self, instant: Instant) -> Duration {
        instant
            .checked_duration_since(self.started_at)
            .unwrap_or(Duration::ZERO)
    }

    /// Converts a wall-clock duration into musical cycles using the transport rate.
    pub fn duration_to_cycles(&self, duration: Duration) -> Time {
        map_duration_to_cycles(duration, self.cycles_per_second)
    }

    /// Converts musical cycles into a wall-clock duration using the transport rate.
    ///
    /// This expects a non-negative cycle count.
    pub fn cycles_to_duration(&self, cycles: Time) -> Duration {
        if cycles < Time::ZERO {
            panic!("cycles_to_duration requires a non-negative cycle amount");
        }
        if cycles == Time::ZERO {
            return Duration::ZERO;
        }

        let seconds = cycles / self.cycles_per_second;
        rational_seconds_to_duration(seconds)
    }

    /// Returns a snapshot of the current clock state.
    pub fn snapshot(&self) -> ClockSnapshot {
        self.snapshot_at(Instant::now())
    }

    /// Returns a snapshot of the clock state at a specific wall-clock instant.
    pub fn snapshot_at(&self, instant: Instant) -> ClockSnapshot {
        let time = if let Some(elapsed) = instant.checked_duration_since(self.started_at) {
            self.started_at_time + self.duration_to_cycles(elapsed)
        } else {
            let rewind = self.started_at.duration_since(instant);
            self.started_at_time
                .saturating_sub(self.duration_to_cycles(rewind))
        };

        ClockSnapshot { instant, time }
    }

    /// Returns the current musical clock time.
    pub fn now(&self) -> ClockTime {
        self.snapshot().time
    }

    /// Returns the current musical time in the app's `Time` format.
    pub fn now_time(&self) -> Time {
        self.now().as_time()
    }

    /// Converts an absolute musical cycle position into a `ClockTime`.
    pub fn time_for_cycle(&self, cycle: Time) -> ClockTime {
        ClockTime::from_time(cycle)
    }

    /// Converts a `ClockTime` into a wall-clock instant.
    pub fn instant_for(&self, time: ClockTime) -> Instant {
        if time >= self.started_at_time {
            let delta_cycles = time.as_time() - self.started_at_time.as_time();
            self.started_at
                .checked_add(self.cycles_to_duration(delta_cycles))
                .expect("clock target is too far in the future")
        } else {
            let delta_cycles = self.started_at_time.as_time() - time.as_time();
            self.started_at
                .checked_sub(self.cycles_to_duration(delta_cycles))
                .expect("clock target is too far in the past")
        }
    }

    /// Converts an absolute musical cycle position into a wall-clock instant.
    pub fn deadline_for_cycle(&self, cycle: Time) -> Instant {
        self.instant_for(self.time_for_cycle(cycle))
    }

    /// Returns the musical time for an arbitrary wall-clock instant.
    pub fn musical_time_at(&self, instant: Instant) -> Time {
        self.snapshot_at(instant).time.as_time()
    }
}

fn rational_seconds_to_duration(seconds: Time) -> Duration {
    const NANOS_PER_SEC: i128 = 1_000_000_000;

    if seconds < Time::ZERO {
        panic!("duration conversion requires a non-negative time value");
    }
    if seconds == Time::ZERO {
        return Duration::ZERO;
    }

    let numerator = i128::from(seconds.numerator());
    let denominator = i128::from(seconds.denominator());

    let total_nanos = (numerator * NANOS_PER_SEC + denominator / 2) / denominator;
    let secs = u64::try_from(total_nanos / NANOS_PER_SEC)
        .expect("duration exceeds the supported wall-clock range");
    let nanos = u32::try_from(total_nanos % NANOS_PER_SEC)
        .expect("duration nanoseconds exceed the supported range");

    Duration::new(secs, nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_time_round_trips_with_fraction() {
        let time = Time::new(7, 4);
        let clock_time = ClockTime::from_time(time);

        assert_eq!(clock_time.ticks(), 1);
        assert_eq!(clock_time.fraction(), Time::new(3, 4));
        assert_eq!(clock_time.as_time(), time);
    }

    #[test]
    fn clock_snapshot_tracks_fractional_cycles() {
        let start = Instant::now();
        let clock = Clock::new(Time::ONE, start);

        let snapshot = clock.snapshot_at(
            start
                .checked_add(Duration::from_millis(1500))
                .expect("test instant overflowed"),
        );

        assert_eq!(snapshot.time.as_time(), Time::new(3, 2));
    }

    #[test]
    fn clock_translates_between_musical_and_wall_time() {
        let start = Instant::now();
        let clock = Clock::new(Time::new(1, 2), start);

        let musical_target = Time::new(3, 4);
        let deadline = clock.deadline_for_cycle(musical_target);
        let expected = start
            .checked_add(Duration::from_millis(1500))
            .expect("test instant overflowed");

        assert_eq!(deadline, expected);
        assert_eq!(
            clock.time_for_cycle(musical_target).as_time(),
            musical_target
        );
    }

    #[test]
    fn sub_millisecond_duration_keeps_precision() {
        let elapsed_time = map_duration_to_cycles(Duration::from_nanos(1_500_000), Time::ONE);
        assert_eq!(elapsed_time, Time::new(3, 2000));
    }

    #[test]
    fn cycle_aliases_match_tick_conversions() {
        let speed = ClockSpeed::from_cycles_per_second(1.5);

        assert_eq!(speed.as_cycles_per_second(), 1.5);
        assert_eq!(speed.as_cycles_per_minute(), 90.0);
    }

    #[test]
    fn changing_cycles_per_second_preserves_current_musical_time() {
        let start = Instant::now();
        let change_at = start + Duration::from_secs(10);
        let one_second_later = change_at + Duration::from_secs(1);
        let mut clock = Clock::new(Time::ONE, start);

        clock.set_cycles_per_second_at(Time::new(2, 1), change_at);

        assert_eq!(
            clock.snapshot_at(change_at).time.as_time(),
            Time::new(10, 1)
        );
        assert_eq!(
            clock.snapshot_at(one_second_later).time.as_time(),
            Time::new(12, 1)
        );
    }

    #[test]
    fn reset_at_instant_starts_counting_from_given_musical_time() {
        let start = Instant::now();
        let reset_at = start + Duration::from_secs(5);
        let one_second_later = reset_at + Duration::from_secs(1);
        let mut clock = Clock::new(Time::ONE, start);

        clock.reset_at_instant(ClockTime::from_time(Time::new(7, 2)), reset_at);

        assert_eq!(clock.snapshot_at(reset_at).time.as_time(), Time::new(7, 2));
        assert_eq!(
            clock.snapshot_at(one_second_later).time.as_time(),
            Time::new(9, 2)
        );
    }

    #[test]
    fn delayed_start_time_becomes_immediate_after_waiting() {
        let mut start_time = StartTime::Delayed(Duration::from_millis(50));

        assert!(!start_time.update(0.025, None));
        assert_eq!(start_time, StartTime::Delayed(Duration::from_millis(25)));

        assert!(!start_time.update(0.025, None));
        assert_eq!(start_time, StartTime::Immediate);
    }

    #[test]
    fn clock_time_start_activates_when_transport_reaches_target() {
        let target = ClockTime::from_time(Time::new(1, 2));
        let mut start_time = StartTime::ClockTime(target);
        let waiting_transport = ClockInfo {
            ticking: true,
            time: ClockTime::from_time(Time::new(1, 4)),
        };

        assert!(!start_time.update(0.0, Some(waiting_transport)));
        assert_eq!(start_time, StartTime::ClockTime(target));

        let ready_transport = ClockInfo {
            ticking: true,
            time: target,
        };

        assert!(!start_time.update(0.0, Some(ready_transport)));
        assert_eq!(start_time, StartTime::Immediate);
    }
}
