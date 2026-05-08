//! Bindings between Cadence host IR and `cadence_core`.

#[cfg(not(target_arch = "wasm32"))]
mod native_audio;
mod runtime_host;
#[cfg(target_arch = "wasm32")]
pub(crate) mod web_audio;

use std::{collections::BTreeMap, time::Duration};

use cadence_core::infrastructure::{
    projection::TransportSpan,
    render::RendererCore,
    score::{
        CompressorSettings, ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue,
        DelaySettings, ReverbSettings, Score, Symbol, UnitValue,
    },
    voice::{BuiltInSynthSource, Intent, Repeat, Tile, Time, Voice},
};

use crate::domain::{
    common::GridPos,
    preview::{PreviewEvent, RationalTime},
    program::{
        CadenceCallArg, CadenceControlKey, CadenceControlValueExpr, CadenceOutput,
        CadencePatternExpr, CadenceProgram, CadenceSourceKind, CadenceSynthSource,
        CadenceValueExpr,
    },
    script::parse_pitch_text,
};

pub(crate) use runtime_host::MusicRuntimeHost;
pub(crate) type PlaybackScore = Score;
pub(crate) type PlaybackTime = Time;
pub(crate) type PlaybackState = cadence_core::infrastructure::playback::PlaybackState;

pub(crate) fn program_output_scores(program: &CadenceProgram) -> Vec<(GridPos, Score)> {
    program
        .outputs
        .iter()
        .filter_map(|output| lower_output_score(output, program).map(|score| (output.root, score)))
        .collect()
}

pub(crate) fn merged_playback_score(outputs: &[(GridPos, Score)]) -> Option<Score> {
    match outputs {
        [] => None,
        [(_, score)] => Some(score.clone()),
        many => Some(Score::merge(
            many.iter().map(|(_, score)| score.clone()).collect(),
        )),
    }
}

pub(crate) fn preview_events_for_scores(outputs: &[(GridPos, Score)]) -> Vec<PreviewEvent> {
    let window = one_cycle_window();
    let mut events = Vec::new();

    for (output_lane, score) in outputs {
        let Ok(projected) = RendererCore::new(score.clone()).projected_output(&window) else {
            continue;
        };
        for moment in projected.iter() {
            if matches!(
                moment.controls().get(&ControlKey::Gate),
                Some(ControlValue::Bool(false))
            ) {
                continue;
            }
            let label = match moment.intent() {
                Intent::Sample(sample) => sample.sample_id.clone(),
                Intent::Synth(synth) => render_synth_source(synth.source).to_string(),
                Intent::Select(select) => select.option.clone(),
                other => format!("{other:?}"),
            };
            let pitch = preview_pitch_label(moment);
            events.push(PreviewEvent {
                start: rational_time(moment.visible().start()),
                end: rational_time(moment.visible().end()),
                start_normalized: moment.visible().start().value(),
                end_normalized: moment.visible().end().value(),
                label,
                site: moment.id().and_then(|id| decode_site_id(id.value())),
                output_lane: (*output_lane).into(),
                pitch,
            });
        }
    }

    events
}

fn preview_pitch_label(
    moment: &cadence_core::infrastructure::projection::ProjectedMoment,
) -> Option<String> {
    match moment.controls().get(&ControlKey::Pitch) {
        Some(ControlValue::Scalar(value)) => Some(format_pitch_label(*value)),
        Some(ControlValue::Choice(symbol)) => Some(symbol.as_str().to_string()),
        _ => match moment.intent() {
            Intent::Select(select) if select.target == "note" => Some(select.option.clone()),
            _ => None,
        },
    }
}

fn format_pitch_label(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }

    let rounded = value.round();
    if (value - rounded).abs() >= 1e-9 {
        return value.to_string();
    }

    let midi = rounded as i32;
    let name = match midi.rem_euclid(12) {
        0 => "c",
        1 => "c#",
        2 => "d",
        3 => "d#",
        4 => "e",
        5 => "f",
        6 => "f#",
        7 => "g",
        8 => "g#",
        9 => "a",
        10 => "a#",
        11 => "b",
        _ => unreachable!("pitch class is always within 0..12"),
    };
    let octave = midi.div_euclid(12) - 1;
    format!("{name}{octave}")
}

fn render_synth_source(source: BuiltInSynthSource) -> &'static str {
    match source {
        BuiltInSynthSource::Sine => "sine",
        BuiltInSynthSource::Triangle => "triangle",
        BuiltInSynthSource::Saw => "saw",
        BuiltInSynthSource::Square => "square",
    }
}

fn lower_output_score(output: &CadenceOutput, program: &CadenceProgram) -> Option<Score> {
    let pattern =
        substitute_pattern_expr(&output.expr, &BTreeMap::new(), &BTreeMap::new(), program);
    lower_pattern_expr_to_score(&pattern, program, output.root)
}

fn lower_pattern_expr_to_score(
    expr: &CadencePatternExpr,
    program: &CadenceProgram,
    fallback_site: GridPos,
) -> Option<Score> {
    match expr {
        CadencePatternExpr::Silence => None,
        CadencePatternExpr::Argument { .. } => None,
        CadencePatternExpr::Source {
            source: CadenceSourceKind::Sample,
            value: CadenceValueExpr::Text { value },
            site,
        } => source_score(Intent::sample(value), site.unwrap_or(fallback_site)),
        CadencePatternExpr::Source {
            source: CadenceSourceKind::Synth,
            value: CadenceValueExpr::SynthSource { source },
            site,
        } => source_score(
            Intent::synth(lower_synth_source(*source)),
            site.unwrap_or(fallback_site),
        ),
        CadencePatternExpr::Source {
            source: CadenceSourceKind::Note,
            value,
            site,
        } => source_score(
            Intent::select("note", value_as_text(value)?),
            site.unwrap_or(fallback_site),
        ),
        CadencePatternExpr::Source {
            source: CadenceSourceKind::Pulse,
            value,
            site,
        } => source_score(
            Intent::select("pulse", value_as_text(value)?),
            site.unwrap_or(fallback_site),
        ),
        CadencePatternExpr::Source { .. } => None,
        CadencePatternExpr::Merge { branches } => lower_parallel_branch_collection(
            branches.as_slice(),
            program,
            fallback_site,
            Score::merge,
        ),
        CadencePatternExpr::CycleRoute { branches } => lower_structured_branch_collection(
            branches.as_slice(),
            program,
            fallback_site,
            Score::cycle_route,
        ),
        CadencePatternExpr::CycleSlots { branches } => lower_structured_branch_collection(
            branches.as_slice(),
            program,
            fallback_site,
            Score::cycle_slots,
        ),
        CadencePatternExpr::ReflectCycle { input } => Some(Score::reflect_cycle(
            lower_pattern_expr_to_score(input, program, fallback_site)?,
        )),
        CadencePatternExpr::Mask { input, by } => {
            let source = lower_pattern_expr_to_score(input, program, fallback_site)?;
            let mask = lower_pattern_expr_to_score(by, program, fallback_site);
            apply_control_score(source, gate_controls_from_score(mask.as_ref())?)
        }
        CadencePatternExpr::Gain { input, amount } => {
            let source = lower_pattern_expr_to_score(input, program, fallback_site)?;
            let amount = value_as_number(amount)?;
            apply_constant_control(source, ControlKey::Gain, ControlValue::Scalar(amount))
        }
        CadencePatternExpr::Fast { input, factor } => {
            let source = lower_pattern_expr_to_score(input, program, fallback_site)?;
            Some(Score::time_scale(
                source,
                number_to_time(value_as_number(factor)?)?,
            ))
        }
        CadencePatternExpr::Slow { input, factor } => {
            let source = lower_pattern_expr_to_score(input, program, fallback_site)?;
            let factor = value_as_number(factor)?;
            if !(factor.is_finite() && factor > 0.0) {
                return None;
            }
            Some(Score::time_scale(source, number_to_time(1.0 / factor)?))
        }
        CadencePatternExpr::Shift { input, offset } => Some(Score::shift(
            lower_pattern_expr_to_score(input, program, fallback_site)?,
            number_to_time(value_as_number(offset)?)?,
        )),
        CadencePatternExpr::Control { input, key, value } => {
            let source = lower_pattern_expr_to_score(input, program, fallback_site)?;
            let cadence_key = *key;
            let core_key = lower_control_key(cadence_key)?;
            match value {
                CadenceControlValueExpr::Pattern { expr } => {
                    let value_score = lower_pattern_expr_to_score(expr, program, fallback_site);
                    let controls =
                        scalar_controls_from_score(core_key, cadence_key, value_score.as_ref())?;
                    apply_control_score(source, controls)
                }
                _ => {
                    let cv = lower_control_value(cadence_key, value)?;
                    apply_constant_control(source, core_key, cv)
                }
            }
        }
        CadencePatternExpr::Call { call } => {
            let function = program
                .functions
                .iter()
                .find(|function| function.trick_id == call.trick_id)?;
            let (value_args, pattern_args) = call_argument_maps(call.arguments.as_slice(), program);
            let body = substitute_pattern_expr(&function.body, &value_args, &pattern_args, program);
            lower_pattern_expr_to_score(&body, program, fallback_site)
        }
        CadencePatternExpr::State { .. } => None,
    }
}

fn lower_parallel_branch_collection(
    branches: &[CadencePatternExpr],
    program: &CadenceProgram,
    fallback_site: GridPos,
    combine: impl FnOnce(Vec<Score>) -> Score,
) -> Option<Score> {
    let mut lowered = branches
        .iter()
        .filter_map(|branch| lower_pattern_expr_to_score(branch, program, fallback_site))
        .collect::<Vec<_>>();

    match lowered.len() {
        0 => None,
        1 => lowered.pop(),
        _ => Some(combine(lowered)),
    }
}

fn lower_structured_branch_collection(
    branches: &[CadencePatternExpr],
    program: &CadenceProgram,
    fallback_site: GridPos,
    combine: impl FnOnce(Vec<Score>) -> Score,
) -> Option<Score> {
    let mut lowered = Vec::with_capacity(branches.len());
    for branch in branches {
        match branch {
            CadencePatternExpr::Silence => lowered.push(Score::empty()),
            _ => lowered.push(lower_pattern_expr_to_score(branch, program, fallback_site)?),
        }
    }

    match lowered.len() {
        0 => None,
        1 => lowered.pop(),
        _ => Some(combine(lowered)),
    }
}

fn apply_constant_control(source: Score, key: ControlKey, value: ControlValue) -> Option<Score> {
    apply_control_score(source, constant_control_score(key, value)?)
}

fn apply_control_score(source: Score, controls: ControlScore) -> Option<Score> {
    Some(Score::with_controls(source, controls))
}

fn constant_control_score(key: ControlKey, value: ControlValue) -> Option<ControlScore> {
    let controls = ControlTrack::new(
        Time::ONE,
        vec![ControlTile::spanning(Time::ZERO, Time::ONE, key, value).ok()?],
    )
    .ok()?
    .with_repeat(Repeat::Forever);

    Some(ControlScore::from(controls))
}

fn gate_controls_from_score(mask: Option<&Score>) -> Option<ControlScore> {
    let mut spans = mask
        .map(|score| {
            RendererCore::new(score.clone())
                .projected_output(&one_cycle_window())
                .ok()
                .map(|projected| {
                    projected
                        .iter()
                        .filter_map(|moment| {
                            let visible = moment.visible();
                            (visible.start() < visible.end())
                                .then_some((visible.start(), visible.end()))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    spans.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut active = Vec::<(Time, Time)>::new();
    for (start, end) in spans {
        if let Some((_, last_end)) = active.last_mut()
            && start <= *last_end
        {
            if end > *last_end {
                *last_end = end;
            }
        } else {
            active.push((start, end));
        }
    }

    let mut tiles = Vec::new();
    let mut cursor = Time::ZERO;
    if active.is_empty() {
        tiles.push(
            ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                ControlKey::Gate,
                ControlValue::Bool(false),
            )
            .ok()?,
        );
    } else {
        for (start, end) in active {
            if cursor < start {
                tiles.push(
                    ControlTile::spanning(
                        cursor,
                        start,
                        ControlKey::Gate,
                        ControlValue::Bool(false),
                    )
                    .ok()?,
                );
            }
            tiles.push(
                ControlTile::spanning(start, end, ControlKey::Gate, ControlValue::Bool(true))
                    .ok()?,
            );
            cursor = end;
        }
        if cursor < Time::ONE {
            tiles.push(
                ControlTile::spanning(
                    cursor,
                    Time::ONE,
                    ControlKey::Gate,
                    ControlValue::Bool(false),
                )
                .ok()?,
            );
        }
    }

    let track = ControlTrack::new(Time::ONE, tiles)
        .ok()?
        .with_repeat(Repeat::Forever);
    Some(ControlScore::from(track))
}

fn scalar_controls_from_score(
    key: ControlKey,
    cadence_key: CadenceControlKey,
    score: Option<&Score>,
) -> Option<ControlScore> {
    let score = score?;
    let window = one_cycle_window();
    let projected = RendererCore::new(score.clone())
        .projected_output(&window)
        .ok()?;

    let mut tiles = Vec::new();
    for moment in projected.iter() {
        let Some(cv) = projected_control_value(moment, cadence_key) else {
            continue;
        };
        if let Ok(tile) = ControlTile::spanning(
            moment.visible().start(),
            moment.visible().end(),
            key.clone(),
            cv,
        ) {
            tiles.push(tile);
        }
    }

    if tiles.is_empty() {
        return None;
    }

    let track = ControlTrack::new(Time::ONE, tiles)
        .ok()?
        .with_repeat(Repeat::Forever);
    Some(ControlScore::from(track))
}

fn projected_control_value(
    moment: &cadence_core::infrastructure::projection::ProjectedMoment,
    cadence_key: CadenceControlKey,
) -> Option<ControlValue> {
    let core_key = lower_control_key(cadence_key)?;
    if let Some(value) = moment.controls().get(&core_key) {
        return Some(value.clone());
    }

    projected_control_text(moment).and_then(|text| text_to_control_value(text, cadence_key))
}

fn projected_control_text(
    moment: &cadence_core::infrastructure::projection::ProjectedMoment,
) -> Option<&str> {
    match moment.intent() {
        Intent::Select(select) => Some(select.option.as_str()),
        Intent::Sample(sample) => Some(sample.sample_id.as_str()),
        _ => None,
    }
}

fn text_to_control_value(text: &str, cadence_key: CadenceControlKey) -> Option<ControlValue> {
    match cadence_key {
        CadenceControlKey::Pitch => Some(ControlValue::Scalar(parse_pitch_text(text)?)),
        CadenceControlKey::Velocity
        | CadenceControlKey::Sustain
        | CadenceControlKey::LowPassResonance
        | CadenceControlKey::HighPassResonance
        | CadenceControlKey::ReverbSend
        | CadenceControlKey::DelaySend => {
            let v: f64 = text.trim().parse().ok().filter(|x: &f64| x.is_finite())?;
            unit_value(&CadenceValueExpr::number(v)).map(ControlValue::Unipolar)
        }
        _ => {
            let v: f64 = text.trim().parse().ok().filter(|x: &f64| x.is_finite())?;
            Some(ControlValue::Scalar(v))
        }
    }
}

fn lower_control_key(key: CadenceControlKey) -> Option<ControlKey> {
    Some(match key {
        CadenceControlKey::Gate => ControlKey::Gate,
        CadenceControlKey::SampleBank => ControlKey::SampleBank,
        CadenceControlKey::SampleVariant => ControlKey::SampleVariant,
        CadenceControlKey::Pitch => ControlKey::Pitch,
        CadenceControlKey::Velocity => ControlKey::Velocity,
        CadenceControlKey::Legato => ControlKey::Legato,
        CadenceControlKey::Attack => ControlKey::Attack,
        CadenceControlKey::Decay => ControlKey::Decay,
        CadenceControlKey::Sustain => ControlKey::Sustain,
        CadenceControlKey::Release => ControlKey::Release,
        CadenceControlKey::Gain => ControlKey::Gain,
        CadenceControlKey::Pan => ControlKey::Pan,
        CadenceControlKey::PlaybackRate => ControlKey::PlaybackRate,
        CadenceControlKey::ClipLength => ControlKey::ClipLength,
        CadenceControlKey::PlaybackStart => ControlKey::PlaybackStart,
        CadenceControlKey::PlaybackEnd => ControlKey::PlaybackEnd,
        CadenceControlKey::Reverse => ControlKey::Reverse,
        CadenceControlKey::LowPassCutoff => ControlKey::LowPassCutoff,
        CadenceControlKey::LowPassResonance => ControlKey::LowPassResonance,
        CadenceControlKey::HighPassCutoff => ControlKey::HighPassCutoff,
        CadenceControlKey::HighPassResonance => ControlKey::HighPassResonance,
        CadenceControlKey::PostGain => ControlKey::PostGain,
        CadenceControlKey::ReverbSend => ControlKey::ReverbSend,
        CadenceControlKey::DelaySend => ControlKey::DelaySend,
        CadenceControlKey::Compressor => ControlKey::Compressor,
    })
}

fn lower_control_value(
    key: CadenceControlKey,
    expr: &CadenceControlValueExpr,
) -> Option<ControlValue> {
    match expr {
        CadenceControlValueExpr::Scalar { value } => match key {
            CadenceControlKey::Pitch => Some(ControlValue::Scalar(value_as_pitch(value)?)),
            CadenceControlKey::Velocity
            | CadenceControlKey::Sustain
            | CadenceControlKey::LowPassResonance
            | CadenceControlKey::HighPassResonance
            | CadenceControlKey::ReverbSend
            | CadenceControlKey::DelaySend => Some(ControlValue::Unipolar(unit_value(value)?)),
            _ => Some(ControlValue::Scalar(value_as_number(value)?)),
        },
        CadenceControlValueExpr::Choice { value } => {
            Some(ControlValue::Choice(Symbol::from(value_as_text(value)?)))
        }
        CadenceControlValueExpr::Bool { value } => Some(ControlValue::Bool(*value)),
        CadenceControlValueExpr::Reverb {
            amount,
            decay,
            damping,
        } => Some(ControlValue::Reverb(ReverbSettings::new(
            unit_value(amount)?,
            duration_from_seconds(value_as_number(decay)?)?,
            unit_value(damping)?,
        ))),
        CadenceControlValueExpr::Delay {
            amount,
            time,
            feedback,
            damping,
        } => Some(ControlValue::Delay(DelaySettings::new(
            unit_value(amount)?,
            duration_from_seconds(value_as_number(time)?)?,
            unit_value(feedback)?,
            unit_value(damping)?,
        ))),
        CadenceControlValueExpr::Compressor {
            threshold,
            ratio,
            attack,
            release,
        } => Some(ControlValue::Compressor(CompressorSettings::new(
            unit_value(threshold)?,
            value_as_number(ratio)?,
            duration_from_seconds(value_as_number(attack)?)?,
            duration_from_seconds(value_as_number(release)?)?,
        ))),
        CadenceControlValueExpr::Pattern { .. } => {
            // Pattern control values are projected to a ControlScore before this
            // function is called; they must never reach lower_control_value directly.
            unreachable!("Pattern control values must be handled before lower_control_value")
        }
    }
}

fn call_argument_maps(
    args: &[crate::domain::program::CadenceCallBinding],
    program: &CadenceProgram,
) -> (
    BTreeMap<u8, CadenceValueExpr>,
    BTreeMap<u8, CadencePatternExpr>,
) {
    let mut value_args = BTreeMap::new();
    let mut pattern_args = BTreeMap::new();

    for (index, arg) in args.iter().enumerate() {
        let slot = (index + 1) as u8;
        match &arg.arg {
            CadenceCallArg::Value { value } => {
                value_args.insert(slot, value.clone());
            }
            CadenceCallArg::Pattern { pattern } => {
                pattern_args.insert(
                    slot,
                    substitute_pattern_expr(pattern, &BTreeMap::new(), &BTreeMap::new(), program),
                );
            }
        }
    }

    (value_args, pattern_args)
}

fn substitute_pattern_expr(
    expr: &CadencePatternExpr,
    value_args: &BTreeMap<u8, CadenceValueExpr>,
    pattern_args: &BTreeMap<u8, CadencePatternExpr>,
    program: &CadenceProgram,
) -> CadencePatternExpr {
    match expr {
        CadencePatternExpr::Silence => CadencePatternExpr::Silence,
        CadencePatternExpr::Argument { slot, label } => pattern_args
            .get(slot)
            .cloned()
            .unwrap_or_else(|| CadencePatternExpr::Argument {
                slot: *slot,
                label: label.clone(),
            }),
        CadencePatternExpr::Source {
            source,
            value,
            site,
        } => CadencePatternExpr::Source {
            source: source.clone(),
            value: substitute_value_expr(value, value_args),
            site: *site,
        },
        CadencePatternExpr::Merge { branches } => CadencePatternExpr::Merge {
            branches: branches
                .iter()
                .map(|branch| substitute_pattern_expr(branch, value_args, pattern_args, program))
                .collect(),
        },
        CadencePatternExpr::CycleRoute { branches } => CadencePatternExpr::CycleRoute {
            branches: branches
                .iter()
                .map(|branch| substitute_pattern_expr(branch, value_args, pattern_args, program))
                .collect(),
        },
        CadencePatternExpr::CycleSlots { branches } => CadencePatternExpr::CycleSlots {
            branches: branches
                .iter()
                .map(|branch| substitute_pattern_expr(branch, value_args, pattern_args, program))
                .collect(),
        },
        CadencePatternExpr::ReflectCycle { input } => CadencePatternExpr::ReflectCycle {
            input: Box::new(substitute_pattern_expr(
                input,
                value_args,
                pattern_args,
                program,
            )),
        },
        CadencePatternExpr::Mask { input, by } => CadencePatternExpr::Mask {
            input: Box::new(substitute_pattern_expr(
                input,
                value_args,
                pattern_args,
                program,
            )),
            by: Box::new(substitute_pattern_expr(
                by,
                value_args,
                pattern_args,
                program,
            )),
        },
        CadencePatternExpr::Gain { input, amount } => CadencePatternExpr::Gain {
            input: Box::new(substitute_pattern_expr(
                input,
                value_args,
                pattern_args,
                program,
            )),
            amount: substitute_value_expr(amount, value_args),
        },
        CadencePatternExpr::Fast { input, factor } => CadencePatternExpr::Fast {
            input: Box::new(substitute_pattern_expr(
                input,
                value_args,
                pattern_args,
                program,
            )),
            factor: substitute_value_expr(factor, value_args),
        },
        CadencePatternExpr::Slow { input, factor } => CadencePatternExpr::Slow {
            input: Box::new(substitute_pattern_expr(
                input,
                value_args,
                pattern_args,
                program,
            )),
            factor: substitute_value_expr(factor, value_args),
        },
        CadencePatternExpr::Shift { input, offset } => CadencePatternExpr::Shift {
            input: Box::new(substitute_pattern_expr(
                input,
                value_args,
                pattern_args,
                program,
            )),
            offset: substitute_value_expr(offset, value_args),
        },
        CadencePatternExpr::Control { input, key, value } => CadencePatternExpr::Control {
            input: Box::new(substitute_pattern_expr(
                input,
                value_args,
                pattern_args,
                program,
            )),
            key: *key,
            value: match value {
                CadenceControlValueExpr::Pattern { expr } => CadenceControlValueExpr::Pattern {
                    expr: Box::new(substitute_pattern_expr(
                        expr,
                        value_args,
                        pattern_args,
                        program,
                    )),
                },
                other => substitute_control_value_expr(other, value_args),
            },
        },
        CadencePatternExpr::Call { call } => CadencePatternExpr::Call {
            call: crate::domain::program::CadenceCallExpr {
                trick_id: call.trick_id.clone(),
                trick_name: call.trick_name.clone(),
                arguments: call
                    .arguments
                    .iter()
                    .map(|binding| crate::domain::program::CadenceCallBinding {
                        param: binding.param.clone(),
                        arg: match &binding.arg {
                            CadenceCallArg::Value { value } => CadenceCallArg::Value {
                                value: substitute_value_expr(value, value_args),
                            },
                            CadenceCallArg::Pattern { pattern } => CadenceCallArg::Pattern {
                                pattern: substitute_pattern_expr(
                                    pattern,
                                    value_args,
                                    pattern_args,
                                    program,
                                ),
                            },
                        },
                    })
                    .collect(),
            },
        },
        CadencePatternExpr::State { state } => CadencePatternExpr::State {
            state: state.clone(),
        },
    }
}

fn substitute_control_value_expr(
    expr: &CadenceControlValueExpr,
    value_args: &BTreeMap<u8, CadenceValueExpr>,
) -> CadenceControlValueExpr {
    match expr {
        CadenceControlValueExpr::Scalar { value } => CadenceControlValueExpr::Scalar {
            value: substitute_value_expr(value, value_args),
        },
        CadenceControlValueExpr::Choice { value } => CadenceControlValueExpr::Choice {
            value: substitute_value_expr(value, value_args),
        },
        CadenceControlValueExpr::Bool { value } => CadenceControlValueExpr::Bool { value: *value },
        CadenceControlValueExpr::Reverb {
            amount,
            decay,
            damping,
        } => CadenceControlValueExpr::Reverb {
            amount: substitute_value_expr(amount, value_args),
            decay: substitute_value_expr(decay, value_args),
            damping: substitute_value_expr(damping, value_args),
        },
        CadenceControlValueExpr::Delay {
            amount,
            time,
            feedback,
            damping,
        } => CadenceControlValueExpr::Delay {
            amount: substitute_value_expr(amount, value_args),
            time: substitute_value_expr(time, value_args),
            feedback: substitute_value_expr(feedback, value_args),
            damping: substitute_value_expr(damping, value_args),
        },
        CadenceControlValueExpr::Compressor {
            threshold,
            ratio,
            attack,
            release,
        } => CadenceControlValueExpr::Compressor {
            threshold: substitute_value_expr(threshold, value_args),
            ratio: substitute_value_expr(ratio, value_args),
            attack: substitute_value_expr(attack, value_args),
            release: substitute_value_expr(release, value_args),
        },
        CadenceControlValueExpr::Pattern { expr } => {
            // Pattern exprs are substituted at the Control arm level in
            // substitute_pattern_expr, but handle here for completeness.
            CadenceControlValueExpr::Pattern { expr: expr.clone() }
        }
    }
}

fn substitute_value_expr(
    expr: &CadenceValueExpr,
    value_args: &BTreeMap<u8, CadenceValueExpr>,
) -> CadenceValueExpr {
    match expr {
        CadenceValueExpr::Argument { slot, label } => {
            value_args
                .get(slot)
                .cloned()
                .unwrap_or_else(|| CadenceValueExpr::Argument {
                    slot: *slot,
                    label: label.clone(),
                })
        }
        _ => expr.clone(),
    }
}

fn source_score(intent: Intent, site: GridPos) -> Option<Score> {
    let voice = Voice::new(
        Time::ONE,
        vec![Tile::spanning(Time::ZERO, Time::ONE, intent)?.with_id(encode_site_id(site))],
    )?
    .with_repeat(Repeat::Forever);
    Some(Score::from(voice))
}

fn value_as_number(expr: &CadenceValueExpr) -> Option<f64> {
    match expr {
        CadenceValueExpr::Number { value } => Some(*value),
        CadenceValueExpr::Text { value } => value.trim().parse().ok(),
        CadenceValueExpr::Bundle { .. } => None,
        _ => None,
    }
}

fn value_as_pitch(expr: &CadenceValueExpr) -> Option<f64> {
    match expr {
        CadenceValueExpr::Number { value } => Some(*value),
        CadenceValueExpr::Text { value } => parse_pitch_text(value),
        CadenceValueExpr::Bundle { .. } => None,
        _ => None,
    }
}

fn value_as_text(expr: &CadenceValueExpr) -> Option<String> {
    match expr {
        CadenceValueExpr::Text { value } => Some(value.clone()),
        CadenceValueExpr::SynthSource { source } => Some(source.as_str().to_string()),
        CadenceValueExpr::Number { value } => Some({
            if value.fract() == 0.0 {
                (*value as i64).to_string()
            } else {
                value.to_string()
            }
        }),
        CadenceValueExpr::Bool { value } => Some(value.to_string()),
        CadenceValueExpr::Bundle { .. } => None,
        CadenceValueExpr::Argument { .. } => None,
    }
}

fn lower_synth_source(source: CadenceSynthSource) -> BuiltInSynthSource {
    match source {
        CadenceSynthSource::Sine => BuiltInSynthSource::Sine,
        CadenceSynthSource::Triangle => BuiltInSynthSource::Triangle,
        CadenceSynthSource::Saw => BuiltInSynthSource::Saw,
        CadenceSynthSource::Square => BuiltInSynthSource::Square,
    }
}

fn unit_value(expr: &CadenceValueExpr) -> Option<UnitValue> {
    UnitValue::new(value_as_number(expr)?)
}

fn duration_from_seconds(seconds: f64) -> Option<Duration> {
    (seconds.is_finite() && seconds > 0.0).then(|| Duration::from_secs_f64(seconds))
}

fn number_to_time(value: f64) -> Option<Time> {
    if !(value.is_finite() && value > 0.0) {
        return None;
    }
    const SCALE: i64 = 1_000;
    Some(Time::new((value * SCALE as f64).round() as i64, SCALE))
}

fn one_cycle_window() -> TransportSpan {
    TransportSpan::new(Time::ZERO, Time::ONE).expect("one cycle window")
}

fn rational_time(value: Time) -> RationalTime {
    RationalTime {
        numerator: value.numerator(),
        denominator: value.denominator(),
    }
}

fn encode_site_id(site: GridPos) -> u64 {
    let col = site.col as u32 as u64;
    let row = site.row as u32 as u64;
    (col << 32) | row
}

fn decode_site_id(value: u64) -> Option<GridPos> {
    let col = (value >> 32) as u32 as i32;
    let row = (value & 0xffff_ffff) as u32 as i32;
    Some(GridPos { col, row })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::common::PortType;
    use crate::domain::program::{
        CadenceCallArg, CadenceCallBinding, CadenceCallExpr, CadenceControlValueExpr,
        CadenceFunction, CadenceOutput, CadenceProgram, CadenceSourceKind,
        CadenceSubgraphSignature, CadenceValueExpr,
    };
    use cadence_core::infrastructure::score::ControlKey;

    fn projected_output(
        program: &CadenceProgram,
    ) -> cadence_core::infrastructure::projection::ProjectedMosaic {
        let outputs = program_output_scores(program);
        RendererCore::new(outputs[0].1.clone())
            .projected_output(&one_cycle_window())
            .expect("score should project")
    }

    #[test]
    fn simple_program_projects_one_cycle_deterministically() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 1, row: 0 },
                expr: CadencePatternExpr::Fast {
                    input: Box::new(CadencePatternExpr::Source {
                        source: CadenceSourceKind::Sample,
                        value: CadenceValueExpr::text("bd"),
                        site: None,
                    }),
                    factor: CadenceValueExpr::number(2.0),
                },
            }],
            functions: Vec::new(),
        };

        let first = preview_events_for_scores(program_output_scores(&program).as_slice());
        let second = preview_events_for_scores(program_output_scores(&program).as_slice());

        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|event| event.label == "bd"));
    }

    #[test]
    fn cycle_slots_preserve_silent_branch_positions() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 0, row: 0 },
                expr: CadencePatternExpr::CycleSlots {
                    branches: vec![
                        CadencePatternExpr::Source {
                            source: CadenceSourceKind::Sample,
                            value: CadenceValueExpr::text("bd"),
                            site: None,
                        },
                        CadencePatternExpr::Silence,
                        CadencePatternExpr::Source {
                            source: CadenceSourceKind::Sample,
                            value: CadenceValueExpr::text("cp"),
                            site: None,
                        },
                        CadencePatternExpr::Silence,
                    ],
                },
            }],
            functions: Vec::new(),
        };

        let events = preview_events_for_scores(program_output_scores(&program).as_slice());

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].label, "bd");
        assert_eq!(events[0].start_normalized, 0.0);
        assert_eq!(events[0].end_normalized, 0.25);
        assert_eq!(events[1].label, "cp");
        assert_eq!(events[1].start_normalized, 0.5);
        assert_eq!(events[1].end_normalized, 0.75);
    }

    #[test]
    fn gain_lowering_attaches_controls_to_projected_output() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 0, row: 0 },
                expr: CadencePatternExpr::Gain {
                    input: Box::new(CadencePatternExpr::Source {
                        source: CadenceSourceKind::Sample,
                        value: CadenceValueExpr::text("bd"),
                        site: None,
                    }),
                    amount: CadenceValueExpr::number(0.5),
                },
            }],
            functions: Vec::new(),
        };

        let outputs = program_output_scores(&program);
        let projected = RendererCore::new(outputs[0].1.clone())
            .projected_output(&one_cycle_window())
            .expect("score should project");

        assert!(matches!(
            projected.moments()[0].controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.5).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn gain_control_patterns_drive_runtime_gain_controls() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 0, row: 0 },
                expr: CadencePatternExpr::Control {
                    input: Box::new(CadencePatternExpr::Source {
                        source: CadenceSourceKind::Sample,
                        value: CadenceValueExpr::text("bd"),
                        site: None,
                    }),
                    key: CadenceControlKey::Gain,
                    value: CadenceControlValueExpr::Pattern {
                        expr: Box::new(CadencePatternExpr::CycleSlots {
                            branches: vec![
                                CadencePatternExpr::Source {
                                    source: CadenceSourceKind::Pulse,
                                    value: CadenceValueExpr::text("0.5"),
                                    site: None,
                                },
                                CadencePatternExpr::Source {
                                    source: CadenceSourceKind::Pulse,
                                    value: CadenceValueExpr::text("0.75"),
                                    site: None,
                                },
                            ],
                        }),
                    },
                },
            }],
            functions: Vec::new(),
        };

        assert_eq!(
            program.render_debug(),
            vec!["gain(pattern(polymeter(pulse(\"0.5\"), pulse(\"0.75\"))), sample(\"bd\"))"]
        );

        let projected = projected_output(&program);
        assert!(matches!(
            projected.moments()[0].controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.5).abs() < f64::EPSILON
        ));
        assert!(matches!(
            projected.moments()[1].controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.75).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn pitch_control_patterns_drive_runtime_pitch_controls() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 0, row: 0 },
                expr: CadencePatternExpr::Control {
                    input: Box::new(CadencePatternExpr::CycleSlots {
                        branches: vec![
                            CadencePatternExpr::Source {
                                source: CadenceSourceKind::Sample,
                                value: CadenceValueExpr::text("bd"),
                                site: None,
                            },
                            CadencePatternExpr::Source {
                                source: CadenceSourceKind::Sample,
                                value: CadenceValueExpr::text("bd"),
                                site: None,
                            },
                        ],
                    }),
                    key: CadenceControlKey::Pitch,
                    value: CadenceControlValueExpr::Pattern {
                        expr: Box::new(CadencePatternExpr::CycleSlots {
                            branches: vec![
                                CadencePatternExpr::Source {
                                    source: CadenceSourceKind::Pulse,
                                    value: CadenceValueExpr::text("c3"),
                                    site: None,
                                },
                                CadencePatternExpr::Source {
                                    source: CadenceSourceKind::Pulse,
                                    value: CadenceValueExpr::text("e3"),
                                    site: None,
                                },
                            ],
                        }),
                    },
                },
            }],
            functions: Vec::new(),
        };

        let projected = projected_output(&program);
        assert!(matches!(
            projected.moments()[0].controls().get(&ControlKey::Pitch),
            Some(ControlValue::Scalar(value)) if (*value - parse_pitch_text("c3").unwrap()).abs() < f64::EPSILON
        ));
        assert!(matches!(
            projected.moments()[1].controls().get(&ControlKey::Pitch),
            Some(ControlValue::Scalar(value)) if (*value - parse_pitch_text("e3").unwrap()).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn lowpass_control_patterns_drive_runtime_filter_controls() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 0, row: 0 },
                expr: CadencePatternExpr::Control {
                    input: Box::new(CadencePatternExpr::CycleSlots {
                        branches: vec![
                            CadencePatternExpr::Source {
                                source: CadenceSourceKind::Sample,
                                value: CadenceValueExpr::text("bd"),
                                site: None,
                            },
                            CadencePatternExpr::Source {
                                source: CadenceSourceKind::Sample,
                                value: CadenceValueExpr::text("bd"),
                                site: None,
                            },
                        ],
                    }),
                    key: CadenceControlKey::LowPassCutoff,
                    value: CadenceControlValueExpr::Pattern {
                        expr: Box::new(CadencePatternExpr::CycleSlots {
                            branches: vec![
                                CadencePatternExpr::Source {
                                    source: CadenceSourceKind::Pulse,
                                    value: CadenceValueExpr::text("220"),
                                    site: None,
                                },
                                CadencePatternExpr::Source {
                                    source: CadenceSourceKind::Pulse,
                                    value: CadenceValueExpr::text("1400"),
                                    site: None,
                                },
                            ],
                        }),
                    },
                },
            }],
            functions: Vec::new(),
        };

        let projected = projected_output(&program);
        assert!(matches!(
            projected.moments()[0]
                .controls()
                .get(&ControlKey::LowPassCutoff),
            Some(ControlValue::Scalar(value)) if (*value - 220.0).abs() < f64::EPSILON
        ));
        assert!(matches!(
            projected.moments()[1]
                .controls()
                .get(&ControlKey::LowPassCutoff),
            Some(ControlValue::Scalar(value)) if (*value - 1400.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn trick_call_substitutes_arguments_before_runtime_lowering() {
        let function = CadenceFunction {
            trick_id: "groove".into(),
            trick_name: "groove".into(),
            signature: CadenceSubgraphSignature {
                inputs: Vec::new(),
                output_pos: GridPos { col: 1, row: 0 },
                output_type: Some(PortType::new("pattern")),
            },
            body: CadencePatternExpr::Fast {
                input: Box::new(CadencePatternExpr::Argument {
                    slot: 1,
                    label: "pattern".into(),
                }),
                factor: CadenceValueExpr::number(2.0),
            },
        };
        let program = CadenceProgram {
            functions: vec![function],
            outputs: vec![CadenceOutput {
                root: GridPos { col: 2, row: 0 },
                expr: CadencePatternExpr::Call {
                    call: CadenceCallExpr {
                        trick_id: "groove".into(),
                        trick_name: "groove".into(),
                        arguments: vec![CadenceCallBinding {
                            param: "pattern".into(),
                            arg: CadenceCallArg::Pattern {
                                pattern: CadencePatternExpr::Source {
                                    source: CadenceSourceKind::Sample,
                                    value: CadenceValueExpr::text("bd"),
                                    site: None,
                                },
                            },
                        }],
                    },
                },
            }],
        };

        let events = preview_events_for_scores(program_output_scores(&program).as_slice());
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|event| event.output_lane == GridPos { col: 2, row: 0 }.into())
        );
    }
}
