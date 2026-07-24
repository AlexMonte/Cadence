//! Audio voice mixer and simple send-effect buses.

use std::time::Duration;

use crate::adapter::{
    audio::{Frame, LoadedSampleTrigger, SampleVoice, SynthVoice},
    sample_bank::ChokeGroup,
};
use crate::application::{
    audio::{AudioRuntimeControlDelta, VoiceInstanceId},
    synth::SynthTrigger,
};
use crate::domain::control::{DelaySettings, ReverbSettings};

const CHOKE_FADE_OUT: Duration = Duration::from_millis(8);
const SYNTH_POLYPHONY_LIMIT: usize = 8;

#[derive(Debug)]
enum ActiveVoice {
    Sample {
        choke_group: Option<ChokeGroup>,
        voice: SampleVoice,
    },
    Synth(SynthVoice),
}

#[derive(Debug)]
/// Mixes active sample and synth voices into output frames.
pub struct AudioMixer {
    output_sample_rate: u32,
    voices: Vec<ActiveVoice>,
    reverb_buses: Vec<(ReverbSettings, ReverbBus)>,
    delay_buses: Vec<(DelaySettings, DelayBus)>,
}

impl AudioMixer {
    /// Creates an empty mixer for the given output sample rate.
    #[must_use]
    pub fn new(output_sample_rate: u32) -> Self {
        Self {
            output_sample_rate,
            voices: Vec::new(),
            reverb_buses: Vec::new(),
            delay_buses: Vec::new(),
        }
    }

    /// Adds a resolved sample voice to the mix.
    pub fn push(&mut self, loaded: LoadedSampleTrigger) {
        let choke_group = loaded.choke_group;

        if let Some(group) = choke_group {
            for active in self
                .voices
                .iter_mut()
                .filter(|active| active.choke_group() == Some(group))
            {
                active.start_fade_out(CHOKE_FADE_OUT, self.output_sample_rate);
            }
        }

        // Choke groups let one incoming sample fade out earlier members of the
        // same family, such as closed hats muting open hats.
        if let Some(voice) = SampleVoice::new(loaded, self.output_sample_rate) {
            self.voices.push(ActiveVoice::Sample { choke_group, voice });
        }
    }

    /// Adds a synth voice to the mix.
    pub fn push_synth(&mut self, trigger: SynthTrigger) {
        if self
            .voices
            .iter()
            .filter(|voice| matches!(voice, ActiveVoice::Synth(_)))
            .count()
            >= SYNTH_POLYPHONY_LIMIT
        {
            if let Some(index) = self.stealable_synth_voice_index() {
                self.voices.remove(index);
            }
        }

        if let Some(voice) = SynthVoice::new(trigger, self.output_sample_rate) {
            self.voices.push(ActiveVoice::Synth(voice));
        }
    }

    /// Applies a runtime control update to one active voice instance.
    pub fn update_voice_controls(
        &mut self,
        voice_id: VoiceInstanceId,
        delta: AudioRuntimeControlDelta,
    ) {
        for active in self
            .voices
            .iter_mut()
            .filter(|active| active.voice_id() == voice_id)
        {
            active.update_runtime_controls(delta);
        }
    }

    /// Releases one voice instance explicitly.
    pub fn release_voice(&mut self, voice_id: VoiceInstanceId) {
        for active in self
            .voices
            .iter_mut()
            .filter(|active| active.voice_id() == voice_id)
        {
            active.release();
        }
    }

    /// Renders one block of output frames.
    pub fn render(&mut self, out: &mut [Frame]) {
        out.fill(Frame::ZERO);

        for mixed_frame in out.iter_mut() {
            let mut frame = Frame::ZERO;

            for active in &mut self.voices {
                let rendered = active.render_next();
                frame += rendered.dry;

                if let Some((settings, send)) = rendered.reverb {
                    let bus = reverb_bus(&mut self.reverb_buses, settings, self.output_sample_rate);
                    bus.push(send);
                }

                if let Some((settings, send)) = rendered.delay {
                    let bus = delay_bus(&mut self.delay_buses, settings, self.output_sample_rate);
                    bus.push(send);
                }
            }

            for (_, bus) in &mut self.reverb_buses {
                frame += bus.render_next();
            }
            for (_, bus) in &mut self.delay_buses {
                frame += bus.render_next();
            }

            *mixed_frame = frame;
        }

        self.voices.retain(|active| !active.finished());
    }

    /// Returns the number of active voices currently being mixed.
    #[must_use]
    pub fn active_voice_count(&self) -> usize {
        self.voices.len()
    }

    fn stealable_synth_voice_index(&self) -> Option<usize> {
        self.voices
            .iter()
            .enumerate()
            .find(|(_, voice)| matches!(voice, ActiveVoice::Synth(synth) if synth.releasing()))
            .map(|(index, _)| index)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .find(|(_, voice)| matches!(voice, ActiveVoice::Synth(_)))
                    .map(|(index, _)| index)
            })
    }
}

impl ActiveVoice {
    fn voice_id(&self) -> VoiceInstanceId {
        match self {
            Self::Sample { voice, .. } => voice.voice_id(),
            Self::Synth(voice) => voice.voice_id(),
        }
    }

    fn choke_group(&self) -> Option<ChokeGroup> {
        match self {
            Self::Sample { choke_group, .. } => *choke_group,
            Self::Synth(_) => None,
        }
    }

    fn release(&mut self) {
        match self {
            Self::Sample { voice, .. } => voice.release(),
            Self::Synth(voice) => voice.release(),
        }
    }

    fn update_runtime_controls(&mut self, delta: AudioRuntimeControlDelta) {
        match self {
            Self::Sample { voice, .. } => voice.update_runtime_controls(delta),
            Self::Synth(voice) => voice.update_runtime_controls(delta),
        }
    }

    fn start_fade_out(&mut self, fade_out: Duration, output_sample_rate: u32) {
        if let Self::Sample { voice, .. } = self {
            voice.start_fade_out(fade_out, output_sample_rate);
        }
    }

    fn render_next(&mut self) -> crate::adapter::audio::voice::VoiceFrame {
        match self {
            Self::Sample { voice, .. } => voice.render_next(),
            Self::Synth(voice) => voice.render_next(),
        }
    }

    fn finished(&self) -> bool {
        match self {
            Self::Sample { voice, .. } => voice.finished(),
            Self::Synth(voice) => voice.finished(),
        }
    }
}

/// Backwards-compatible alias for [`AudioMixer`].
pub type SampleMixer = AudioMixer;

fn reverb_bus(
    buses: &mut Vec<(ReverbSettings, ReverbBus)>,
    settings: ReverbSettings,
    sample_rate: u32,
) -> &mut ReverbBus {
    if let Some(index) = buses.iter().position(|(existing, _)| *existing == settings) {
        return &mut buses[index].1;
    }

    buses.push((settings.clone(), ReverbBus::new(settings, sample_rate)));
    &mut buses.last_mut().unwrap().1
}

fn delay_bus(
    buses: &mut Vec<(DelaySettings, DelayBus)>,
    settings: DelaySettings,
    sample_rate: u32,
) -> &mut DelayBus {
    if let Some(index) = buses.iter().position(|(existing, _)| *existing == settings) {
        return &mut buses[index].1;
    }

    buses.push((settings.clone(), DelayBus::new(settings, sample_rate)));
    &mut buses.last_mut().unwrap().1
}

#[derive(Debug)]
struct DelayBus {
    settings: DelaySettings,
    buffer: Vec<Frame>,
    write_index: usize,
    low_pass: f32,
    pending: Frame,
}

impl DelayBus {
    fn new(settings: DelaySettings, sample_rate: u32) -> Self {
        let frames = (settings.time().as_secs_f64() * sample_rate as f64)
            .round()
            .max(1.0) as usize;

        Self {
            settings,
            buffer: vec![Frame::ZERO; frames.max(1)],
            write_index: 0,
            low_pass: 0.0,
            pending: Frame::ZERO,
        }
    }

    fn push(&mut self, input: Frame) {
        self.pending += input;
    }

    fn render_next(&mut self) -> Frame {
        let delayed = self.buffer[self.write_index];
        let damping = self.settings.damping().value() as f32;
        self.low_pass += (delayed.as_mono().left - self.low_pass) * (1.0 - damping);
        let filtered = Frame::from_mono(self.low_pass) + (delayed * damping);
        let feedback = filtered * self.settings.feedback().value() as f32;
        self.buffer[self.write_index] = self.pending + feedback;
        self.pending = Frame::ZERO;
        self.write_index = (self.write_index + 1) % self.buffer.len();
        filtered
    }
}

#[derive(Debug)]
struct ReverbBus {
    delays: [DelayBus; 2],
    pending: Frame,
}

impl ReverbBus {
    fn new(settings: ReverbSettings, sample_rate: u32) -> Self {
        let size = (settings.decay().as_secs_f64() / 4.0).clamp(0.05_f64, 1.0_f64);
        let damping = settings.damping().value();
        let left = DelaySettings::new(
            settings.amount(),
            Duration::from_secs_f64(0.020 + 0.040 * size),
            crate::domain::control::UnitValue::new(
                (0.35_f64 + 0.5_f64 * size).clamp(0.0_f64, 1.0_f64),
            )
            .unwrap(),
            crate::domain::control::UnitValue::new(damping).unwrap(),
        );
        let right = DelaySettings::new(
            settings.amount(),
            Duration::from_secs_f64(0.027 + 0.055 * size),
            crate::domain::control::UnitValue::new(
                (0.40_f64 + 0.45_f64 * size).clamp(0.0_f64, 1.0_f64),
            )
            .unwrap(),
            crate::domain::control::UnitValue::new(damping).unwrap(),
        );

        Self {
            delays: [
                DelayBus::new(left, sample_rate),
                DelayBus::new(right, sample_rate),
            ],
            pending: Frame::ZERO,
        }
    }

    fn push(&mut self, input: Frame) {
        self.pending += input;
    }

    fn render_next(&mut self) -> Frame {
        let mono = self.pending.as_mono();
        self.pending = Frame::ZERO;
        self.delays[0].push(Frame::new(mono.left, 0.0));
        self.delays[1].push(Frame::new(0.0, mono.right));
        let left = self.delays[0].render_next();
        let right = self.delays[1].render_next();
        Frame::new(left.left, right.right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::{audio::SampleBuffer, sample_bank::ChokeGroup},
        application::{
            sample::{SampleEnvelope, SampleTrigger},
            synth::SynthTrigger,
        },
        domain::{
            control::{DelaySettings, ReverbSettings, UnitValue},
            input::NoteNumber,
            intent::BuiltInSynthSource,
        },
    };
    use std::sync::Arc;

    fn envelope() -> SampleEnvelope {
        SampleEnvelope::new(
            Duration::ZERO,
            Duration::ZERO,
            UnitValue::new(1.0).unwrap(),
            Duration::ZERO,
            None,
            Duration::ZERO,
        )
    }

    fn loaded_trigger(
        name: &str,
        frames: &[f32],
        sample_rate: u32,
        choke_group: Option<ChokeGroup>,
    ) -> LoadedSampleTrigger {
        LoadedSampleTrigger {
            trigger: SampleTrigger::builder()
                .sample(name)
                .envelope(envelope())
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            sample_key: name.to_string(),
            sample: Arc::new(SampleBuffer::new(
                sample_rate,
                frames
                    .iter()
                    .copied()
                    .map(Frame::from_mono)
                    .collect::<Vec<_>>(),
            )),
            playback_limit: None,
            choke_group,
        }
    }

    fn synth_trigger(source: BuiltInSynthSource, pitch: f64) -> SynthTrigger {
        SynthTrigger::builder()
            .source(source)
            .pitch(pitch)
            .envelope(envelope())
            .play_for(Duration::from_millis(20))
            .build()
            .unwrap()
    }

    #[test]
    fn mixes_two_voices_into_same_output_buffer() {
        let mut mixer = SampleMixer::new(1);
        mixer.push(loaded_trigger("kick", &[0.25], 1, None));
        mixer.push(loaded_trigger("snare", &[0.5], 1, None));
        let mut out = vec![Frame::ZERO; 1];

        mixer.render(&mut out);

        assert_eq!(out[0], Frame::from_mono(0.75));
    }

    #[test]
    fn removes_finished_voices_after_render() {
        let mut mixer = SampleMixer::new(1);
        mixer.push(loaded_trigger("kick", &[0.25], 1, None));
        let mut out = vec![Frame::ZERO; 2];

        mixer.render(&mut out);

        assert_eq!(mixer.active_voice_count(), 0);
    }

    #[test]
    fn hat_choke_replaces_prior_hat_voice_with_fade_out() {
        let mut mixer = SampleMixer::new(1_000);
        mixer.push(loaded_trigger(
            "hat_one",
            &[0.3; 20],
            1_000,
            Some(ChokeGroup::Hat),
        ));
        mixer.push(loaded_trigger(
            "hat_two",
            &[0.4; 20],
            1_000,
            Some(ChokeGroup::Hat),
        ));

        assert_eq!(mixer.active_voice_count(), 2);

        let mut out = vec![Frame::ZERO; 8];
        mixer.render(&mut out);

        assert_eq!(mixer.active_voice_count(), 1);
    }

    #[test]
    fn release_note_releases_matching_live_voices() {
        let note = NoteNumber::new(60).unwrap();
        let voice_id = VoiceInstanceId::new(60);
        let mut mixer = SampleMixer::new(8_000);
        mixer.push(LoadedSampleTrigger {
            trigger: SampleTrigger::builder()
                .voice_id(voice_id)
                .sample("pad")
                .live_note(note)
                .envelope(SampleEnvelope::new(
                    Duration::ZERO,
                    Duration::ZERO,
                    UnitValue::new(1.0).unwrap(),
                    Duration::from_millis(10),
                    None,
                    Duration::ZERO,
                ))
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            sample_key: "pad".to_string(),
            sample: Arc::new(SampleBuffer::new(8_000, vec![Frame::from_mono(1.0); 256])),
            playback_limit: None,
            choke_group: None,
        });

        mixer.release_voice(voice_id);
        let mut out = vec![Frame::ZERO; 64];
        mixer.render(&mut out);

        assert!(out.iter().any(|frame| frame.left > 0.0));
    }

    #[test]
    fn reverb_and_delay_sends_add_wet_output() {
        let mut mixer = SampleMixer::new(8_000);
        mixer.push(LoadedSampleTrigger {
            trigger: SampleTrigger::builder()
                .sample("pad")
                .reverb(ReverbSettings::new(
                    UnitValue::new(0.5).unwrap(),
                    Duration::from_secs_f64(2.0),
                    UnitValue::new(0.3).unwrap(),
                ))
                .delay(DelaySettings::new(
                    UnitValue::new(0.4).unwrap(),
                    Duration::from_millis(10),
                    UnitValue::new(0.2).unwrap(),
                    UnitValue::new(0.2).unwrap(),
                ))
                .envelope(envelope())
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
            sample_key: "pad".to_string(),
            sample: Arc::new(SampleBuffer::new(8_000, vec![Frame::from_mono(1.0); 256])),
            playback_limit: None,
            choke_group: None,
        });

        let mut out = vec![Frame::ZERO; 200];
        mixer.render(&mut out);

        assert!(out.iter().skip(20).any(|frame| frame.left.abs() > 0.0));
    }

    #[test]
    fn sample_and_synth_voices_can_coexist_in_the_same_mix() {
        let mut mixer = SampleMixer::new(8_000);
        mixer.push(loaded_trigger("kick", &[0.25; 16], 8_000, None));
        mixer.push_synth(synth_trigger(BuiltInSynthSource::Sine, 69.0));
        let mut out = vec![Frame::ZERO; 32];

        mixer.render(&mut out);

        assert!(out.iter().any(|frame| *frame != Frame::ZERO));
    }

    #[test]
    fn release_note_releases_matching_live_synth_voices() {
        let note = NoteNumber::new(64).unwrap();
        let voice_id = VoiceInstanceId::new(64);
        let mut mixer = SampleMixer::new(8_000);
        mixer.push_synth(
            SynthTrigger::builder()
                .voice_id(voice_id)
                .source(BuiltInSynthSource::Triangle)
                .pitch(64.0)
                .live_note(note)
                .envelope(SampleEnvelope::new(
                    Duration::ZERO,
                    Duration::ZERO,
                    UnitValue::new(1.0).unwrap(),
                    Duration::from_millis(10),
                    None,
                    Duration::ZERO,
                ))
                .play_for(Duration::ZERO)
                .build()
                .unwrap(),
        );

        mixer.release_voice(voice_id);
        let mut out = vec![Frame::ZERO; 64];
        mixer.render(&mut out);

        assert!(out.iter().any(|frame| frame.left.abs() > 0.0));
    }

    #[test]
    fn synth_polyphony_prefers_stealing_released_voices_then_oldest_active_voice() {
        let mut mixer = SampleMixer::new(8_000);
        let released = VoiceInstanceId::new(60);

        for note in 0..8 {
            mixer.push_synth(
                SynthTrigger::builder()
                    .voice_id(VoiceInstanceId::new(60 + i64::from(note)))
                    .source(BuiltInSynthSource::Saw)
                    .pitch(60.0 + f64::from(note))
                    .live_note(NoteNumber::new(60 + note).unwrap())
                    .envelope(SampleEnvelope::new(
                        Duration::ZERO,
                        Duration::ZERO,
                        UnitValue::new(1.0).unwrap(),
                        Duration::from_millis(20),
                        None,
                        Duration::ZERO,
                    ))
                    .play_for(Duration::ZERO)
                    .build()
                    .unwrap(),
            );
        }

        mixer.release_voice(released);
        mixer.push_synth(synth_trigger(BuiltInSynthSource::Square, 84.0));

        assert_eq!(mixer.active_voice_count(), 8);
    }
}
