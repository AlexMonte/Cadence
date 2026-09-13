//! Device-rate insert preparation belongs to the control side of the queue.
//! Mutable state is a fixed array owned exclusively by each rendered voice.
use super::{
    Frame,
    voice::{BiquadCoefficients, BiquadFilter, FilterMode},
};
use crate::domain::inserts::{InsertChain, InsertEffect};

#[derive(Debug, Clone, Copy)]
enum PreparedInsert {
    Filter(BiquadCoefficients),
    Drive {
        amount: f32,
        wet: f32,
        output_gain: f32,
    },
}

/// Immutable coefficients and topology, never changed by frame processing.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PreparedInsertChain([Option<PreparedInsert>; InsertChain::MAX_EFFECTS]);
impl PreparedInsertChain {
    pub(crate) fn new(chain: &InsertChain, sample_rate: u32) -> Self {
        let mut result = Self::default();
        for (slot, effect) in result.0.iter_mut().zip(chain.effects()) {
            *slot = Some(match effect {
                InsertEffect::LowPass(filter) => PreparedInsert::Filter(BiquadCoefficients::new(
                    FilterMode::LowPass,
                    filter.cutoff_hz() as f32,
                    filter.resonance(),
                    sample_rate,
                )),
                InsertEffect::HighPass(filter) => PreparedInsert::Filter(BiquadCoefficients::new(
                    FilterMode::HighPass,
                    filter.cutoff_hz() as f32,
                    filter.resonance(),
                    sample_rate,
                )),
                InsertEffect::Drive(drive) => PreparedInsert::Drive {
                    amount: drive.amount() as f32,
                    wet: drive.wet() as f32,
                    output_gain: drive.output_gain() as f32,
                },
            });
        }
        result
    }
}

#[derive(Debug, Clone)]
enum ActiveInsert {
    Filter(BiquadFilter),
    Drive {
        amount: f32,
        wet: f32,
        output_gain: f32,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct InsertRuntime([Option<ActiveInsert>; InsertChain::MAX_EFFECTS]);
impl InsertRuntime {
    pub(crate) fn new(prepared: PreparedInsertChain) -> Self {
        Self(prepared.0.map(|effect| {
            effect.map(|effect| match effect {
                PreparedInsert::Filter(coefficients) => {
                    ActiveInsert::Filter(BiquadFilter::from_coefficients(coefficients))
                }
                PreparedInsert::Drive {
                    amount,
                    wet,
                    output_gain,
                } => ActiveInsert::Drive {
                    amount,
                    wet,
                    output_gain,
                },
            })
        }))
    }

    pub(crate) fn process(&mut self, mut frame: Frame) -> Frame {
        for effect in self.0.iter_mut().flatten() {
            frame = match effect {
                ActiveInsert::Filter(filter) => filter.process(frame),
                ActiveInsert::Drive {
                    amount,
                    wet,
                    output_gain,
                } => {
                    let process = |sample: f32| {
                        let shaped = if *amount == 0.0 || *wet == 0.0 {
                            sample
                        } else {
                            // Bounded rational soft saturation. It approaches
                            // identity continuously as amount tends to zero.
                            let drive = *amount * 31.0;
                            let saturated = sample * (1.0 + drive) / (1.0 + drive * sample.abs());
                            sample + *wet * (saturated - sample)
                        };
                        shaped * *output_gain
                    };
                    Frame::new(process(frame.left), process(frame.right))
                }
            };
        }
        frame
    }
}
