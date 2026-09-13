//! Timing and clipping preserve the source onset used by velocity signals.
use tessera::prelude::*;

fn span(start: Rational, duration: Rational) -> CycleSpan {
    CycleSpan::new(CycleTime(start), CycleDuration(duration))
}

#[test]
fn clipped_velocity_notes_retain_the_unclipped_onset() {
    let settings = ModulationParameters {
        waveform: ModulationWaveform::Saw,
        rate: Rational::from_integer(4),
        phase: Rational::new(1, 7),
        ..ModulationParameters::for_parameter(ParameterKey::Velocity)
    };
    let source = PatternStream::new(vec![
        PatternEvent::new(
            span(Rational::zero(), Rational::one()),
            EventValue::Note {
                value: "c".into(),
                octave: Some(4),
            },
        )
        .with_fields(vec![EventField::Velocity(FieldValue::Modulation {
            value: settings,
        })]),
    ]);
    let mask = PatternStream::new(
        [1, 3]
            .map(|index| {
                PatternEvent::new(
                    span(Rational::new(index, 8), Rational::new(1, 8)),
                    EventValue::Scalar {
                        value: Rational::one(),
                    },
                )
            })
            .to_vec(),
    );
    let clipped = source.clip_by_mask(&mask);
    assert_eq!(clipped.events.len(), 2);
    for event in clipped.events {
        let EventField::Velocity(FieldValue::Modulation { value }) = &event.fields[0] else {
            panic!("velocity signal missing")
        };
        assert_eq!(
            value.phase + value.rate * event.span.start.0,
            settings.phase
        );
    }
}
