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
    pub kind: TimelineEventKind,
    pub visible_start: CycleTime,
    pub visible_end: CycleTime,
    pub label: String,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct RuntimePreviewSnapshot {
    pub window: Option<Span>,
    events: BTreeMap<ProjectedEventId, TimelineEventRecord>,
    next_id: u64,
}

impl RuntimePreviewSnapshot {
    pub fn clear(&mut self) {
        self.window = None;
        self.events.clear();
        self.next_id = 0;
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
                    kind: TimelineEventKind::StartVoice,
                    visible_start: projected.visible().start(),
                    visible_end: projected.visible().end(),
                    label: label_for_intent(projected.intent()),
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

fn label_for_intent(intent: &Intent) -> String {
    match intent {
        Intent::Sample(sample) => format!("sample:{}", sample.sample_id),
        Intent::Synth(synth) => format!("synth:{:?}", synth.source),
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
        let report = CadenceCompiler::new().preview(&score, &window).unwrap();

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
