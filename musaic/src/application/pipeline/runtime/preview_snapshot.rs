use std::collections::BTreeMap;

use bevy::prelude::*;
use cadence::{
    domain::{control::ControlKey, intent::Intent},
    prelude::{PreviewReport, Span, Time as CycleTime},
};

use super::ProjectedEventId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineEventKind {
    StartVoice,
    ControlUpdate,
}

#[derive(Debug, Clone)]
pub struct TimelineEventRecord {
    pub id: ProjectedEventId,
    pub output_id: String,
    pub source_id: Option<u64>,
    pub kind: TimelineEventKind,
    pub visible_start: CycleTime,
    pub visible_end: CycleTime,
    pub label: String,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct RuntimePreviewSnapshot {
    pub feedback: super::feedback::PlaybackFeedback,
    pub window: Option<Span>,
    pub pending_cycle: Option<CycleTime>,
    pub browsing: bool,
    events: BTreeMap<ProjectedEventId, TimelineEventRecord>,
    next_id: u64,
}

impl RuntimePreviewSnapshot {
    pub fn clear(&mut self) {
        self.window = None;
        self.pending_cycle = None;
        self.browsing = false;
        self.events.clear();
    }

    pub fn event(&self, id: ProjectedEventId) -> Option<&TimelineEventRecord> {
        self.events.get(&id)
    }

    pub fn events(&self) -> impl Iterator<Item = &TimelineEventRecord> {
        self.events.values()
    }

    /// Test helper: insert a synthetic event with a fixed id.
    #[cfg(test)]
    pub fn insert_test_event(&mut self, id: ProjectedEventId) {
        use cadence::prelude::Time as CycleTime;
        self.events.insert(
            id,
            TimelineEventRecord {
                id,
                output_id: "test".into(),
                source_id: None,
                kind: TimelineEventKind::StartVoice,
                visible_start: CycleTime::ZERO,
                visible_end: CycleTime::ZERO,
                label: "test".into(),
            },
        );
        self.next_id = self.next_id.max(id.0 + 1);
    }

    pub fn starts(&self) -> impl Iterator<Item = &TimelineEventRecord> {
        self.events
            .values()
            .filter(|event| event.kind == TimelineEventKind::StartVoice)
    }

    pub fn control_updates(&self) -> impl Iterator<Item = &TimelineEventRecord> {
        self.events
            .values()
            .filter(|event| event.kind == TimelineEventKind::ControlUpdate)
    }

    pub fn ingest_report(&mut self, output_id: &str, report: &PreviewReport) {
        self.window = Some(report.window);

        for event in report.starts() {
            let id = self.next_id();
            let projected = event.projected();
            self.events.insert(
                id,
                TimelineEventRecord {
                    id,
                    output_id: output_id.to_string(),
                    source_id: projected.id().map(|id| id.value()),
                    kind: TimelineEventKind::StartVoice,
                    visible_start: projected.visible().start(),
                    visible_end: projected.visible().end(),
                    label: note_label(projected.intent(), projected.controls()),
                },
            );
        }

        for event in report.control_updates() {
            let id = self.next_id();
            let projected = event.projected();
            self.events.insert(
                id,
                TimelineEventRecord {
                    id,
                    output_id: output_id.to_string(),
                    source_id: projected.id().map(|id| id.value()),
                    kind: TimelineEventKind::ControlUpdate,
                    visible_start: projected.visible().start(),
                    visible_end: projected.visible().end(),
                    label: label_for_controls(projected.controls()),
                },
            );
        }
    }

    fn next_id(&mut self) -> ProjectedEventId {
        let id = ProjectedEventId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }
}

fn note_label(intent: &Intent, controls: &cadence::domain::control::ControlMap) -> String {
    use cadence::domain::control::ControlValue;
    if let Some(ControlValue::Scalar(pitch)) = controls.get(&ControlKey::Pitch) {
        let transpose = match controls.get(&ControlKey::Transpose) {
            Some(ControlValue::Scalar(value)) => *value,
            _ => 0.0,
        };
        let pitch = pitch + transpose;
        if pitch.is_finite() && (-128.0..=255.0).contains(&pitch) {
            let midi = pitch.round() as i32;
            let name = [
                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
            ][midi.rem_euclid(12) as usize];
            return format!("{name}{}", midi.div_euclid(12) - 1);
        }
    }
    match intent {
        Intent::Sample(sample) => match sample.sample_id.as_str() {
            "bd" => "Kick".into(),
            "sd" => "Snare".into(),
            "hh" => "Hat".into(),
            "oh" => "Open hat".into(),
            _ => "Sample".into(),
        },
        Intent::Synth(_) => "Note".into(),
        Intent::Toggle(toggle) => format!("toggle:{}", toggle.target),
        Intent::Level(level) => format!("level:{}", level.target),
        Intent::Rate(rate) => format!("rate:{}", rate.target),
        Intent::Region(region) => format!("region:{}", region.target),
        Intent::Gate(gate) => format!("gate:{}", gate.target),
        Intent::Select(select) => format!("select:{}", select.target),
    }
}

fn label_for_controls(controls: &cadence::domain::control::ControlMap) -> String {
    if controls.is_empty() {
        return "control update".into();
    }

    controls
        .keys()
        .map(control_key_label)
        .collect::<Vec<_>>()
        .join(", ")
}

fn control_key_label(key: &ControlKey) -> &'static str {
    match key {
        ControlKey::Gate => "gate",
        ControlKey::Gain => "gain",
        ControlKey::PostGain => "post_gain",
        ControlKey::Pan => "pan",
        ControlKey::PitchBend => "pitch_bend",
        ControlKey::PlaybackRate => "playback_rate",
        ControlKey::Attack => "attack",
        ControlKey::Decay => "decay",
        ControlKey::Sustain => "sustain",
        ControlKey::Release => "release",
        ControlKey::Expression => "expression",
        ControlKey::ModWheel => "mod_wheel",
        ControlKey::SustainPedal => "sustain_pedal",
        _ => "control",
    }
}

#[cfg(test)]
mod pitch_label_tests {
    use super::*;
    use cadence::prelude::{BuiltInSynthSource, ControlValue};

    #[test]
    fn preview_labels_show_sounding_pitch_across_octave_boundaries() {
        let source = Intent::synth(BuiltInSynthSource::Sine);
        for (pitch, transpose, expected) in
            [(61.0, 0.0, "C#4"), (59.0, 1.0, "C4"), (60.0, -1.0, "B3")]
        {
            let controls = std::collections::BTreeMap::from([
                (ControlKey::Pitch, ControlValue::Scalar(pitch)),
                (ControlKey::Transpose, ControlValue::Scalar(transpose)),
            ]);
            assert_eq!(note_label(&source, &controls), expected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadence::prelude::{
        CadenceCompiler, ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue, Score,
        Time, Voice, sample, tile,
    };

    #[test]
    fn snapshot_ingests_starts_and_control_updates() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::Gain,
                    ControlValue::Scalar(1.0),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Scalar(0.5),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let voice =
            Voice::new(Time::ONE, vec![tile(Time::ZERO, Time::ONE, sample("pad"))]).unwrap();
        let score = Score::with_controls(Score::from(voice), ControlScore::from(controls));
        let window = Span::new(Time::ZERO, Time::ONE).unwrap();
        let prepared = cadence::prelude::PreparedScore::new(score).unwrap();
        let report = CadenceCompiler::new().preview(&prepared, &window).unwrap();

        let mut snapshot = RuntimePreviewSnapshot::default();
        snapshot.ingest_report("main", &report);

        assert_eq!(snapshot.starts().count(), 1);
        assert_eq!(snapshot.control_updates().count(), 1);
        assert!(
            snapshot
                .control_updates()
                .next()
                .is_some_and(|event| event.label.contains("gain"))
        );
    }
}
