//! Active sample and synth voice renderers.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use std::{f32::consts::PI as PI32, f64::consts::PI};

use crate::{
    adapter::audio::{
        Frame, LoadedSampleTrigger, PlaybackPosition, Region, SampleBuffer, Transport,
        interpolate_frame,
        spatializer::{Spatializer, StereoVectorPanner},
    },
    application::{
        audio::{
            AudioRuntimeControlDelta, AudioRuntimeControlState, FilterPlan, RuntimeControlValue,
            SignalBinding, VoiceInstanceId, VoiceSpatial,
        },
        sample::{SampleEnvelope, SampleGainRamp},
        synth::SynthTrigger,
    },
    domain::{
        control::{CompressorSettings, ControlKey, DelaySettings, ReverbSettings, UnitValue},
        input::NoteNumber,
        intent::BuiltInSynthSource,
    },
};

static INVALID_REGION_WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

/// Per-output-frame modulation values derived from active [`SignalBinding`]s.
#[derive(Debug, Clone, Copy, Default)]
struct FrameModulation {
    gain: Option<f32>,
    playback_rate: Option<f64>,
    cutoff: Option<f32>,
}

/// Evaluates the active signal bindings for the current output frame.
///
/// Cycle time is recovered as `start_cycle + (rendered_frames / sample_rate) *
/// cps`. This is allocation-free and safe to call on the audio thread.
fn evaluate_frame_modulation(
    modulations: &[Option<SignalBinding>; 3],
    rendered_frames: usize,
    sample_rate: u32,
) -> FrameModulation {
    let mut output = FrameModulation::default();
    if modulations.is_empty() {
        return output;
    }

    let seconds = rendered_frames as f64 / sample_rate.max(1) as f64;
    for binding in modulations.iter().flatten() {
        let cycle_time = binding.start_cycle + seconds * binding.cps;
        let value = binding.signal.eval(cycle_time);
        match &binding.key {
            ControlKey::Gain => output.gain = Some(value as f32),
            ControlKey::PlaybackRate => output.playback_rate = Some(value),
            ControlKey::LowPassCutoff => output.cutoff = Some(value as f32),
            _ => {}
        }
    }
    output
}

fn voice_modulations(bindings: &[SignalBinding]) -> [Option<SignalBinding>; 3] {
    let keys = [
        ControlKey::Gain,
        ControlKey::PlaybackRate,
        ControlKey::LowPassCutoff,
    ];
    std::array::from_fn(|index| {
        bindings
            .iter()
            .find(|binding| binding.key == keys[index])
            .cloned()
    })
}

fn update_voice_modulations(
    bindings: &mut [Option<SignalBinding>; 3],
    delta: &AudioRuntimeControlDelta,
    clock: VoiceSpatial,
) {
    for (index, (key, change)) in [
        (ControlKey::Gain, delta.gain_signal),
        (ControlKey::PlaybackRate, delta.playback_rate_signal),
        (ControlKey::LowPassCutoff, delta.low_pass_signal),
    ]
    .into_iter()
    .enumerate()
    {
        match change {
            RuntimeControlValue::Keep => {}
            RuntimeControlValue::Reset => bindings[index] = None,
            RuntimeControlValue::Set(signal) => {
                bindings[index] = Some(SignalBinding {
                    key,
                    signal,
                    start_cycle: clock.start_cycle,
                    cps: clock.cps,
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
/// Active sample-playback voice owned by the renderer.
pub struct SampleVoice {
    voice_id: VoiceInstanceId,
    sample: Arc<SampleBuffer>,
    output_sample_rate: u32,
    transport: Transport,
    phase: f64,
    gain: f32,
    gain_ramp: Option<ActiveLinearGainRamp>,
    velocity: f32,
    spatial: VoiceSpatial,
    spatializer: StereoVectorPanner,
    base_pitch: Option<f64>,
    base_playback_rate: f64,
    resolved_rate_scale: f64,
    playback_limit_frames: Option<usize>,
    rendered_frames: usize,
    fade_out_remaining: Option<usize>,
    fade_out_total: usize,
    envelope: EnvelopeRuntime,
    runtime_controls: AudioRuntimeControlState,
    modulations: [Option<SignalBinding>; 3],
    low_pass: Option<BiquadFilter>,
    high_pass: Option<BiquadFilter>,
    compressor: Option<CompressorRuntime>,
    post_gain: f32,
    inserts: super::inserts::InsertRuntime,
    reverb: Option<ReverbSettings>,
    delay: Option<DelaySettings>,
    live_note: Option<NoteNumber>,
    finished: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VoiceFrame {
    pub dry: Frame,
    pub reverb: Option<(ReverbSettings, Frame)>,
    pub delay: Option<(DelaySettings, Frame)>,
}

impl VoiceFrame {
    pub const SILENT: Self = Self {
        dry: Frame::ZERO,
        reverb: None,
        delay: None,
    };
}

#[derive(Debug, Clone)]
struct Oscillator {
    source: BuiltInSynthSource,
    phase: f64,
    sample_rate: u32,
    noise_frame: u64,
}

impl Oscillator {
    fn new(source: BuiltInSynthSource, sample_rate: u32) -> Self {
        Self {
            source,
            phase: 0.0,
            sample_rate: sample_rate.max(1),
            noise_frame: 0,
        }
    }

    fn seek(&mut self, frame: u64, frequency_hz: f64) {
        self.noise_frame = frame;
        self.phase = (frame as f64 * frequency_hz / f64::from(self.sample_rate)).fract();
    }

    fn next_mono(&mut self, frequency_hz: f64) -> f32 {
        if matches!(self.source, BuiltInSynthSource::Noise) {
            // Counter-based SplitMix64: no shared random state, allocations,
            // per-voice identity dependence or iteration when seeking.
            let mut value = self.noise_frame.wrapping_add(0x9e37_79b9_7f4a_7c15);
            value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            value ^= value >> 31;
            self.noise_frame = self.noise_frame.wrapping_add(1);
            return ((value >> 40) as f32 / 8_388_608.0) - 1.0;
        }
        let increment = (frequency_hz / f64::from(self.sample_rate)).clamp(0.0, 0.5);
        if frequency_hz >= f64::from(self.sample_rate) * 0.5 {
            // Frequencies above the device's representable range are silent,
            // rather than being folded or clamped to a different audible pitch.
            self.phase = (self.phase + increment).fract();
            return 0.0;
        }
        let sample = match self.source {
            BuiltInSynthSource::Sine => (2.0_f64 * PI * self.phase).sin(),
            BuiltInSynthSource::Square => {
                let square = if self.phase < 0.5 { 1.0 } else { -1.0 };
                square + poly_blep(self.phase, increment)
                    - poly_blep((self.phase + 0.5).fract(), increment)
            }
            BuiltInSynthSource::Saw => 2.0 * self.phase - 1.0 - poly_blep(self.phase, increment),
            BuiltInSynthSource::Triangle => {
                1.0 - 4.0 * (self.phase - 0.5).abs()
                    + 4.0
                        * increment
                        * (poly_blamp(self.phase, increment)
                            - poly_blamp((self.phase + 0.5).fract(), increment))
            }
            BuiltInSynthSource::Noise => unreachable!("noise returned above"),
        } as f32;

        self.phase = (self.phase + increment).fract();
        sample
    }
}

// Two-sample polynomial residuals remove the sharp step/corner locally.
// These reduce aliasing; they are not ideal brick-wall bandlimiting.
fn poly_blep(phase: f64, width: f64) -> f64 {
    if width <= 0.0 {
        return 0.0;
    }
    if phase < width {
        let x = phase / width;
        2.0 * x - x * x - 1.0
    } else if phase > 1.0 - width {
        let x = (phase - 1.0) / width;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

fn poly_blamp(phase: f64, width: f64) -> f64 {
    if width <= 0.0 {
        return 0.0;
    }
    if phase < width {
        let x = 1.0 - phase / width;
        x * x * x / 3.0
    } else if phase > 1.0 - width {
        let x = 1.0 + (phase - 1.0) / width;
        x * x * x / 3.0
    } else {
        0.0
    }
}

#[derive(Clone)]
/// Active built-in synth voice owned by the renderer.
pub struct SynthVoice {
    voice_id: VoiceInstanceId,
    oscillator: Oscillator,
    base_pitch: f64,
    gain: f32,
    gain_ramp: Option<ActiveLinearGainRamp>,
    velocity: f32,
    spatial: VoiceSpatial,
    spatializer: StereoVectorPanner,
    envelope: EnvelopeRuntime,
    runtime_controls: AudioRuntimeControlState,
    modulations: [Option<SignalBinding>; 3],
    rendered_frames: usize,
    sample_rate: u32,
    low_pass: Option<BiquadFilter>,
    high_pass: Option<BiquadFilter>,
    compressor: Option<CompressorRuntime>,
    post_gain: f32,
    inserts: super::inserts::InsertRuntime,
    reverb: Option<ReverbSettings>,
    delay: Option<DelaySettings>,
    live_note: Option<NoteNumber>,
    finished: bool,
}

impl std::fmt::Debug for SynthVoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SynthVoice")
            .field("voice_id", &self.voice_id)
            .field("base_pitch", &self.base_pitch)
            .field("gain", &self.gain)
            .field("gain_ramp", &self.gain_ramp)
            .field("velocity", &self.velocity)
            .field("spatial", &self.spatial)
            .field("envelope", &self.envelope)
            .field("runtime_controls", &self.runtime_controls)
            .field("low_pass", &self.low_pass)
            .field("high_pass", &self.high_pass)
            .field("compressor", &self.compressor)
            .field("post_gain", &self.post_gain)
            .field("reverb", &self.reverb)
            .field("delay", &self.delay)
            .field("live_note", &self.live_note)
            .field("finished", &self.finished)
            .finish()
    }
}

impl SampleVoice {
    /// Creates a sample voice from a resolved sample trigger.
    ///
    /// Returns `None` when playback cannot start, for example because the
    /// output sample rate is zero or the sample buffer is empty.
    #[must_use]
    pub fn new(loaded: LoadedSampleTrigger, output_sample_rate: u32) -> Option<Self> {
        let inserts =
            super::inserts::PreparedInsertChain::new(&loaded.trigger.inserts, output_sample_rate);
        Self::with_prepared_inserts(loaded, output_sample_rate, inserts)
    }

    pub(super) fn with_prepared_inserts(
        loaded: LoadedSampleTrigger,
        output_sample_rate: u32,
        inserts: super::inserts::PreparedInsertChain,
    ) -> Option<Self> {
        if output_sample_rate == 0 || loaded.sample.is_empty() {
            return None;
        }

        let region = normalized_region(
            &loaded.sample,
            loaded.trigger.playback_start,
            loaded.trigger.playback_end,
        )?;
        let gain = sanitize_gain(loaded.trigger.gain);
        let velocity = loaded.trigger.velocity.value() as f32;
        let speed = loaded.trigger.playback_rate;
        if !speed.is_finite() || speed <= 0.0 || speed > 65_536.0 {
            return None;
        }
        let elapsed_seconds = loaded.trigger.envelope.elapsed().as_secs_f64();
        let playback_step = loaded.sample.sample_rate() as f64 / output_sample_rate as f64 * speed;
        let gain_ramp = loaded.trigger.gain_ramp.map(|gain_ramp| {
            let natural_output_frames =
                natural_output_frame_count(&region, loaded.sample.sample_rate(), playback_step);
            let total_output_frames = loaded
                .playback_limit
                .and_then(|limit| frames_for_duration(limit, output_sample_rate))
                .unwrap_or(natural_output_frames)
                .max(1);

            ActiveLinearGainRamp::new(gain_ramp, total_output_frames)
        });

        let mut voice = Self {
            voice_id: loaded.trigger.voice_id,
            output_sample_rate,
            transport: Transport::new(
                PlaybackPosition::default(),
                Some(region),
                loaded
                    .sustain_loop
                    .and_then(|sustain| sustain.region(loaded.sample.len()))
                    .or_else(|| loaded.trigger.looped.then_some(region)),
                loaded.trigger.reverse,
                loaded.sample.sample_rate(),
                loaded.sample.len(),
            ),
            sample: loaded.sample,
            phase: 0.0,
            gain,
            gain_ramp,
            velocity,
            spatial: loaded.trigger.spatial,
            spatializer: StereoVectorPanner::new(),
            base_pitch: loaded.trigger.pitch,
            base_playback_rate: speed,
            resolved_rate_scale: loaded.trigger.resolved_rate_scale,
            playback_limit_frames: loaded
                .playback_limit
                .and_then(|limit| frames_for_duration(limit, output_sample_rate)),
            rendered_frames: 0,
            fade_out_remaining: None,
            fade_out_total: 0,
            envelope: EnvelopeRuntime::new(loaded.trigger.envelope.clone(), output_sample_rate),
            runtime_controls: AudioRuntimeControlState {
                filters: FilterPlan {
                    low_pass_cutoff_hz: loaded.trigger.low_pass_cutoff_hz,
                    low_pass_resonance: loaded.trigger.low_pass_resonance,
                    high_pass_cutoff_hz: loaded.trigger.high_pass_cutoff_hz,
                    high_pass_resonance: loaded.trigger.high_pass_resonance,
                },
                ..loaded.trigger.runtime_controls
            },
            modulations: voice_modulations(&loaded.trigger.modulations),
            low_pass: loaded.trigger.low_pass_cutoff_hz.map(|cutoff| {
                BiquadFilter::low_pass(
                    cutoff as f32,
                    loaded.trigger.low_pass_resonance,
                    output_sample_rate,
                )
            }),
            high_pass: loaded.trigger.high_pass_cutoff_hz.map(|cutoff| {
                BiquadFilter::high_pass(
                    cutoff as f32,
                    loaded.trigger.high_pass_resonance,
                    output_sample_rate,
                )
            }),
            compressor: loaded
                .trigger
                .compressor
                .map(|settings| CompressorRuntime::new(settings, output_sample_rate)),
            post_gain: sanitize_gain(loaded.trigger.post_gain),
            inserts: super::inserts::InsertRuntime::new(inserts),
            reverb: loaded.trigger.reverb,
            delay: loaded.trigger.delay,
            live_note: loaded.trigger.live_note,
            finished: false,
        };
        // Keep the original region so a seek cannot shorten the loop. Source
        // position follows elapsed wall time and the resolved playback rate.
        voice.advance_by(
            elapsed_seconds * f64::from(output_sample_rate) * voice.current_playback_step(),
        );
        Some(voice)
    }

    /// Returns `true` when the voice is finished and can be removed.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.finished
            || self.sample.is_empty()
            || !self.transport.playing()
            || self.envelope.finished()
    }

    /// Returns the concrete voice identifier.
    #[must_use]
    pub fn voice_id(&self) -> VoiceInstanceId {
        self.voice_id
    }

    /// Returns the live note associated with this voice, if any.
    #[must_use]
    pub fn live_note(&self) -> Option<NoteNumber> {
        self.live_note
    }

    /// Starts the release stage of the voice envelope.
    pub fn release(&mut self) {
        self.envelope.release();
    }

    /// Applies a runtime control update to this voice.
    pub fn update_runtime_controls(&mut self, mut delta: AudioRuntimeControlDelta) {
        delta.playback_rate = match delta.playback_rate {
            RuntimeControlValue::Keep => RuntimeControlValue::Keep,
            RuntimeControlValue::Set(rate) => {
                RuntimeControlValue::Set(rate * self.resolved_rate_scale)
            }
            RuntimeControlValue::Reset => RuntimeControlValue::Set(self.resolved_rate_scale),
        };
        update_voice_modulations(&mut self.modulations, &delta, self.spatial);
        delta.apply_to(&mut self.runtime_controls);
        apply_voice_dsp_delta(
            &delta,
            self.runtime_controls.filters,
            self.output_sample_rate,
            VoiceDspState {
                low_pass: &mut self.low_pass,
                high_pass: &mut self.high_pass,
                compressor: &mut self.compressor,
                reverb: &mut self.reverb,
                delay: &mut self.delay,
            },
        );
        apply_voice_level_delta(
            &delta,
            &mut self.gain,
            &mut self.gain_ramp,
            &mut self.post_gain,
            Some(&mut self.base_playback_rate),
        );
    }

    /// Starts a short fade-out, typically for choke behavior.
    pub fn start_fade_out(&mut self, fade_out: Duration, output_sample_rate: u32) {
        let fade_frames = frames_for_duration(fade_out, output_sample_rate)
            .unwrap_or(1)
            .max(1);

        match self.fade_out_remaining {
            Some(current) if current <= fade_frames => {}
            _ => {
                self.fade_out_total = fade_frames;
                self.fade_out_remaining = Some(fade_frames);
            }
        }
    }

    pub(crate) fn render_next(&mut self) -> VoiceFrame {
        if self.finished() {
            self.finished = true;
            return VoiceFrame::SILENT;
        }

        let amplitude = self.envelope.current_level();
        if amplitude <= f32::EPSILON {
            self.consume_output_frame();
            return VoiceFrame::SILENT;
        }

        let modulation = evaluate_frame_modulation(
            &self.modulations,
            self.rendered_frames,
            self.output_sample_rate,
        );

        let mut frame = self.current_frame();
        frame *= self.current_gain() * self.velocity * amplitude;
        if let Some(gain) = modulation.gain {
            frame *= gain;
        }

        if let Some(filter) = &mut self.high_pass {
            frame = filter.process(frame);
        }

        if let Some(filter) = &mut self.low_pass {
            if let Some(cutoff) = modulation.cutoff {
                filter.set_cutoff(cutoff);
            }
            frame = filter.process(frame);
        }

        if let Some(compressor) = &mut self.compressor {
            frame = compressor.process(frame);
        }

        frame = self.inserts.process(frame);
        frame *= self.post_gain;
        if !self.runtime_controls.gate {
            frame = Frame::ZERO;
        }
        let position = self
            .spatial
            .position_at_frame(self.rendered_frames, self.output_sample_rate);
        frame = self.spatializer.place(frame, position);
        frame = frame.panned(self.runtime_controls.pan.clamp(-1.0, 1.0) as f32);

        if let Some(remaining) = self.fade_out_remaining {
            let denominator = self.fade_out_total.max(1) as f32;
            frame *= remaining as f32 / denominator;
        }

        let reverb = self
            .reverb
            .as_ref()
            .map(|settings| (*settings, frame * settings.amount().value() as f32));
        let delay = self
            .delay
            .as_ref()
            .map(|settings| (*settings, frame * settings.amount().value() as f32));

        let playback_step = self.current_playback_step() * modulation.playback_rate.unwrap_or(1.0);
        self.advance_by(playback_step);
        self.consume_output_frame();

        VoiceFrame {
            dry: frame,
            reverb,
            delay,
        }
    }

    fn current_gain(&self) -> f32 {
        self.gain_ramp
            .as_ref()
            .map_or(self.gain, ActiveLinearGainRamp::current_gain)
            * self.runtime_controls.expression.value() as f32
    }

    fn current_playback_step(&self) -> f64 {
        let cycle = self.spatial.start_cycle
            + self.rendered_frames as f64 / self.output_sample_rate as f64 * self.spatial.cps;
        let transpose = self.runtime_controls.transpose.eval(cycle);
        let bend = self
            .base_pitch
            .map_or(0.0, |_| self.runtime_controls.pitch_bend_semitones);
        let bend_ratio = 2.0_f64.powf((transpose + bend) / 12.0);
        self.sample.sample_rate() as f64 / self.output_sample_rate as f64
            * self.base_playback_rate
            * bend_ratio
    }

    fn current_frame(&self) -> Frame {
        if self.finished() {
            return Frame::ZERO;
        }

        let fraction = self.phase as f32;

        if self.transport.reverse() {
            interpolate_frame(
                self.frame_at(self.transport.sample_index(1)),
                self.frame_at(self.transport.sample_index(0)),
                self.frame_at(self.transport.sample_index(-1)),
                self.frame_at(self.transport.sample_index(-2)),
                fraction,
            )
        } else {
            interpolate_frame(
                self.frame_at(self.transport.sample_index(-1)),
                self.frame_at(self.transport.sample_index(0)),
                self.frame_at(self.transport.sample_index(1)),
                self.frame_at(self.transport.sample_index(2)),
                fraction,
            )
        }
    }

    fn frame_at(&self, index: usize) -> Frame {
        self.sample.frames()[index]
    }

    fn advance_by(&mut self, step: f64) {
        let position = self.phase + step.max(0.0);
        if !position.is_finite() {
            self.finished = true;
            return;
        }
        self.phase = position.fract();
        self.transport.advance_frames(position.floor() as usize);

        if !self.transport.playing() {
            self.finished = true;
        }
    }

    fn consume_output_frame(&mut self) {
        self.rendered_frames = self.rendered_frames.saturating_add(1);

        if let Some(limit) = self.playback_limit_frames
            && self.rendered_frames >= limit
        {
            self.finished = true;
        }

        if let Some(gain_ramp) = &mut self.gain_ramp {
            gain_ramp.advance();
        }

        if let Some(remaining) = &mut self.fade_out_remaining {
            *remaining = remaining.saturating_sub(1);
            if *remaining == 0 {
                self.finished = true;
            }
        }

        self.envelope.advance();

        if self.envelope.finished() {
            self.finished = true;
        }
    }
}

impl SynthVoice {
    /// Creates a synth voice from a synth trigger.
    ///
    /// Returns `None` when playback cannot start, for example because the
    /// output sample rate is zero or the trigger pitch does not map to a valid
    /// frequency.
    #[must_use]
    pub fn new(trigger: SynthTrigger, output_sample_rate: u32) -> Option<Self> {
        let inserts =
            super::inserts::PreparedInsertChain::new(&trigger.inserts, output_sample_rate);
        Self::with_prepared_inserts(trigger, output_sample_rate, inserts)
    }

    pub(super) fn with_prepared_inserts(
        trigger: SynthTrigger,
        output_sample_rate: u32,
        inserts: super::inserts::PreparedInsertChain,
    ) -> Option<Self> {
        if output_sample_rate == 0 {
            return None;
        }

        let initial_frequency_hz = midi_note_to_hz(
            trigger.pitch
                + trigger
                    .runtime_controls
                    .transpose
                    .eval(trigger.spatial.start_cycle)
                + trigger.runtime_controls.pitch_bend_semitones,
        )?;
        if !initial_frequency_hz.is_finite() || initial_frequency_hz <= 0.0 {
            return None;
        }

        let gain_ramp = trigger.gain_ramp.and_then(|gain_ramp| {
            gain_ramp_total_frames(&trigger.envelope, trigger.play_for, output_sample_rate)
                .map(|frames| ActiveLinearGainRamp::new(gain_ramp, frames))
        });
        let mut oscillator = Oscillator::new(trigger.source, output_sample_rate);
        let elapsed_frames = (trigger.envelope.elapsed().as_secs_f64()
            * f64::from(output_sample_rate))
        .round() as u64;
        oscillator.seek(elapsed_frames, initial_frequency_hz);
        Some(Self {
            voice_id: trigger.voice_id,
            oscillator,
            base_pitch: trigger.pitch,
            gain: sanitize_gain(trigger.gain),
            gain_ramp,
            velocity: trigger.velocity.value() as f32,
            spatial: trigger.spatial,
            spatializer: StereoVectorPanner::new(),
            envelope: EnvelopeRuntime::new(trigger.envelope.clone(), output_sample_rate),
            runtime_controls: AudioRuntimeControlState {
                filters: FilterPlan {
                    low_pass_cutoff_hz: trigger.low_pass_cutoff_hz,
                    low_pass_resonance: trigger.low_pass_resonance,
                    high_pass_cutoff_hz: trigger.high_pass_cutoff_hz,
                    high_pass_resonance: trigger.high_pass_resonance,
                },
                ..trigger.runtime_controls
            },
            modulations: voice_modulations(&trigger.modulations),
            rendered_frames: 0,
            sample_rate: output_sample_rate,
            low_pass: trigger.low_pass_cutoff_hz.map(|cutoff| {
                BiquadFilter::low_pass(
                    cutoff as f32,
                    trigger.low_pass_resonance,
                    output_sample_rate,
                )
            }),
            high_pass: trigger.high_pass_cutoff_hz.map(|cutoff| {
                BiquadFilter::high_pass(
                    cutoff as f32,
                    trigger.high_pass_resonance,
                    output_sample_rate,
                )
            }),
            compressor: trigger
                .compressor
                .map(|settings| CompressorRuntime::new(settings, output_sample_rate)),
            post_gain: sanitize_gain(trigger.post_gain),
            inserts: super::inserts::InsertRuntime::new(inserts),
            reverb: trigger.reverb,
            delay: trigger.delay,
            live_note: trigger.live_note,
            finished: false,
        })
    }

    /// Returns `true` when the voice is finished and can be removed.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.finished || self.envelope.finished()
    }

    /// Returns the concrete voice identifier.
    #[must_use]
    pub fn voice_id(&self) -> VoiceInstanceId {
        self.voice_id
    }

    /// Returns the live note associated with this voice, if any.
    #[must_use]
    pub fn live_note(&self) -> Option<NoteNumber> {
        self.live_note
    }

    /// Starts the release stage of the voice envelope.
    pub fn release(&mut self) {
        self.envelope.release();
    }

    /// Applies a runtime control update to this voice.
    pub fn update_runtime_controls(&mut self, delta: AudioRuntimeControlDelta) {
        update_voice_modulations(&mut self.modulations, &delta, self.spatial);
        delta.apply_to(&mut self.runtime_controls);
        apply_voice_dsp_delta(
            &delta,
            self.runtime_controls.filters,
            self.sample_rate,
            VoiceDspState {
                low_pass: &mut self.low_pass,
                high_pass: &mut self.high_pass,
                compressor: &mut self.compressor,
                reverb: &mut self.reverb,
                delay: &mut self.delay,
            },
        );
        apply_voice_level_delta(
            &delta,
            &mut self.gain,
            &mut self.gain_ramp,
            &mut self.post_gain,
            None,
        );
    }

    #[must_use]
    pub(crate) fn releasing(&self) -> bool {
        self.envelope.releasing()
    }

    pub(crate) fn render_next(&mut self) -> VoiceFrame {
        if self.finished() {
            self.finished = true;
            return VoiceFrame::SILENT;
        }

        let cycle = self.spatial.start_cycle
            + self.rendered_frames as f64 / self.sample_rate as f64 * self.spatial.cps;
        let transpose = self.runtime_controls.transpose.eval(cycle);
        let Some(frequency_hz) = midi_note_to_hz(
            self.base_pitch + transpose + self.runtime_controls.pitch_bend_semitones,
        ) else {
            self.finished = true;
            return VoiceFrame::SILENT;
        };
        let modulation =
            evaluate_frame_modulation(&self.modulations, self.rendered_frames, self.sample_rate);

        let mut frame = Frame::from_mono(self.oscillator.next_mono(frequency_hz));
        // Advance phase even while an attack starts at zero; block boundaries
        // and silent envelope samples must not change note phase/noise position.
        let amplitude = self.envelope.current_level();
        frame *= self.current_gain() * self.velocity * amplitude;
        if let Some(gain) = modulation.gain {
            frame *= gain;
        }

        if let Some(filter) = &mut self.high_pass {
            frame = filter.process(frame);
        }

        if let Some(filter) = &mut self.low_pass {
            if let Some(cutoff) = modulation.cutoff {
                filter.set_cutoff(cutoff);
            }
            frame = filter.process(frame);
        }

        if let Some(compressor) = &mut self.compressor {
            frame = compressor.process(frame);
        }

        frame = self.inserts.process(frame);
        frame *= self.post_gain;
        if !self.runtime_controls.gate {
            frame = Frame::ZERO;
        }
        let position = self
            .spatial
            .position_at_frame(self.rendered_frames, self.sample_rate);
        frame = self.spatializer.place(frame, position);
        frame = frame.panned(self.runtime_controls.pan.clamp(-1.0, 1.0) as f32);

        let reverb = self
            .reverb
            .as_ref()
            .map(|settings| (*settings, frame * settings.amount().value() as f32));
        let delay = self
            .delay
            .as_ref()
            .map(|settings| (*settings, frame * settings.amount().value() as f32));

        self.consume_output_frame();

        VoiceFrame {
            dry: frame,
            reverb,
            delay,
        }
    }

    fn current_gain(&self) -> f32 {
        self.gain_ramp
            .as_ref()
            .map_or(self.gain, ActiveLinearGainRamp::current_gain)
            * self.runtime_controls.expression.value() as f32
    }
    fn consume_output_frame(&mut self) {
        if let Some(gain_ramp) = &mut self.gain_ramp {
            gain_ramp.advance();
        }

        self.envelope.advance();
        self.rendered_frames = self.rendered_frames.saturating_add(1);

        if self.envelope.finished() {
            self.finished = true;
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveLinearGainRamp {
    ramp: SampleGainRamp,
    total_output_frames: usize,
    rendered_output_frames: usize,
}

impl ActiveLinearGainRamp {
    fn new(ramp: SampleGainRamp, total_output_frames: usize) -> Self {
        Self {
            ramp,
            total_output_frames: total_output_frames.max(1),
            rendered_output_frames: 0,
        }
    }

    fn current_gain(&self) -> f32 {
        if self.total_output_frames <= 1 {
            return self.ramp.from() as f32;
        }

        let progress = self.rendered_output_frames as f64
            / (self.total_output_frames.saturating_sub(1)) as f64;

        self.ramp.value_at(progress) as f32
    }

    fn advance(&mut self) {
        self.rendered_output_frames = self
            .rendered_output_frames
            .saturating_add(1)
            .min(self.total_output_frames.saturating_sub(1));
    }
}

#[derive(Debug, Clone)]
struct EnvelopeRuntime {
    spec: SampleEnvelope,
    elapsed_frames: usize,
    attack_frames: usize,
    decay_frames: usize,
    release_frames: usize,
    gate_frames: Option<usize>,
    stage: EnvelopeStage,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EnvelopeStage {
    Attack {
        elapsed_frames: usize,
    },
    Decay {
        elapsed_frames: usize,
    },
    Sustain,
    Release {
        elapsed_frames: usize,
        start_level: f32,
    },
    Done,
}

impl EnvelopeRuntime {
    fn new(spec: SampleEnvelope, sample_rate: u32) -> Self {
        let elapsed_frames = frames_for_duration(spec.elapsed(), sample_rate).unwrap_or(0);
        let attack_frames = frames_for_duration(spec.attack(), sample_rate).unwrap_or(0);
        let decay_frames = frames_for_duration(spec.decay(), sample_rate).unwrap_or(0);
        let release_frames = frames_for_duration(spec.release(), sample_rate).unwrap_or(0);
        let gate_frames = spec
            .gate_duration()
            .and_then(|gate_duration| frames_for_duration(gate_duration, sample_rate));

        let mut runtime = Self {
            spec,
            elapsed_frames,
            attack_frames,
            decay_frames,
            release_frames,
            gate_frames,
            stage: EnvelopeStage::Done,
        };
        runtime.stage = runtime.stage_for_elapsed(elapsed_frames);
        runtime
    }

    fn current_level(&self) -> f32 {
        match self.stage {
            EnvelopeStage::Attack { elapsed_frames } => {
                if self.attack_frames == 0 {
                    1.0
                } else {
                    elapsed_frames as f32 / self.attack_frames as f32
                }
            }
            EnvelopeStage::Decay { elapsed_frames } => {
                if self.decay_frames == 0 {
                    self.spec.sustain_level().value() as f32
                } else {
                    let sustain = self.spec.sustain_level().value() as f32;
                    let progress = elapsed_frames as f32 / self.decay_frames as f32;
                    1.0 + (sustain - 1.0) * progress
                }
            }
            EnvelopeStage::Sustain => self.spec.sustain_level().value() as f32,
            EnvelopeStage::Release {
                elapsed_frames,
                start_level,
            } => self.release_level(start_level, elapsed_frames),
            EnvelopeStage::Done => 0.0,
        }
    }

    fn advance(&mut self) {
        if self.finished() {
            return;
        }

        self.elapsed_frames = self.elapsed_frames.saturating_add(1);
        self.stage = match self.stage {
            EnvelopeStage::Release {
                elapsed_frames,
                start_level,
            } => self.release_stage(start_level, elapsed_frames.saturating_add(1)),
            _ => self.stage_for_elapsed(self.elapsed_frames),
        };
    }

    fn release(&mut self) {
        if !self.releasing() && !self.finished() {
            self.stage = self.release_stage(self.current_level(), 0);
        }
    }

    fn releasing(&self) -> bool {
        matches!(self.stage, EnvelopeStage::Release { .. })
    }

    fn finished(&self) -> bool {
        matches!(self.stage, EnvelopeStage::Done)
    }

    fn base_level_at(&self, elapsed_frames: usize) -> f32 {
        let sustain = self.spec.sustain_level().value() as f32;

        if self.attack_frames > 0 && elapsed_frames < self.attack_frames {
            return elapsed_frames as f32 / self.attack_frames as f32;
        }

        let after_attack = elapsed_frames.saturating_sub(self.attack_frames);
        if self.decay_frames > 0 && after_attack < self.decay_frames {
            let progress = after_attack as f32 / self.decay_frames as f32;
            return 1.0 + (sustain - 1.0) * progress;
        }

        sustain
    }

    fn release_level(&self, start_level: f32, elapsed_frames: usize) -> f32 {
        if self.release_frames == 0 {
            return 0.0;
        }

        let progress = elapsed_frames.min(self.release_frames) as f32 / self.release_frames as f32;
        start_level * (1.0 - progress)
    }

    fn stage_for_elapsed(&self, elapsed_frames: usize) -> EnvelopeStage {
        if let Some(gate_frames) = self.gate_frames
            && elapsed_frames >= gate_frames
        {
            return self.release_stage(
                self.base_level_at(gate_frames),
                elapsed_frames.saturating_sub(gate_frames),
            );
        }

        if self.attack_frames > 0 && elapsed_frames < self.attack_frames {
            return EnvelopeStage::Attack { elapsed_frames };
        }

        let after_attack = elapsed_frames.saturating_sub(self.attack_frames);
        if self.decay_frames > 0 && after_attack < self.decay_frames {
            return EnvelopeStage::Decay {
                elapsed_frames: after_attack,
            };
        }

        EnvelopeStage::Sustain
    }

    fn release_stage(&self, start_level: f32, elapsed_frames: usize) -> EnvelopeStage {
        if self.release_frames == 0 || elapsed_frames >= self.release_frames {
            EnvelopeStage::Done
        } else {
            EnvelopeStage::Release {
                elapsed_frames,
                start_level,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct BiquadFilter {
    coefficients: BiquadCoefficients,
    mode: FilterMode,
    resonance: UnitValue,
    sample_rate: u32,
    left_1: f32,
    left_2: f32,
    right_1: f32,
    right_2: f32,
}

impl BiquadFilter {
    pub(super) fn from_coefficients(coefficients: BiquadCoefficients) -> Self {
        Self {
            coefficients,
            mode: FilterMode::LowPass,
            resonance: UnitValue::new(0.0).unwrap(),
            sample_rate: 1,
            left_1: 0.0,
            left_2: 0.0,
            right_1: 0.0,
            right_2: 0.0,
        }
    }
    fn low_pass(cutoff_hz: f32, resonance: UnitValue, sample_rate: u32) -> Self {
        Self::new(FilterMode::LowPass, cutoff_hz, resonance, sample_rate)
    }

    fn high_pass(cutoff_hz: f32, resonance: UnitValue, sample_rate: u32) -> Self {
        Self::new(FilterMode::HighPass, cutoff_hz, resonance, sample_rate)
    }

    fn new(mode: FilterMode, cutoff_hz: f32, resonance: UnitValue, sample_rate: u32) -> Self {
        Self {
            coefficients: BiquadCoefficients::new(mode, cutoff_hz, resonance, sample_rate),
            mode,
            resonance,
            sample_rate,
            left_1: 0.0,
            left_2: 0.0,
            right_1: 0.0,
            right_2: 0.0,
        }
    }

    /// Recomputes coefficients for a new cutoff while preserving filter state.
    ///
    /// This is used for per-frame cutoff modulation on the audio thread; it
    /// allocates nothing and keeps the delay-line memory intact.
    fn set_cutoff(&mut self, cutoff_hz: f32) {
        self.coefficients =
            BiquadCoefficients::new(self.mode, cutoff_hz, self.resonance, self.sample_rate);
    }

    pub(super) fn process(&mut self, input: Frame) -> Frame {
        Frame::new(
            self.process_left(input.left),
            self.process_right(input.right),
        )
    }

    fn process_left(&mut self, input: f32) -> f32 {
        let output = self.coefficients.b0 * input + self.left_1;
        self.left_1 = self.coefficients.b1 * input - self.coefficients.a1 * output + self.left_2;
        self.left_2 = self.coefficients.b2 * input - self.coefficients.a2 * output;
        output
    }

    fn process_right(&mut self, input: f32) -> f32 {
        let output = self.coefficients.b0 * input + self.right_1;
        self.right_1 = self.coefficients.b1 * input - self.coefficients.a1 * output + self.right_2;
        self.right_2 = self.coefficients.b2 * input - self.coefficients.a2 * output;
        output
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FilterMode {
    LowPass,
    HighPass,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BiquadCoefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoefficients {
    pub(super) fn new(
        mode: FilterMode,
        cutoff_hz: f32,
        resonance: UnitValue,
        sample_rate: u32,
    ) -> Self {
        let nyquist_guard = (sample_rate as f32 * 0.45).max(20.0);
        let cutoff_hz = cutoff_hz.clamp(20.0, nyquist_guard);
        let omega = 2.0 * PI32 * cutoff_hz / sample_rate.max(1) as f32;
        let sin = omega.sin();
        let cos = omega.cos();
        let q = 0.5 + resonance.value() as f32 * 11.5;
        let alpha = sin / (2.0 * q.max(f32::MIN_POSITIVE));

        let (b0, b1, b2, a0, a1, a2) = match mode {
            FilterMode::LowPass => (
                (1.0 - cos) * 0.5,
                1.0 - cos,
                (1.0 - cos) * 0.5,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            FilterMode::HighPass => (
                (1.0 + cos) * 0.5,
                -(1.0 + cos),
                (1.0 + cos) * 0.5,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
        };

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

/// Updates fixed-size DSP state in place on the audio thread. Filter memories
/// and the compressor detector survive parameter changes; no buffers allocate.
struct VoiceDspState<'a> {
    low_pass: &'a mut Option<BiquadFilter>,
    high_pass: &'a mut Option<BiquadFilter>,
    compressor: &'a mut Option<CompressorRuntime>,
    reverb: &'a mut Option<ReverbSettings>,
    delay: &'a mut Option<DelaySettings>,
}

fn apply_voice_dsp_delta(
    delta: &AudioRuntimeControlDelta,
    filters: FilterPlan,
    sample_rate: u32,
    state: VoiceDspState<'_>,
) {
    let VoiceDspState {
        low_pass,
        high_pass,
        compressor,
        reverb,
        delay,
    } = state;
    for (cutoff_delta, resonance_delta, cutoff, resonance, mode, filter) in [
        (
            delta.low_pass_cutoff_hz,
            delta.low_pass_resonance,
            filters.low_pass_cutoff_hz,
            filters.low_pass_resonance,
            FilterMode::LowPass,
            low_pass,
        ),
        (
            delta.high_pass_cutoff_hz,
            delta.high_pass_resonance,
            filters.high_pass_cutoff_hz,
            filters.high_pass_resonance,
            FilterMode::HighPass,
            high_pass,
        ),
    ] {
        if matches!(cutoff_delta, RuntimeControlValue::Keep)
            && matches!(resonance_delta, RuntimeControlValue::Keep)
        {
            continue;
        }
        match (cutoff, filter.as_mut()) {
            (Some(cutoff), Some(active)) => {
                active.resonance = resonance;
                active.set_cutoff(cutoff as f32);
            }
            (Some(cutoff), None) => {
                *filter = Some(BiquadFilter::new(
                    mode,
                    cutoff as f32,
                    resonance,
                    sample_rate,
                ))
            }
            (None, _) => *filter = None,
        }
    }
    match delta.compressor {
        RuntimeControlValue::Keep => {}
        RuntimeControlValue::Set(settings) => match compressor {
            Some(active) => active.settings = settings,
            None => *compressor = Some(CompressorRuntime::new(settings, sample_rate)),
        },
        RuntimeControlValue::Reset => *compressor = None,
    }
    match delta.reverb {
        RuntimeControlValue::Keep => {}
        RuntimeControlValue::Set(settings) => *reverb = Some(settings),
        RuntimeControlValue::Reset => *reverb = None,
    }
    match delta.delay {
        RuntimeControlValue::Keep => {}
        RuntimeControlValue::Set(settings) => *delay = Some(settings),
        RuntimeControlValue::Reset => *delay = None,
    }
}

#[derive(Debug, Clone)]
struct CompressorRuntime {
    settings: CompressorSettings,
    sample_rate: u32,
    detector: f32,
}

impl CompressorRuntime {
    fn new(settings: CompressorSettings, sample_rate: u32) -> Self {
        Self {
            settings,
            sample_rate,
            detector: 0.0,
        }
    }

    fn process(&mut self, input: Frame) -> Frame {
        let amplitude = input.left.abs().max(input.right.abs());
        let attack_coeff = smoothing_coefficient(self.settings.attack(), self.sample_rate);
        let release_coeff = smoothing_coefficient(self.settings.release(), self.sample_rate);
        let target = amplitude;

        if target > self.detector {
            self.detector += (target - self.detector) * attack_coeff;
        } else {
            self.detector += (target - self.detector) * release_coeff;
        }

        let gain = compressor_gain(self.detector, &self.settings);

        input * gain
    }
}

/// Smooth downward compression in decibels. The knee spans the threshold;
/// its quadratic joins both straight sections with matching slopes.
fn compressor_gain(level: f32, settings: &CompressorSettings) -> f32 {
    let threshold = settings.threshold().value() as f32;
    if level <= f32::EPSILON || threshold <= f32::EPSILON {
        return 1.0;
    }
    let over_db = 20.0 * (level / threshold).log10();
    let knee = settings.knee_db() as f32;
    let slope = (1.0 / settings.ratio() as f32 - 1.0).min(0.0);
    let reduction_db = if knee > 0.0 && over_db > -knee * 0.5 && over_db < knee * 0.5 {
        slope * (over_db + knee * 0.5).powi(2) / (2.0 * knee)
    } else if over_db >= knee * 0.5 {
        slope * over_db
    } else {
        0.0
    };
    10.0_f32.powf(reduction_db / 20.0).clamp(0.0, 1.0)
}

fn smoothing_coefficient(duration: Duration, sample_rate: u32) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }

    let frames = (duration.as_secs_f32() * sample_rate as f32).max(1.0);
    1.0 / frames
}

fn midi_note_to_hz(pitch: f64) -> Option<f64> {
    if !pitch.is_finite() {
        return None;
    }

    Some(440.0 * 2.0_f64.powf((pitch - 69.0) / 12.0))
}

fn gain_ramp_total_frames(
    envelope: &SampleEnvelope,
    play_for: Duration,
    output_sample_rate: u32,
) -> Option<usize> {
    envelope
        .gate_duration()
        .or((!play_for.is_zero()).then_some(play_for))
        .and_then(|duration| frames_for_duration(duration, output_sample_rate))
}

fn normalized_region(sample: &SampleBuffer, begin: f64, end: f64) -> Option<Region> {
    let begin = sanitize_fraction(begin, 0.0);
    let end = sanitize_fraction(end, 1.0);

    if end <= begin {
        warn_invalid_region_once();
        return None;
    }

    let sample_len = sample.len();
    let start_index = ((sample_len as f64) * begin).floor() as usize;
    let end_index = ((sample_len as f64) * end).ceil() as usize;
    let start_index = start_index.min(sample_len);
    let end_index = end_index.min(sample_len);

    if end_index <= start_index {
        warn_invalid_region_once();
        return None;
    }

    Some((start_index as u64..end_index as u64).into())
}

fn natural_output_frame_count(region: &Region, sample_rate: u32, playback_step: f64) -> usize {
    let start = region.start.into_samples(sample_rate);
    let end = match region.end {
        crate::adapter::audio::EndPosition::EndOfAudio => start,
        crate::adapter::audio::EndPosition::Custom(position) => position.into_samples(sample_rate),
    };
    let playback_frames = end.saturating_sub(start).max(1);

    ((playback_frames as f64) / playback_step.max(f64::MIN_POSITIVE))
        .ceil()
        .max(1.0) as usize
}

fn frames_for_duration(duration: Duration, output_sample_rate: u32) -> Option<usize> {
    if duration.is_zero() || output_sample_rate == 0 {
        return None;
    }

    let frames = (duration.as_secs_f64() * output_sample_rate as f64).ceil() as usize;
    Some(frames.max(1))
}

fn apply_voice_level_delta(
    delta: &AudioRuntimeControlDelta,
    gain: &mut f32,
    gain_ramp: &mut Option<ActiveLinearGainRamp>,
    post_gain: &mut f32,
    playback_rate: Option<&mut f64>,
) {
    match delta.gain {
        RuntimeControlValue::Keep => {}
        RuntimeControlValue::Set(value) => {
            *gain = value;
            *gain_ramp = None;
        }
        RuntimeControlValue::Reset => {
            *gain = 1.0;
            *gain_ramp = None;
        }
    }
    match delta.post_gain {
        RuntimeControlValue::Keep => {}
        RuntimeControlValue::Set(value) => *post_gain = value,
        RuntimeControlValue::Reset => *post_gain = 1.0,
    }
    if let Some(playback_rate) = playback_rate {
        match delta.playback_rate {
            RuntimeControlValue::Keep => {}
            RuntimeControlValue::Set(value) => *playback_rate = value,
            RuntimeControlValue::Reset => *playback_rate = 1.0,
        }
    }
}

fn sanitize_gain(gain: f64) -> f32 {
    if gain.is_finite() && gain >= 0.0 {
        gain as f32
    } else {
        1.0
    }
}

fn sanitize_fraction(value: f64, default: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        default
    }
}

fn warn_invalid_region_once() {
    if INVALID_REGION_WARNING_EMITTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        eprintln!("dropping sample trigger because begin/end produced an empty region");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{sample::SampleTriggerBuilder, synth::SynthTrigger},
        domain::{
            control::{CompressorSettings, UnitValue},
            intent::BuiltInSynthSource,
        },
    };

    fn mono_sample(values: &[f32], sample_rate: u32) -> Arc<SampleBuffer> {
        Arc::new(SampleBuffer::new(
            sample_rate,
            values
                .iter()
                .copied()
                .map(Frame::from_mono)
                .collect::<Vec<_>>(),
        ))
    }

    fn envelope(
        attack: Duration,
        decay: Duration,
        sustain: f64,
        release: Duration,
        gate: Option<Duration>,
        elapsed: Duration,
    ) -> SampleEnvelope {
        SampleEnvelope::new(
            attack,
            decay,
            UnitValue::new(sustain).unwrap(),
            release,
            gate,
            elapsed,
        )
    }

    fn synth_voice(source: BuiltInSynthSource, pitch: f64) -> SynthVoice {
        SynthVoice::new(
            SynthTrigger::builder()
                .source(source)
                .pitch(pitch)
                .envelope(envelope(
                    Duration::ZERO,
                    Duration::ZERO,
                    1.0,
                    Duration::from_millis(10),
                    None,
                    Duration::ZERO,
                ))
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            8_000,
        )
        .unwrap()
    }

    #[test]
    fn attack_decay_sustain_release_shape_voice_level() {
        let mut voice = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("pad")
                    .envelope(envelope(
                        Duration::from_millis(2),
                        Duration::from_millis(2),
                        0.5,
                        Duration::from_millis(2),
                        Some(Duration::from_millis(6)),
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "pad".to_string(),
                sample: mono_sample(&[1.0; 128], 8_000),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        let first = voice.render_next().dry.left;
        for _ in 0..16 {
            let _ = voice.render_next();
        }
        let later = voice.render_next().dry.left;

        assert!(later >= first);
    }

    #[test]
    fn gain_signal_modulates_frame_amplitude_per_frame() {
        use crate::domain::rational::Rational;
        use crate::domain::signal::Signal;

        // A fast gain LFO over a constant unit sample: the rendered amplitude
        // must track the signal rather than stay flat.
        let lfo = Signal::sine()
            .with_rate(Rational::whole_number(100))
            .with_bias(0.5)
            .with_depth(0.5);
        let binding = SignalBinding {
            key: ControlKey::Gain,
            signal: lfo,
            start_cycle: 0.0,
            cps: 1.0,
        };

        let mut voice = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("pad")
                    .envelope(envelope(
                        Duration::ZERO,
                        Duration::ZERO,
                        1.0,
                        Duration::from_millis(50),
                        None,
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .modulations(vec![binding])
                    .build()
                    .unwrap(),
                sample_key: "pad".to_string(),
                sample: mono_sample(&[1.0; 512], 8_000),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        let mut samples = Vec::new();
        for _ in 0..64 {
            samples.push(voice.render_next().dry.left.abs());
        }

        let max = samples.iter().copied().fold(f32::MIN, f32::max);
        let min = samples.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            (max - min) > 0.05,
            "gain LFO should vary frame amplitude (min={min}, max={max})"
        );
    }

    #[test]
    fn nonzero_attack_does_not_finish_synth_before_voice_becomes_audible() {
        let mut voice = SynthVoice::new(
            SynthTrigger::builder()
                .source(BuiltInSynthSource::Saw)
                .pitch(69.0)
                .envelope(envelope(
                    Duration::from_millis(4),
                    Duration::ZERO,
                    1.0,
                    Duration::from_millis(10),
                    None,
                    Duration::ZERO,
                ))
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            8_000,
        )
        .unwrap();

        assert!(!voice.finished());
        assert_eq!(voice.render_next(), VoiceFrame::SILENT);
        assert!(!voice.finished());

        let later_frames: Vec<_> = (0..16).map(|_| voice.render_next().dry.left).collect();
        assert!(
            later_frames
                .iter()
                .any(|sample| sample.abs() > f32::EPSILON)
        );
    }

    #[test]
    fn in_progress_attack_enters_with_intermediate_level_instead_of_finishing() {
        let mut voice = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("pad")
                    .envelope(envelope(
                        Duration::from_millis(4),
                        Duration::ZERO,
                        1.0,
                        Duration::from_millis(10),
                        Some(Duration::from_millis(20)),
                        Duration::from_millis(2),
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "pad".to_string(),
                sample: mono_sample(&[1.0; 128], 8_000),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        assert!(!voice.finished());
        let first = voice.render_next().dry.left;
        assert!(first > 0.0);
    }

    #[test]
    fn gate_duration_drives_release_until_voice_is_done() {
        let mut voice = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("pad")
                    .envelope(envelope(
                        Duration::ZERO,
                        Duration::ZERO,
                        1.0,
                        Duration::from_millis(2),
                        Some(Duration::from_millis(2)),
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "pad".to_string(),
                sample: mono_sample(&[1.0; 256], 8_000),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        let samples: Vec<_> = (0..40)
            .map(|_| voice.render_next().dry.left.abs())
            .collect();

        assert!(samples.iter().take(16).any(|sample| *sample > 0.0));
        assert!(voice.finished());
    }

    #[test]
    fn release_moves_live_voice_toward_silence() {
        let note = NoteNumber::new(60).unwrap();
        let mut voice = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("pad")
                    .live_note(note)
                    .envelope(envelope(
                        Duration::ZERO,
                        Duration::ZERO,
                        1.0,
                        Duration::from_millis(10),
                        None,
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "pad".to_string(),
                sample: mono_sample(&[1.0; 128], 8_000),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        voice.release();
        let mut last = 1.0_f32;
        for _ in 0..64 {
            let current = voice.render_next().dry.left;
            assert!(current <= last + 0.001);
            last = current;
        }
    }

    #[test]
    fn high_pass_reduces_dc_heavily() {
        let mut voice = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("dc")
                    .high_pass_cutoff_hz(800.0)
                    .envelope(envelope(
                        Duration::ZERO,
                        Duration::ZERO,
                        1.0,
                        Duration::ZERO,
                        None,
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "dc".to_string(),
                sample: mono_sample(&[1.0; 64], 8_000),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        let mut total = 0.0_f32;
        for _ in 0..32 {
            total += voice.render_next().dry.left.abs();
        }

        assert!(total < 10.0);
    }

    #[test]
    fn compressor_ratio_and_soft_knee_follow_decibel_levels() {
        let hard = CompressorSettings::new(
            UnitValue::new(0.1).unwrap(),
            4.0,
            Duration::from_millis(1),
            Duration::from_millis(10),
        );
        let soft = hard.with_knee_db(10.0);
        // 20 dB over a 4:1 threshold becomes 5 dB over: 15 dB reduction.
        assert!((compressor_gain(1.0, &hard) - 10.0_f32.powf(-15.0 / 20.0)).abs() < 1e-6);
        assert_eq!(compressor_gain(0.08, &hard), 1.0);
        assert!(compressor_gain(0.08, &soft) < 1.0);
        assert_eq!(compressor_gain(0.01, &soft), 1.0);
        for boundary in [-5.0_f32, 5.0] {
            let below = 0.1 * 10.0_f32.powf((boundary - 0.0001) / 20.0);
            let above = 0.1 * 10.0_f32.powf((boundary + 0.0001) / 20.0);
            assert!((compressor_gain(below, &soft) - compressor_gain(above, &soft)).abs() < 1e-4);
        }
        // Opposite-polarity stereo must still activate the linked detector.
        let mut runtime = CompressorRuntime::new(soft, 1000);
        let frame = runtime.process(Frame {
            left: 1.0,
            right: -1.0,
        });
        assert!(frame.left < 0.3 && frame.right > -0.3);
        assert_eq!(frame.left, -frame.right);
    }

    #[test]
    fn compressor_reduces_hot_signal() {
        let compressor = CompressorSettings::new(
            UnitValue::new(0.3).unwrap(),
            6.0,
            Duration::from_millis(2),
            Duration::from_millis(20),
        );
        let mut voice = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("hot")
                    .compressor(compressor)
                    .envelope(envelope(
                        Duration::ZERO,
                        Duration::ZERO,
                        1.0,
                        Duration::ZERO,
                        None,
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "hot".to_string(),
                sample: mono_sample(&[1.0; 64], 8_000),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        let mut reduced = false;
        for _ in 0..16 {
            let frame = voice.render_next().dry;
            if frame.left < 1.0 {
                reduced = true;
                break;
            }
        }

        assert!(reduced);
    }

    #[test]
    fn synth_waveforms_have_distinct_smoke_test_shapes() {
        let mut sine = synth_voice(BuiltInSynthSource::Sine, 69.0);
        let mut square = synth_voice(BuiltInSynthSource::Square, 69.0);
        let mut saw = synth_voice(BuiltInSynthSource::Saw, 69.0);
        let mut triangle = synth_voice(BuiltInSynthSource::Triangle, 69.0);

        let sine_shape: Vec<_> = (0..8).map(|_| sine.render_next().dry.left).collect();
        let square_shape: Vec<_> = (0..8).map(|_| square.render_next().dry.left).collect();
        let saw_shape: Vec<_> = (0..8).map(|_| saw.render_next().dry.left).collect();
        let triangle_shape: Vec<_> = (0..8).map(|_| triangle.render_next().dry.left).collect();

        assert_ne!(sine_shape, square_shape);
        assert_ne!(square_shape, saw_shape);
        assert_ne!(saw_shape, triangle_shape);
    }

    #[test]
    fn synth_release_moves_voice_toward_silence() {
        let note = NoteNumber::new(60).unwrap();
        let mut voice = SynthVoice::new(
            SynthTrigger::builder()
                .source(BuiltInSynthSource::Sine)
                .pitch(60.0)
                .live_note(note)
                .envelope(envelope(
                    Duration::ZERO,
                    Duration::ZERO,
                    1.0,
                    Duration::from_millis(10),
                    None,
                    Duration::ZERO,
                ))
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            8_000,
        )
        .unwrap();

        voice.release();
        let samples: Vec<_> = (0..128)
            .map(|_| voice.render_next().dry.left.abs())
            .collect();
        let early_energy: f32 = samples.iter().take(32).sum();
        let late_energy: f32 = samples.iter().skip(96).sum();

        assert!(late_energy < early_energy);
        assert!(voice.finished());
    }

    #[test]
    fn synth_resonance_changes_low_pass_response() {
        let mut plain = SynthVoice::new(
            SynthTrigger::builder()
                .source(BuiltInSynthSource::Saw)
                .pitch(69.0)
                .low_pass_cutoff_hz(800.0)
                .low_pass_resonance(UnitValue::new(0.0).unwrap())
                .envelope(envelope(
                    Duration::ZERO,
                    Duration::ZERO,
                    1.0,
                    Duration::ZERO,
                    None,
                    Duration::ZERO,
                ))
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            8_000,
        )
        .unwrap();
        let mut resonant = SynthVoice::new(
            SynthTrigger::builder()
                .source(BuiltInSynthSource::Saw)
                .pitch(69.0)
                .low_pass_cutoff_hz(800.0)
                .low_pass_resonance(UnitValue::new(0.9).unwrap())
                .envelope(envelope(
                    Duration::ZERO,
                    Duration::ZERO,
                    1.0,
                    Duration::ZERO,
                    None,
                    Duration::ZERO,
                ))
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            8_000,
        )
        .unwrap();

        let plain_shape: Vec<_> = (0..32).map(|_| plain.render_next().dry.left).collect();
        let resonant_shape: Vec<_> = (0..32).map(|_| resonant.render_next().dry.left).collect();

        assert_ne!(plain_shape, resonant_shape);
    }

    #[test]
    fn sample_resonance_changes_low_pass_response() {
        let mut values = Vec::new();
        for _ in 0..16 {
            values.extend([1.0, 0.0, -1.0, 0.5, -0.5, 0.25, -0.25, 0.0]);
        }
        let sample = mono_sample(&values, 8_000);
        let mut plain = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("resonant")
                    .low_pass_cutoff_hz(1_200.0)
                    .low_pass_resonance(UnitValue::new(0.0).unwrap())
                    .envelope(envelope(
                        Duration::ZERO,
                        Duration::ZERO,
                        1.0,
                        Duration::ZERO,
                        None,
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "resonant".to_string(),
                sample: Arc::clone(&sample),
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();
        let mut resonant = SampleVoice::new(
            LoadedSampleTrigger {
                sustain_loop: None,
                trigger: SampleTriggerBuilder::new()
                    .sample("resonant")
                    .low_pass_cutoff_hz(1_200.0)
                    .low_pass_resonance(UnitValue::new(0.9).unwrap())
                    .envelope(envelope(
                        Duration::ZERO,
                        Duration::ZERO,
                        1.0,
                        Duration::ZERO,
                        None,
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
                sample_key: "resonant".to_string(),
                sample,
                playback_limit: None,
                choke_group: None,
            },
            8_000,
        )
        .unwrap();

        let plain_shape: Vec<_> = (0..32).map(|_| plain.render_next().dry.left).collect();
        let resonant_shape: Vec<_> = (0..32).map(|_| resonant.render_next().dry.left).collect();

        assert_ne!(plain_shape, resonant_shape);
    }
}
