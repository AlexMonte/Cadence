use crate::application::compile_context::{CompileContext, ProvenanceFrame};
use crate::domain::{
    AtomExpr, AtomExprKind, AtomModifier, ContainerAxis, ContainerKind, CycleDuration, CycleSpan,
    CycleTime, Diagnostic, EventValue, MusicalValue, NormalizedContainer, NormalizedProgram,
    PatternEvent, PatternNodeIr, PatternStream, Point3Ir, Rational, TimedPatternIr,
    WeightedPatternIr,
};

fn unit_span() -> CycleSpan {
    CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()))
}

/// A bounded preview evaluates the same recursive tree exported to the audio host.
pub fn compile_container(
    program: &NormalizedProgram,
    container: &NormalizedContainer,
    span: CycleSpan,
    ctx: &mut CompileContext,
) -> Result<PatternStream, Vec<Diagnostic>> {
    let node = compile_container_ir(program, container, ctx)?;
    let origin = Rational::from_integer(ctx.cycle_index as i64);
    Ok(PatternStream::new(
        node.query(unit_span().shift_by(CycleDuration(origin)))
            .events
            .into_iter()
            .map(|mut event| {
                event.span = CycleSpan::new(
                    CycleTime(span.start.0 + (event.span.start.0 - origin) * span.duration.0),
                    CycleDuration(event.span.duration.0 * span.duration.0),
                );
                event
            })
            .collect(),
    ))
}

pub(crate) fn compile_container_ir(
    program: &NormalizedProgram,
    container: &NormalizedContainer,
    ctx: &mut CompileContext,
) -> Result<PatternNodeIr, Vec<Diagnostic>> {
    if ctx.provenance_stack.len() >= 128
        || ctx
            .provenance_stack
            .iter()
            .any(|frame| frame.container.as_ref() == Some(&container.id))
    {
        return Err(vec![Diagnostic::new(
            crate::domain::DiagnosticCategory::Compile,
            crate::domain::DiagnosticKind::CompileFailed,
            "Container nesting must be acyclic and at most 128 levels deep.",
            Some(crate::domain::DiagnosticLocation::ContainerStack {
                container: container.id.clone(),
                index: 0,
            }),
        )]);
    }
    ctx.with_provenance_frame(
        ProvenanceFrame {
            node: None,
            container: Some(container.id.clone()),
            stack_index: None,
        },
        |ctx| {
            let mut children = Vec::new();
            for (index, expr) in container.exprs.iter().enumerate() {
                if let Some(frame) = ctx.provenance_stack.last_mut() {
                    frame.stack_index = Some(index);
                }
                let child = place_on_axis(
                    compile_expr_ir(program, expr, ctx)?,
                    container.axis,
                    index,
                    container.exprs.len(),
                );
                let weight = if container.kind == ContainerKind::Arrangement {
                    arrangement_duration(expr).map_err(|message| {
                        vec![Diagnostic::new(
                            crate::domain::DiagnosticCategory::Compile,
                            crate::domain::DiagnosticKind::CompileFailed,
                            message,
                            Some(crate::domain::DiagnosticLocation::ContainerStack {
                                container: container.id.clone(),
                                index,
                            }),
                        )]
                    })?
                } else {
                    sequence_weight(expr)
                };
                children.push(WeightedPatternIr::new(weight, child));
            }
            Ok(match container.kind {
                ContainerKind::Sequence => PatternNodeIr::sequence(children),
                ContainerKind::Arrangement => {
                    let segments: Vec<_> = children
                        .into_iter()
                        .map(|child| TimedPatternIr::new(child.weight, 1, *child.node))
                        .collect();
                    TimedPatternIr::total_duration(&segments).map_err(|message| {
                        vec![Diagnostic::new(
                            crate::domain::DiagnosticCategory::Compile,
                            crate::domain::DiagnosticKind::CompileFailed,
                            message,
                            Some(crate::domain::DiagnosticLocation::ContainerStack {
                                container: container.id.clone(),
                                index: 0,
                            }),
                        )]
                    })?;
                    PatternNodeIr::arrange(segments)
                }
                ContainerKind::Alternate => PatternNodeIr::cycle_route(
                    children.into_iter().map(|child| *child.node).collect(),
                ),
                ContainerKind::Layer => {
                    PatternNodeIr::merge(children.into_iter().map(|child| *child.node).collect())
                }
            })
        },
    )
}

fn place_on_axis(
    node: PatternNodeIr,
    axis: ContainerAxis,
    index: usize,
    count: usize,
) -> PatternNodeIr {
    let cell = Rational::new(2 * index as i64 - (count as i64 - 1), 2);
    let zero = Rational::zero();
    let offset = match axis {
        ContainerAxis::Time => return node,
        ContainerAxis::X => Point3Ir::new(cell, zero, zero),
        ContainerAxis::Y => Point3Ir::new(zero, cell, zero),
        ContainerAxis::Z => Point3Ir::new(zero, zero, cell),
    };
    PatternNodeIr::space_shift(node, offset)
}

fn compile_expr_ir(
    program: &NormalizedProgram,
    expr: &AtomExpr,
    ctx: &mut CompileContext,
) -> Result<PatternNodeIr, Vec<Diagnostic>> {
    let base = match &expr.kind {
        AtomExprKind::Value(MusicalValue::NestedContainer(id)) => {
            let Some(container) = program.containers.get(id) else {
                return Err(vec![Diagnostic::new(
                    crate::domain::DiagnosticCategory::Placement,
                    crate::domain::DiagnosticKind::MissingContainer,
                    "Nested container is missing.",
                    Some(crate::domain::DiagnosticLocation::ContainerStack {
                        container: id.clone(),
                        index: 0,
                    }),
                )]);
            };
            compile_container_ir(program, container, ctx)?
        }
        AtomExprKind::Value(MusicalValue::Effect(value)) => {
            let (key, value) = value.control();
            PatternNodeIr::control_stream(crate::domain::ControlStream::new(vec![
                crate::domain::ControlEvent::new(unit_span(), key, value),
            ]))
        }
        AtomExprKind::Value(value) => {
            let value = match value {
                MusicalValue::Note(note) => EventValue::Note {
                    value: note.pitch_label(),
                    octave: note.octave,
                },
                MusicalValue::Sound(value) => EventValue::Sound {
                    value: value.clone(),
                },
                MusicalValue::Rest => EventValue::Rest,
                MusicalValue::Scalar(value) => EventValue::Scalar { value: value.value },
                MusicalValue::NestedContainer(_) | MusicalValue::Effect(_) => unreachable!(),
            };
            PatternNodeIr::cycle_event_stream(PatternStream::new(vec![
                PatternEvent::new(unit_span(), value).with_source({
                    let mut source = ctx.current_provenance().unwrap_or_default();
                    if let Some(node) = &expr.source_node {
                        source.node = Some(node.clone());
                    }
                    Some(source)
                }),
            ]))
        }
        AtomExprKind::Choice(branches) => PatternNodeIr::cycle_route(
            branches
                .iter()
                .map(|branch| compile_expr_ir(program, branch, ctx))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        AtomExprKind::Parallel(branches) => PatternNodeIr::merge(
            branches
                .iter()
                .map(|branch| compile_expr_ir(program, branch, ctx))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    expr.modifiers.iter().try_fold(base, |node, modifier| {
        Ok(match modifier {
            AtomModifier::Scale(scale) => apply_scale(node, *scale)?,
            AtomModifier::EuclidPattern(pattern) => {
                // Normalization validates every cycle and bounds the combined repeat period.
                PatternNodeIr::cycle_route(
                    (0..pattern.period().expect("validated rhythm"))
                        .map(|cycle| {
                            let (pulses, steps, rotation) = pattern.at_cycle(cycle);
                            apply_euclid(node.clone(), pulses, steps, rotation)
                        })
                        .collect(),
                )
            }
            AtomModifier::Modulation { parameter, value } => {
                let value = crate::domain::FieldValue::Modulation { value: *value };
                let field = match parameter {
                    crate::domain::ParameterKey::Gain => crate::domain::EventField::Gain(value),
                    crate::domain::ParameterKey::Velocity => {
                        crate::domain::EventField::Velocity(value)
                    }
                    crate::domain::ParameterKey::PlaybackRate => {
                        crate::domain::EventField::PlaybackRate(value)
                    }
                    crate::domain::ParameterKey::LowPassCutoff => {
                        crate::domain::EventField::LowPassCutoff(value)
                    }
                    crate::domain::ParameterKey::Transpose => {
                        crate::domain::EventField::Transpose(value)
                    }
                    _ => unreachable!("validated modulation lane"),
                };
                apply_note_field(node, field)
            }
            AtomModifier::Rev => PatternNodeIr::reflect_cycle(node),
            AtomModifier::Late(offset) => {
                PatternNodeIr::shift(node, crate::domain::CycleDuration(*offset))
            }
            AtomModifier::Fast(factor) => {
                PatternNodeIr::time_scale(node, Rational::one() / *factor)
            }
            AtomModifier::Slow(factor) => PatternNodeIr::time_scale(node, *factor),
            AtomModifier::Gain(value) => apply_note_field(
                node,
                crate::domain::EventField::Gain(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Attack(value) => apply_note_field(
                node,
                crate::domain::EventField::Attack(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Decay(value) => apply_note_field(
                node,
                crate::domain::EventField::Decay(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Release(value) => apply_note_field(
                node,
                crate::domain::EventField::Release(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Transpose(value) => apply_note_field(
                node,
                crate::domain::EventField::Transpose(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Pan(value) => apply_note_field(
                node,
                crate::domain::EventField::Pan(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::HighPassCutoff(value) => apply_note_field(
                node,
                crate::domain::EventField::HighPassCutoff(crate::domain::FieldValue::rational(
                    *value,
                )),
            ),
            AtomModifier::HighPassResonance(value) => apply_note_field(
                node,
                crate::domain::EventField::HighPassResonance(crate::domain::FieldValue::rational(
                    *value,
                )),
            ),
            AtomModifier::Velocity(value) => apply_note_field(
                node,
                crate::domain::EventField::Velocity(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::ClipLength(value) => apply_note_field(
                node,
                crate::domain::EventField::ClipLength(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::PostGain(value) => apply_note_field(
                node,
                crate::domain::EventField::PostGain(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::PitchBend(value) => apply_note_field(
                node,
                crate::domain::EventField::PitchBend(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Expression(value) => apply_note_field(
                node,
                crate::domain::EventField::Expression(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Delay(value) => apply_note_field(
                node,
                crate::domain::EventField::DelaySend(crate::domain::FieldValue::Delay {
                    value: *value,
                }),
            ),
            AtomModifier::Reverb(value) => apply_note_field(
                node,
                crate::domain::EventField::ReverbSend(crate::domain::FieldValue::Reverb {
                    value: *value,
                }),
            ),
            AtomModifier::Compressor(value) => apply_note_field(
                node,
                crate::domain::EventField::Compressor(crate::domain::FieldValue::Compressor {
                    value: *value,
                }),
            ),
            AtomModifier::Gate(value) => apply_note_field(
                node,
                crate::domain::EventField::Gate(crate::domain::FieldValue::bool(*value)),
            ),
            AtomModifier::Legato(value) => apply_note_field(
                node,
                crate::domain::EventField::Legato(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Sustain(value) => apply_note_field(
                node,
                crate::domain::EventField::Sustain(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::LowPassCutoff(value) => apply_note_field(
                node,
                crate::domain::EventField::LowPassCutoff(crate::domain::FieldValue::rational(
                    *value,
                )),
            ),
            AtomModifier::LowPassResonance(value) => apply_note_field(
                node,
                crate::domain::EventField::LowPassResonance(crate::domain::FieldValue::rational(
                    *value,
                )),
            ),
            AtomModifier::SampleBank(value) => apply_note_field(
                node,
                crate::domain::EventField::SampleBank(crate::domain::FieldValue::symbol(value)),
            ),
            AtomModifier::SampleVariant(value) => apply_note_field(
                node,
                crate::domain::EventField::SampleVariant(crate::domain::FieldValue::rational(
                    Rational::from_integer(i64::from(*value)),
                )),
            ),
            AtomModifier::PlaybackRate(value) => apply_note_field(
                node,
                crate::domain::EventField::PlaybackRate(crate::domain::FieldValue::rational(
                    *value,
                )),
            ),
            AtomModifier::PlaybackStart(value) => apply_note_field(
                node,
                crate::domain::EventField::PlaybackStart(crate::domain::FieldValue::rational(
                    *value,
                )),
            ),
            AtomModifier::PlaybackEnd(value) => apply_note_field(
                node,
                crate::domain::EventField::PlaybackEnd(crate::domain::FieldValue::rational(*value)),
            ),
            AtomModifier::Reverse(value) => apply_note_field(
                node,
                crate::domain::EventField::Reverse(crate::domain::FieldValue::bool(*value)),
            ),
            AtomModifier::Fit(value) => apply_note_field(
                node,
                crate::domain::EventField::Fit(crate::domain::FieldValue::bool(*value)),
            ),
            AtomModifier::Loop(value) => apply_note_field(
                node,
                crate::domain::EventField::Loop(crate::domain::FieldValue::bool(*value)),
            ),
            AtomModifier::Slice { index, count } => apply_note_field(
                node,
                crate::domain::EventField::Slice(crate::domain::FieldValue::Slice {
                    index: *index,
                    count: *count,
                }),
            ),
            AtomModifier::Elongate(_) => node,
            AtomModifier::Replicate(count) => {
                PatternNodeIr::cycle_slots(vec![node; *count as usize])
            }
            AtomModifier::Degrade(probability) => PatternNodeIr::degrade(
                node,
                Rational::one() - probability.unwrap_or(Rational::new(1, 2)),
                0,
            ),
            AtomModifier::Euclid { pulses, steps } => apply_euclid(node, *pulses, *steps, 0),
            AtomModifier::EuclidRot {
                pulses,
                steps,
                rotation,
            } => apply_euclid(node, *pulses, *steps, *rotation),
        })
    })
}

fn apply_scale(
    node: PatternNodeIr,
    scale: crate::domain::ScaleParameters,
) -> Result<PatternNodeIr, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mapped = node.map_event_leaves(&mut |stream, periodic| {
        let mut stream = stream.clone();
        for event in &mut stream.events {
            if let EventValue::Scalar { value } = event.value {
                match scale.note(value) {
                    Ok(note) => {
                        event.value = EventValue::Note {
                            value: note.pitch_label(),
                            octave: note.octave,
                        }
                    }
                    Err(message) => diagnostics.push(Diagnostic::new(
                        crate::domain::DiagnosticCategory::Compile,
                        crate::domain::DiagnosticKind::InvalidModifierArgument,
                        message,
                        None,
                    )),
                }
            }
        }
        if periodic {
            PatternNodeIr::cycle_event_stream(stream)
        } else {
            PatternNodeIr::event_stream(stream)
        }
    });
    if diagnostics.is_empty() {
        Ok(mapped)
    } else {
        Err(diagnostics)
    }
}

fn apply_note_field(node: PatternNodeIr, field: crate::domain::EventField) -> PatternNodeIr {
    node.map_event_leaves(&mut |stream, periodic| {
        let mut stream = stream.clone();
        for event in &mut stream.events {
            event.fields.push(field.clone());
        }
        if periodic {
            PatternNodeIr::cycle_event_stream(stream)
        } else {
            PatternNodeIr::event_stream(stream)
        }
    })
}

fn apply_euclid(node: PatternNodeIr, pulses: u32, steps: u32, rotation: i32) -> PatternNodeIr {
    let mut pattern = (0..steps)
        .map(|step| (step * pulses) % steps < pulses)
        .collect::<Vec<_>>();
    pattern.rotate_right(rotation.rem_euclid(steps as i32) as usize);
    PatternNodeIr::cycle_slots(
        pattern
            .into_iter()
            .map(|active| {
                if active {
                    node.clone()
                } else {
                    PatternNodeIr::cycle_event_stream(PatternStream::default())
                }
            })
            .collect(),
    )
}

// Replication changes the child pattern, never the time assigned to a section.
fn arrangement_duration(expr: &AtomExpr) -> Result<Rational, &'static str> {
    expr.modifiers
        .iter()
        .try_fold(Rational::one(), |duration, modifier| {
            let AtomModifier::Elongate(value) = modifier else {
                return Ok(duration);
            };
            // Each operand passed normalization, but their product may exceed the
            // editor's exact-time range. Reduce in a wider integer before narrowing,
            // so reciprocal duration settings still cancel without an overflow.
            let numerator = i128::from(duration.numerator) * i128::from(value.numerator);
            let denominator = i128::from(duration.denominator) * i128::from(value.denominator);
            let (mut a, mut b) = (numerator, denominator);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            let invalid = "Arrangement duration product exceeds the supported rational range.";
            let numerator = i64::try_from(numerator / a).map_err(|_| invalid)?;
            let denominator = i64::try_from(denominator / a).map_err(|_| invalid)?;
            Ok(Rational::new(numerator, denominator))
        })
}

fn sequence_weight(expr: &AtomExpr) -> Rational {
    expr.modifiers
        .iter()
        .fold(Rational::one(), |weight, modifier| match modifier {
            AtomModifier::Elongate(value) => weight * *value,
            AtomModifier::Replicate(count) => weight * Rational::from_integer(i64::from(*count)),
            _ => weight,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::compile_context::CompileContext;
    use crate::domain::{
        ContainerAxis, ContainerId, ContainerKind, NormalizedContainer, NormalizedProgram,
    };

    #[test]
    fn compile_container_leaves_provenance_stack_empty_on_error() {
        let mut ctx = CompileContext::default();
        let program = NormalizedProgram {
            containers: Default::default(),
            ..Default::default()
        };
        let container = NormalizedContainer {
            id: ContainerId::new("parent"),
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            exprs: vec![crate::domain::AtomExpr {
                source_node: None,
                kind: crate::domain::AtomExprKind::Value(
                    crate::domain::MusicalValue::NestedContainer(ContainerId::new("missing")),
                ),
                modifiers: Vec::new(),
            }],
        };
        let span = CycleSpan {
            start: CycleTime(Rational::zero()),
            duration: CycleDuration(Rational::one()),
        };
        let result = compile_container(&program, &container, span, &mut ctx);
        assert!(result.is_err());
        assert!(ctx.provenance_stack.is_empty());
    }
}
