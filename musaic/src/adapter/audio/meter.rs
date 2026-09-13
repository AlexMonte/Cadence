//! Delivery metering: only the native output callback publishes samples here.
//! The callback aggregates a block locally, then touches a fixed set of atomics.
use bevy::prelude::*;

#[derive(Resource, Default, Debug)]
pub struct MasterMeter {
    /// Linear sample peaks with a short visual decay; not perceived loudness.
    pub left: f32,
    pub right: f32,
    pub clipped: bool,
    pub receiving: bool,
    #[cfg(any(not(target_arch = "wasm32"), test))]
    idle_seconds: f32,
    #[cfg(any(not(target_arch = "wasm32"), test))]
    callbacks: u64,
}

#[derive(Message)]
pub struct ClearMasterClip;

impl MasterMeter {
    pub fn db(peak: f32) -> Option<f32> {
        (peak.is_finite() && peak > 0.000_001).then(|| 20.0 * peak.log10())
    }

    #[cfg(any(not(target_arch = "wasm32"), test))]
    fn update(&mut self, reading: Reading, seconds: f32) {
        if reading.callbacks != self.callbacks {
            self.idle_seconds = 0.0;
            self.callbacks = reading.callbacks;
        } else {
            self.idle_seconds += seconds;
        }
        self.receiving = self.callbacks > 0 && self.idle_seconds < 0.5;
        let decay = (-seconds * 5.0).exp();
        self.left = if self.receiving {
            reading.left.max(self.left * decay)
        } else {
            0.0
        };
        self.right = if self.receiving {
            reading.right.max(self.right * decay)
        } else {
            0.0
        };
        self.clipped = reading.clipped;
    }
}

#[cfg(any(not(target_arch = "wasm32"), test))]
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

#[cfg(any(not(target_arch = "wasm32"), test))]
#[derive(Default)]
pub(super) struct MeterBridge {
    left: AtomicU32,
    right: AtomicU32,
    clipped: AtomicBool,
    callbacks: AtomicU64,
}

#[cfg(any(not(target_arch = "wasm32"), test))]
#[derive(Default)]
struct Reading {
    left: f32,
    right: f32,
    clipped: bool,
    callbacks: u64,
}

#[cfg(any(not(target_arch = "wasm32"), test))]
impl MeterBridge {
    pub(super) fn publish(&self, left: f32, right: f32) {
        let left = if left.is_finite() {
            left.max(0.0)
        } else {
            f32::MAX
        };
        let right = if right.is_finite() {
            right.max(0.0)
        } else {
            f32::MAX
        };
        // Positive finite floats have the same ordering as their bit patterns.
        // Max + swap retains brief peaks between UI polls without a queue/lock.
        self.left.fetch_max(left.to_bits(), Ordering::Relaxed);
        self.right.fetch_max(right.to_bits(), Ordering::Relaxed);
        if left > 1.0 || right > 1.0 {
            self.clipped.store(true, Ordering::Relaxed);
        }
        self.callbacks.fetch_add(1, Ordering::Release);
    }

    pub(super) fn sample(&self, meter: &mut MasterMeter, seconds: f32, clear: bool) {
        if clear {
            self.clipped.store(false, Ordering::Relaxed);
        }
        let callbacks = self.callbacks.load(Ordering::Acquire);
        meter.update(
            Reading {
                callbacks,
                left: f32::from_bits(self.left.swap(0, Ordering::Relaxed)),
                right: f32::from_bits(self.right.swap(0, Ordering::Relaxed)),
                clipped: self.clipped.load(Ordering::Relaxed),
            },
            seconds,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_peaks_survive_silent_blocks_until_consumed_and_clip_is_latched() {
        let bridge = MeterBridge::default();
        let mut meter = MasterMeter::default();
        bridge.publish(0.5, 1.25);
        bridge.publish(0.0, 0.0);
        bridge.sample(&mut meter, 0.1, false);
        assert_eq!((meter.left, meter.right), (0.5, 1.25));
        assert!(meter.clipped && meter.receiving);
        assert!((MasterMeter::db(0.5).unwrap() + 6.0206).abs() < 0.001);
        bridge.publish(0.0, 0.0);
        bridge.sample(&mut meter, 0.1, false);
        assert!(meter.left < 0.5 && meter.left > 0.0 && meter.clipped);
        bridge.sample(&mut meter, 0.1, true);
        assert!(!meter.clipped);
        bridge.publish(1.1, 0.0);
        bridge.sample(&mut meter, 0.1, false);
        assert!(meter.clipped);
    }
    #[test]
    fn unavailable_callback_cannot_be_confused_with_delivered_silence() {
        let bridge = MeterBridge::default();
        let mut meter = MasterMeter::default();
        bridge.sample(&mut meter, 0.1, false);
        assert!(!meter.receiving);
        bridge.publish(0.0, 0.0);
        bridge.sample(&mut meter, 0.1, false);
        assert!(meter.receiving);
        assert_eq!(MasterMeter::db(meter.left), None);
        bridge.sample(&mut meter, 0.6, false);
        assert!(!meter.receiving);
        assert_eq!((meter.left, meter.right), (0.0, 0.0));
        bridge.publish(0.2, 0.2);
        bridge.sample(&mut meter, 0.1, false);
        assert!(meter.receiving);
    }
}
