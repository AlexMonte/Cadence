//! Bounded native audio delivery. DSP preparation and reclamation run on a
//! worker; the device callback performs only ring reads, scalar arithmetic and
//! atomics. The worker may lead the device by at most `capacity` frames.

#[cfg(not(target_arch = "wasm32"))]
use super::AudioRenderer;
use super::Frame;
use super::renderer::RenderClock;
use rtrb::Consumer;
#[cfg(not(target_arch = "wasm32"))]
use rtrb::{Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, Copy)]
struct OutputFrame {
    frame: Frame,
    position: u64,
    generation: u64,
}

/// Shared, allocation-free counters for the host's audio status display.
#[derive(Clone)]
pub struct OutputStatus {
    underruns: Arc<AtomicU64>,
    clock: Arc<RenderClock>,
}
impl OutputStatus {
    /// Number of requested device frames for which no current output was ready.
    pub fn underrun_frames(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }
    /// Absolute frame count requested by the device.
    pub fn played_frames(&self) -> u64 {
        self.clock.played.load(Ordering::Acquire)
    }
}

/// Callback-owned, preallocated output reader. Dropping it stops its worker.
pub struct AudioOutput {
    consumer: Consumer<OutputFrame>,
    pending: Option<OutputFrame>,
    status: OutputStatus,
    running: Arc<AtomicBool>,
    generation: u64,
    position: u64,
    fade_from: Frame,
    last_frame: Frame,
    fade_remaining: usize,
    fade_frames: usize,
}
impl AudioOutput {
    /// Returns shared output counters.
    pub fn status(&self) -> OutputStatus {
        self.status.clone()
    }
    /// Returns the next stereo frame without allocation or blocking.
    pub fn next_frame(&mut self) -> Frame {
        let generation = self.status.clock.generation.load(Ordering::Acquire);
        if generation != self.generation {
            self.generation = generation;
            self.fade_from = self.last_frame;
            self.fade_remaining =
                if self.status.clock.panic_generation.load(Ordering::Acquire) == generation {
                    0
                } else {
                    self.fade_frames
                };
        }
        let mut frame = Frame::ZERO;
        let mut available = false;
        while let Some(next) = self.pending.take().or_else(|| self.consumer.pop().ok()) {
            if next.position < self.position || next.generation != generation {
                continue;
            }
            if next.position > self.position {
                self.pending = Some(next);
                break;
            }
            frame = next.frame;
            available = true;
            break;
        }
        if !available {
            self.status.underruns.fetch_add(1, Ordering::Relaxed);
        }
        if self.fade_remaining > 0 {
            let weight = self.fade_remaining as f32 / self.fade_frames as f32;
            frame = frame * (1.0 - weight) + self.fade_from * weight;
            self.fade_remaining -= 1;
        }
        self.last_frame = frame;
        self.position += 1;
        self.status
            .clock
            .played
            .store(self.position, Ordering::Release);
        frame
    }
}
impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

/// Lifetime guard for a native render worker. Drop it off the audio callback.
#[cfg(not(target_arch = "wasm32"))]
pub struct AudioOutputWorker {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for AudioOutputWorker {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Starts a worker with a fixed frame bound. No DSP work depends on UI updates.
#[cfg(not(target_arch = "wasm32"))]
pub fn start_output_worker(
    renderer: AudioRenderer,
    capacity: usize,
) -> std::io::Result<(AudioOutput, AudioOutputWorker)> {
    if capacity == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "audio output capacity must be positive",
        ));
    }
    let (mut producer, output) = output_ring(&renderer, capacity);
    let running = output.running.clone();
    let worker_running = running.clone();
    let thread = std::thread::Builder::new()
        .name("cadence-audio".into())
        .spawn(move || {
            let mut renderer = renderer;
            let mut scratch = [Frame::ZERO; 128];
            while worker_running.load(Ordering::Acquire) {
                let count = producer.slots().min(scratch.len());
                if count == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
                produce(&mut renderer, &mut producer, &mut scratch[..count]);
            }
        })?;
    Ok((
        output,
        AudioOutputWorker {
            running,
            thread: Some(thread),
        },
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn output_ring(renderer: &AudioRenderer, capacity: usize) -> (Producer<OutputFrame>, AudioOutput) {
    let clock = renderer.command_sender().clock;
    clock.device_driven.store(true, Ordering::Release);
    let (producer, consumer) = RingBuffer::new(capacity);
    let status = OutputStatus {
        underruns: Arc::new(AtomicU64::new(0)),
        clock,
    };
    let output = AudioOutput {
        consumer,
        pending: None,
        status,
        running: Arc::new(AtomicBool::new(true)),
        generation: 0,
        position: renderer.frame_position(),
        fade_from: Frame::ZERO,
        last_frame: Frame::ZERO,
        fade_remaining: 0,
        fade_frames: (renderer.sample_rate() as usize * 8 / 1000).max(1),
    };
    (producer, output)
}

#[cfg(not(target_arch = "wasm32"))]
fn produce(
    renderer: &mut AudioRenderer,
    producer: &mut Producer<OutputFrame>,
    scratch: &mut [Frame],
) {
    let clock = renderer.command_sender().clock;
    // After an underrun, advance DSP to the device's present instead of replaying late audio.
    let played = clock.played.load(Ordering::Acquire);
    while renderer.frame_position() < played {
        let count = ((played - renderer.frame_position()) as usize).min(scratch.len());
        renderer.render(&mut scratch[..count]);
    }
    let start = renderer.frame_position();
    renderer.render(scratch);
    let generation = renderer.active_generation();
    for (index, frame) in scratch.iter().copied().enumerate() {
        // Only the worker pushes; `scratch` never exceeds the available slots.
        let _ = producer.push(OutputFrame {
            frame,
            position: start + index as u64,
            generation,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::audio::AudioRendererSettings;
    #[test]
    fn output_is_device_driven_and_ring_bounds_worker_lead() {
        let (audio, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(48_000, 16)).unwrap();
        let (mut producer, mut output) = output_ring(&renderer, 8);
        produce(&mut renderer, &mut producer, &mut [Frame::ZERO; 8]);
        assert_eq!(audio.next_render_frame(), 8);
        assert_eq!(audio.playback_frame(), 0);
        assert_eq!(producer.slots(), 0);
        for _ in 0..3 {
            output.next_frame();
        }
        assert_eq!(audio.playback_frame(), 3);
        assert_eq!(producer.slots(), 3);
        assert_eq!(output.status().underrun_frames(), 0);
    }
    #[test]
    fn preserves_stereo_and_discards_stale_output_after_panic() {
        let (audio, renderer) =
            AudioRenderer::split(AudioRendererSettings::new(48_000, 16)).unwrap();
        let (mut producer, mut output) = output_ring(&renderer, 8);
        for position in 0..4 {
            producer
                .push(OutputFrame {
                    frame: Frame::new(0.25, -0.75),
                    position,
                    generation: 0,
                })
                .unwrap();
        }
        assert_eq!(output.next_frame(), Frame::new(0.25, -0.75));
        audio.panic();
        assert_eq!(output.next_frame(), Frame::ZERO);
        assert_eq!(output.next_frame(), Frame::ZERO);
    }
    #[test]
    fn underrun_recovery_does_not_replay_old_frames() {
        let (_, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(44_100, 16)).unwrap();
        let (mut producer, mut output) = output_ring(&renderer, 8);
        for _ in 0..10 {
            assert_eq!(output.next_frame(), Frame::ZERO);
        }
        produce(&mut renderer, &mut producer, &mut [Frame::ZERO; 8]);
        assert_eq!(renderer.frame_position(), 18);
        output.next_frame();
        assert_eq!(output.status().underrun_frames(), 10);
    }
}
