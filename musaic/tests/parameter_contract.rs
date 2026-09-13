use cadence::prelude::{
    CadenceCompiler, ControlKey, ControlMap, ControlMerge, ControlSupport, ControlTiming,
    ControlValue, ControlValueKind, PreparedScore, Span, Time,
};
use musaic::application::pipeline::lowering::lower_tessera_ir;
use tessera::prelude::{
    ControlEvent, ControlKeyIr, ControlStream, ControlValueIr, CycleDuration, CycleSpan, CycleTime,
    EventValue, FieldValue, NodeId, ParameterKey, ParameterMerge, ParameterSource, ParameterTiming,
    ParameterUnit, PatternEvent, PatternIr, PatternNodeIr, PatternOutput, PatternStream, Rational,
};

fn host_contract(key: ParameterKey) -> Option<(ControlKey, ParameterUnit)> {
    Some(match key {
        ParameterKey::Fast | ParameterKey::Slow | ParameterKey::Late | ParameterKey::Slice => {
            return None;
        }
        ParameterKey::Gain => (ControlKey::Gain, ParameterUnit::LinearGain),
        ParameterKey::Velocity => (ControlKey::Velocity, ParameterUnit::UnitLevel),
        ParameterKey::ClipLength => (ControlKey::ClipLength, ParameterUnit::DurationRatio),
        ParameterKey::PostGain => (ControlKey::PostGain, ParameterUnit::LinearGain),
        ParameterKey::PitchBend => (ControlKey::PitchBend, ParameterUnit::PitchBendAmount),
        ParameterKey::Expression => (ControlKey::Expression, ParameterUnit::UnitLevel),
        ParameterKey::Delay => (ControlKey::DelaySend, ParameterUnit::EffectSettings),
        ParameterKey::Reverb => (ControlKey::ReverbSend, ParameterUnit::EffectSettings),
        ParameterKey::Compressor => (ControlKey::Compressor, ParameterUnit::EffectSettings),
        ParameterKey::Attack => (ControlKey::Attack, ParameterUnit::Seconds),
        ParameterKey::Decay => (ControlKey::Decay, ParameterUnit::Seconds),
        ParameterKey::Release => (ControlKey::Release, ParameterUnit::Seconds),
        ParameterKey::Pan => (ControlKey::Pan, ParameterUnit::StereoPosition),
        ParameterKey::HighPassCutoff => (ControlKey::HighPassCutoff, ParameterUnit::Hertz),
        ParameterKey::HighPassResonance => {
            (ControlKey::HighPassResonance, ParameterUnit::UnitLevel)
        }
        ParameterKey::Transpose => (ControlKey::Transpose, ParameterUnit::Semitones),
        ParameterKey::Gate => (ControlKey::Gate, ParameterUnit::Boolean),
        ParameterKey::Legato => (ControlKey::Legato, ParameterUnit::DurationRatio),
        ParameterKey::Sustain => (ControlKey::Sustain, ParameterUnit::UnitLevel),
        ParameterKey::LowPassCutoff => (ControlKey::LowPassCutoff, ParameterUnit::Hertz),
        ParameterKey::LowPassResonance => (ControlKey::LowPassResonance, ParameterUnit::UnitLevel),
        ParameterKey::SampleBank => (ControlKey::SampleBank, ParameterUnit::BankName),
        ParameterKey::SampleVariant => (ControlKey::SampleVariant, ParameterUnit::VariantIndex),
        ParameterKey::PlaybackRate => (ControlKey::PlaybackRate, ParameterUnit::RateRatio),
        ParameterKey::PlaybackStart => (ControlKey::PlaybackStart, ParameterUnit::SourcePosition),
        ParameterKey::PlaybackEnd => (ControlKey::PlaybackEnd, ParameterUnit::SourcePosition),
        ParameterKey::Reverse => (ControlKey::Reverse, ParameterUnit::Boolean),
        ParameterKey::Fit => (ControlKey::Fit, ParameterUnit::Boolean),
        ParameterKey::Loop => (ControlKey::Loop, ParameterUnit::Boolean),
    })
}

fn actual_lowered_controls(
    controls: Vec<(ControlKeyIr, ControlValueIr)>,
) -> Result<ControlMap, String> {
    let cycle = CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()));
    let ir = PatternIr::new(vec![PatternOutput::new(
        NodeId::new("out"),
        PatternNodeIr::merge(vec![
            PatternNodeIr::cycle_event_stream(PatternStream::new(vec![PatternEvent::new(
                cycle,
                EventValue::Note {
                    value: "pad".into(),
                    octave: None,
                },
            )])),
            PatternNodeIr::control_stream(ControlStream::new(
                controls
                    .into_iter()
                    .map(|(key, value)| ControlEvent::new(cycle, key, value))
                    .collect(),
            )),
        ]),
    )]);
    let (scores, diagnostics) = lower_tessera_ir(&ir);
    if !diagnostics.is_empty() {
        return Err(format!("{diagnostics:?}"));
    }
    let prepared = PreparedScore::new(scores[&NodeId::new("out")].clone())
        .map_err(|error| format!("{error:?}"))?;
    let report = CadenceCompiler::new()
        .preview(&prepared, &Span::new(Time::ZERO, Time::ONE).unwrap())
        .map_err(|error| format!("{error:?}"))?;
    Ok(report
        .starts()
        .next()
        .expect("sample fixture should start")
        .projected()
        .controls()
        .clone())
}

#[test]
fn shared_catalog_matches_engine_metadata_and_actual_lowering() {
    for key in ParameterKey::ALL {
        let language = key.spec();
        let Some((host_key, unit)) = host_contract(*key) else {
            if *key == ParameterKey::Slice {
                assert_eq!(language.unit, ParameterUnit::SliceSelection);
                assert_eq!(language.timing, ParameterTiming::Onset);
                assert_eq!(language.source, ParameterSource::SampleOnly);
                assert!(!language.accepts_pattern);
            } else if *key == ParameterKey::Late {
                assert_eq!(language.unit, ParameterUnit::DurationRatio);
                assert_eq!(language.timing, ParameterTiming::PatternTime);
                assert_eq!(language.merge, ParameterMerge::Add);
                assert_eq!(language.source, ParameterSource::Pattern);
            } else {
                assert_eq!(language.unit, ParameterUnit::RateRatio);
                assert_eq!(language.timing, ParameterTiming::PatternTime);
            }
            assert_eq!(language.host_control_name, None);
            assert_eq!(key.control_key(), None);
            continue;
        };
        let engine = host_key.spec();
        assert_eq!(language.unit, unit, "{key:?}");
        assert_eq!(
            language.host_control_name,
            Some(engine.canonical_name()),
            "{key:?}"
        );
        let timing = match language.timing {
            ParameterTiming::PatternTime => panic!("sound parameter has pattern-time scope"),
            ParameterTiming::Onset => ControlTiming::Onset,
            ParameterTiming::VoiceLifecycle => ControlTiming::VoiceLifecycle,
            ParameterTiming::SegmentSampled => ControlTiming::SegmentSampled,
            ParameterTiming::ContinuousRuntime => ControlTiming::ContinuousRuntime,
        };
        assert_eq!(timing, engine.timing(), "{key:?}");
        let merge = match language.merge {
            ParameterMerge::Multiply => ControlMerge::Multiply,
            ParameterMerge::Add => ControlMerge::Add,
            ParameterMerge::Override => ControlMerge::Override,
        };
        assert_eq!(merge, engine.merge(), "{key:?}");
        let source = match language.source {
            ParameterSource::Pattern => panic!("sound parameter has pattern-only support"),
            ParameterSource::SharedSound => ControlSupport::Shared,
            ParameterSource::SampleOnly => ControlSupport::SampleOnly,
        };
        assert_eq!(source, engine.support(), "{key:?}");
        // Units and shape are separate: normalized resonance is a Scalar lane,
        // whereas the envelope sustain lane advertises Unipolar.
        let shape = match key {
            ParameterKey::Delay => ControlValueKind::DelaySettings,
            ParameterKey::Reverb => ControlValueKind::ReverbSettings,
            ParameterKey::Compressor => ControlValueKind::CompressorSettings,
            ParameterKey::Gate | ParameterKey::Reverse | ParameterKey::Fit | ParameterKey::Loop => {
                ControlValueKind::Bool
            }
            ParameterKey::SampleBank => ControlValueKind::Choice,
            ParameterKey::Sustain | ParameterKey::Velocity | ParameterKey::Expression => {
                ControlValueKind::Unipolar
            }
            ParameterKey::Pan | ParameterKey::PitchBend => ControlValueKind::Bipolar,
            _ => ControlValueKind::Scalar,
        };
        assert_eq!(engine.value_kind(), shape, "{key:?}");
        let value = language
            .default
            .clone()
            .unwrap_or_else(|| FieldValue::symbol("drums"));
        assert!(language.validate(&value).is_ok());
        let value = match value {
            FieldValue::Modulation { value } => ControlValueIr::Modulation { value },
            FieldValue::Delay { value } => ControlValueIr::Delay { value },
            FieldValue::Reverb { value } => ControlValueIr::Reverb { value },
            FieldValue::Compressor { value } => ControlValueIr::Compressor { value },
            FieldValue::Rational { value } => ControlValueIr::rational(value),
            FieldValue::Bool { value } => ControlValueIr::bool(value),
            FieldValue::Symbol { value } => ControlValueIr::symbol(value),
            FieldValue::Slice { .. } => {
                panic!("Composite slice must be resolved against its sample source")
            }
        };
        let actual = actual_lowered_controls(vec![(key.control_key().unwrap(), value)]).unwrap();
        assert!(
            actual.contains_key(&host_key),
            "{key:?} did not reach {host_key:?}"
        );
    }
}

#[test]
fn transpose_and_pitch_bend_retain_different_units_in_audio_conversion() {
    use cadence::application::audio::{AudioRuntimeControlDelta, RuntimeControlValue};
    let actual = actual_lowered_controls(vec![
        (
            ControlKeyIr::Transpose,
            ControlValueIr::rational(Rational::from_integer(12)),
        ),
        (
            ControlKeyIr::PitchBend,
            ControlValueIr::rational(Rational::new(-1, 2)),
        ),
    ])
    .unwrap();
    let delta = AudioRuntimeControlDelta::from_runtime_controls_snapshot(&actual);
    let RuntimeControlValue::Set(transpose) = delta.transpose else {
        panic!("transpose was lost");
    };
    assert_eq!(transpose.eval(0.0), 12.0);
    assert_eq!(delta.pitch_bend_semitones, RuntimeControlValue::Set(-1.0));
    assert_eq!(
        ControlKey::PitchBend.spec().value_kind(),
        ControlValueKind::Bipolar
    );
    assert_eq!(
        ControlKey::PitchBend.spec().timing(),
        ControlTiming::ContinuousRuntime
    );
    assert!(
        actual_lowered_controls(vec![(
            ControlKeyIr::PitchBend,
            ControlValueIr::rational(Rational::from_integer(2))
        )])
        .is_err()
    );
    assert_eq!(
        ParameterKey::PitchBend.control_key(),
        Some(ControlKeyIr::PitchBend)
    );
}

#[test]
fn shared_numeric_domains_agree_without_confusing_editor_ranges_with_limits() {
    for key in [
        ParameterKey::Gain,
        ParameterKey::Attack,
        ParameterKey::ClipLength,
        ParameterKey::PostGain,
        ParameterKey::Velocity,
        ParameterKey::Decay,
        ParameterKey::Release,
        ParameterKey::HighPassCutoff,
        ParameterKey::HighPassResonance,
        ParameterKey::Transpose,
        ParameterKey::Legato,
        ParameterKey::Sustain,
        ParameterKey::LowPassCutoff,
        ParameterKey::LowPassResonance,
        ParameterKey::PlaybackRate,
        ParameterKey::PlaybackStart,
        ParameterKey::PlaybackEnd,
    ] {
        let (host, _) = host_contract(key).unwrap();
        for value in [-128, -127, -1, 0, 1, 2, 127, 128, 25_000, 65_536, 65_537] {
            assert_eq!(
                key.spec()
                    .validate(&FieldValue::rational(Rational::from_integer(value)))
                    .is_ok(),
                ControlValue::Scalar(value as f64)
                    .validate_for(&host)
                    .is_ok(),
                "{key:?}: {value}"
            );
        }
    }
    // These authoring restrictions intentionally exceed the engine's generic
    // scalar/choice validation; do not claim complete domain equality for them.
    assert!(
        ParameterKey::SampleVariant
            .spec()
            .validate(&FieldValue::rational(Rational::new(1, 2)))
            .is_err()
    );
    assert!(
        ParameterKey::SampleVariant
            .spec()
            .validate(&FieldValue::rational(Rational::from_integer(
                i64::from(u32::MAX) + 1
            )))
            .is_err()
    );
    assert!(
        ParameterKey::SampleBank
            .spec()
            .validate(&FieldValue::symbol(" "))
            .is_err()
    );
}

#[test]
fn pan_uses_a_bipolar_value_and_preserves_its_runtime_stereo_unit() {
    use cadence::application::audio::{AudioRuntimeControlDelta, RuntimeControlValue};
    for value in [-1, 0, 1] {
        let actual = actual_lowered_controls(vec![(
            ControlKeyIr::Pan,
            ControlValueIr::rational(Rational::from_integer(value)),
        )])
        .unwrap();
        assert!(matches!(
            actual.get(&ControlKey::Pan),
            Some(ControlValue::Bipolar(_))
        ));
        assert_eq!(
            AudioRuntimeControlDelta::from_runtime_controls_snapshot(&actual).pan,
            RuntimeControlValue::Set(value as f64),
        );
    }
    for value in [-2, 2] {
        let scalar = Rational::from_integer(value);
        assert!(
            ParameterKey::Pan
                .spec()
                .validate(&FieldValue::rational(scalar))
                .is_err()
        );
        assert!(
            actual_lowered_controls(vec![(ControlKeyIr::Pan, ControlValueIr::rational(scalar))])
                .is_err()
        );
    }
}

#[test]
fn malformed_structured_effects_are_diagnostics_instead_of_constructor_panics() {
    use tessera::prelude::{CompressorParameters, DelayParameters, ReverbParameters};
    for (key, value) in [
        (
            ControlKeyIr::DelaySend,
            ControlValueIr::Delay {
                value: DelayParameters {
                    time: Rational::zero(),
                    ..Default::default()
                },
            },
        ),
        (
            ControlKeyIr::ReverbSend,
            ControlValueIr::Reverb {
                value: ReverbParameters {
                    amount: Rational::from_integer(2),
                    ..Default::default()
                },
            },
        ),
        (
            ControlKeyIr::Compressor,
            ControlValueIr::Compressor {
                value: CompressorParameters {
                    ratio: Rational::zero(),
                    ..Default::default()
                },
            },
        ),
        (
            ControlKeyIr::Gain,
            ControlValueIr::Delay {
                value: DelayParameters::default(),
            },
        ),
    ] {
        assert!(actual_lowered_controls(vec![(key, value)]).is_err());
    }
}

#[test]
fn modulation_is_typed_and_lowered_only_for_implemented_audio_lanes() {
    use tessera::prelude::ModulationParameters;
    let supported = [
        ParameterKey::Gain,
        ParameterKey::Velocity,
        ParameterKey::PlaybackRate,
        ParameterKey::LowPassCutoff,
        ParameterKey::Transpose,
    ];
    for key in ParameterKey::ALL {
        let value = ModulationParameters::for_parameter(*key);
        assert_eq!(
            key.spec()
                .validate(&FieldValue::Modulation { value })
                .is_ok(),
            supported.contains(key),
            "{key:?}"
        );
        if !supported.contains(key) {
            continue;
        }
        let actual = actual_lowered_controls(vec![(
            key.control_key().unwrap(),
            ControlValueIr::Modulation { value },
        )])
        .unwrap();
        let (host, _) = host_contract(*key).unwrap();
        match actual.get(&host).unwrap() {
            ControlValue::Signal(signal) => {
                assert!(signal.eval(0.0).is_finite());
                assert_eq!(signal.rate(), Time::ONE);
            }
            ControlValue::Semitones(value) if *key == ParameterKey::Transpose => {
                assert!(value.eval(0.25).is_finite())
            }
            ControlValue::Unipolar(value) if *key == ParameterKey::Velocity => {
                assert_eq!(value.value(), 0.5)
            }
            value => panic!("{key:?} lost its continuous value: {value:?}"),
        }
    }
    for key in [ParameterKey::PlaybackRate, ParameterKey::LowPassCutoff] {
        let invalid = ModulationParameters {
            minimum: Rational::zero(),
            ..ModulationParameters::for_parameter(key)
        };
        assert!(
            actual_lowered_controls(vec![(
                key.control_key().unwrap(),
                ControlValueIr::Modulation { value: invalid }
            )])
            .is_err()
        );
    }
}

#[test]
fn fractional_random_lowers_for_every_supported_signal_target() {
    use tessera::prelude::{ModulationParameters, ModulationWaveform};
    for key in [
        ParameterKey::Gain,
        ParameterKey::Velocity,
        ParameterKey::PlaybackRate,
        ParameterKey::LowPassCutoff,
        ParameterKey::Transpose,
    ] {
        let value = ModulationParameters {
            waveform: ModulationWaveform::Random,
            seed: 73,
            ..ModulationParameters::for_parameter(key)
        };
        key.spec()
            .validate(&FieldValue::Modulation { value })
            .unwrap();
        let actual = actual_lowered_controls(vec![(
            key.control_key().unwrap(),
            ControlValueIr::Modulation { value },
        )])
        .unwrap();
        let (host, _) = host_contract(key).unwrap();
        match &actual[&host] {
            ControlValue::Signal(signal) => assert_eq!(
                signal.waveform(),
                cadence::prelude::Waveform::Random { seed: 73 }
            ),
            ControlValue::Semitones(value) if key == ParameterKey::Transpose => {
                assert!(value.eval(0.25).is_finite())
            }
            ControlValue::Unipolar(value) if key == ParameterKey::Velocity => {
                let expected = cadence::prelude::Signal::random(73)
                    .with_bias(0.5)
                    .with_depth(0.5)
                    .eval_at(Time::ZERO);
                assert_eq!(value.value(), expected);
            }
            actual => panic!("{key:?} lost fractional Random: {actual:?}"),
        }
    }
}
