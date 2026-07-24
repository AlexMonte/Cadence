use crate::application::compile_context::{CompileContext, ProvenanceFrame};
use crate::domain::{
    AtomExprKind, AtomModifier, AxisIr, ContainerAxis, CycleDuration, CycleSpan, CycleTime,
    Diagnostic, EventValue, MusicalValue, NormalizedContainer, NormalizedProgram, PatternEvent,
    PatternProvenance, PatternStream, Point3Ir, Rational, SpatialMotionIr,
};

/// Places each container child at successive lattice cells along a spatial
/// axis, centred on the origin. `Time` is a no-op so purely temporal containers
/// keep their existing behaviour.
fn place_on_axis(
    stream: PatternStream,
    axis: ContainerAxis,
    index: usize,
    count: usize,
) -> PatternStream {
    let spatial_axis = match axis {
        ContainerAxis::Time => return stream,
        ContainerAxis::X => AxisIr::X,
        ContainerAxis::Y => AxisIr::Y,
        ContainerAxis::Z => AxisIr::Z,
    };
    // Centred cell index: (2*index - (count - 1)) / 2 keeps the spread exact and
    // symmetric about the origin (for example -1, 0, 1 for three children).
    let cell = Rational::new(2 * index as i64 - (count as i64 - 1), 2);
    let zero = Rational::zero();
    let offset = match spatial_axis {
        AxisIr::X => Point3Ir::new(cell, zero, zero),
        AxisIr::Y => Point3Ir::new(zero, cell, zero),
        AxisIr::Z => Point3Ir::new(zero, zero, cell),
    };
    stream.space_shift(offset)
}

pub fn compile_container(
    program: &NormalizedProgram,
    container: &NormalizedContainer,
    span: CycleSpan,
    ctx: &mut CompileContext,
) -> Result<PatternStream, Vec<Diagnostic>> {
    if container.exprs.is_empty() {
        return Ok(PatternStream::default());
    }

    ctx.with_provenance_frame(
        ProvenanceFrame {
            node: None,
            container: Some(container.id.clone()),
            stack_index: None,
        },
        |ctx| {
            let axis = container.axis;
            let child_count = container.exprs.len();
            let stream = match container.kind {
                crate::domain::ContainerKind::Sequence => {
                    let weights = container
                        .exprs
                        .iter()
                        .map(sequence_weight)
                        .collect::<Vec<_>>();
                    let total_weight = weights
                        .iter()
                        .copied()
                        .fold(Rational::zero(), |sum, weight| sum + weight);
                    let unit = span.duration.0 / total_weight;
                    let mut events = Vec::new();
                    let mut cursor = span.start.0;
                    for (stack_index, (expr, weight)) in
                        container.exprs.iter().zip(weights).enumerate()
                    {
                        if let Some(frame) = ctx.provenance_stack.last_mut() {
                            frame.stack_index = Some(stack_index);
                        }
                        let child_span = CycleSpan {
                            start: CycleTime(cursor),
                            duration: CycleDuration(unit * weight),
                        };
                        let child = place_on_axis(
                            compile_expr(program, expr, child_span, ctx)?,
                            axis,
                            stack_index,
                            child_count,
                        );
                        events.extend(child.events);
                        cursor = cursor + (unit * weight);
                    }
                    PatternStream { events }
                }
                crate::domain::ContainerKind::Alternate => {
                    let index = ctx.cycle_index % container.exprs.len();
                    place_on_axis(
                        compile_expr(program, &container.exprs[index], span, ctx)?,
                        axis,
                        index,
                        child_count,
                    )
                }
                crate::domain::ContainerKind::Layer => {
                    let mut events = Vec::new();
                    for (stack_index, expr) in container.exprs.iter().enumerate() {
                        if let Some(frame) = ctx.provenance_stack.last_mut() {
                            frame.stack_index = Some(stack_index);
                        }
                        let child = place_on_axis(
                            compile_expr(program, expr, span, ctx)?,
                            axis,
                            stack_index,
                            child_count,
                        );
                        events.extend(child.events);
                    }
                    PatternStream { events }
                }
            };
            Ok(stream)
        },
    )
}

fn compile_expr(
    program: &NormalizedProgram,
    expr: &crate::domain::AtomExpr,
    span: CycleSpan,
    ctx: &mut CompileContext,
) -> Result<PatternStream, Vec<Diagnostic>> {
    let base = match &expr.kind {
        AtomExprKind::Value(value) => compile_value(program, value, span, ctx)?,
        AtomExprKind::Choice(branches) => {
            let selected = &branches[ctx.cycle_index % branches.len()];
            compile_expr(program, selected, span, ctx)?
        }
        AtomExprKind::Parallel(branches) => {
            let mut streams = Vec::new();
            for branch in branches {
                streams.push(compile_expr(program, branch, span, ctx)?);
            }
            PatternStream::layer(streams)
        }
    };
    Ok(stamp_stream(
        apply_atom_modifiers(base, &expr.modifiers),
        ctx.current_provenance(),
    ))
}

fn compile_value(
    program: &NormalizedProgram,
    value: &MusicalValue,
    span: CycleSpan,
    ctx: &mut CompileContext,
) -> Result<PatternStream, Vec<Diagnostic>> {
    let source = ctx.current_provenance();
    Ok(match value {
        MusicalValue::Note(note) => PatternStream {
            events: vec![PatternEvent {
                span,
                value: EventValue::Note {
                    value: note.value.to_string(),
                    octave: note.octave,
                },
                fields: Vec::new(),
                position: SpatialMotionIr::origin(),
                source: source.clone(),
            }],
        },
        MusicalValue::Rest => PatternStream {
            events: vec![PatternEvent {
                span,
                value: EventValue::Rest,
                fields: Vec::new(),
                position: SpatialMotionIr::origin(),
                source: source.clone(),
            }],
        },
        MusicalValue::Scalar(scalar) => PatternStream {
            events: vec![PatternEvent {
                span,
                value: EventValue::Scalar {
                    value: scalar.value,
                },
                fields: Vec::new(),
                position: SpatialMotionIr::origin(),
                source: source.clone(),
            }],
        },
        MusicalValue::NestedContainer(container_id) => {
            let normalized = program
                .containers
                .get(container_id)
                .cloned()
                .ok_or_else(|| {
                    vec![Diagnostic::new(
                        crate::domain::DiagnosticCategory::Placement,
                        crate::domain::DiagnosticKind::MissingContainer,
                        "Nested normalized container is missing from the normalized program.",
                        Some(crate::domain::DiagnosticLocation::ContainerStack {
                            container: container_id.clone(),
                            index: 0,
                        }),
                    )]
                })?;
            compile_container(program, &normalized, span, ctx)?
        }
    })
}

fn stamp_stream(mut stream: PatternStream, source: Option<PatternProvenance>) -> PatternStream {
    if let Some(source) = source {
        for event in &mut stream.events {
            if event.source.is_none() {
                event.source = Some(source.clone());
            }
        }
    }
    stream
}

fn apply_atom_modifiers(stream: PatternStream, modifiers: &[AtomModifier]) -> PatternStream {
    modifiers
        .iter()
        .fold(stream, |stream, modifier| match modifier {
            AtomModifier::Fast(factor) => stream.fast(*factor),
            AtomModifier::Slow(factor) => stream.slow(*factor),
            AtomModifier::Elongate(_) => stream,
            AtomModifier::Replicate(count) => replicate_stream(stream, *count),
            AtomModifier::Degrade(probability) => {
                degrade_stream(stream, probability.unwrap_or_else(|| Rational::new(1, 2)))
            }
            AtomModifier::Euclid { pulses, steps } => euclid_stream(stream, *pulses, *steps, 0),
            AtomModifier::EuclidRot {
                pulses,
                steps,
                rotation,
            } => euclid_stream(stream, *pulses, *steps, *rotation),
        })
}

fn degrade_stream(stream: PatternStream, probability: Rational) -> PatternStream {
    if probability <= Rational::zero() {
        return stream;
    }
    if probability >= Rational::one() {
        return PatternStream::default();
    }

    PatternStream {
        events: stream
            .events
            .into_iter()
            .filter(|event| {
                let seed = deterministic_event_seed(event);
                let threshold =
                    ((probability.numerator * 1024) / probability.denominator).clamp(0, 1024);
                (seed % 1024) >= threshold as u64
            })
            .collect(),
    }
}

fn replicate_stream(stream: PatternStream, count: u32) -> PatternStream {
    if count <= 1 || stream.events.is_empty() {
        return stream;
    }
    let duration = stream.duration().0;
    let mut events = Vec::new();
    for index in 0..count {
        let shifted = stream.shift_by(CycleDuration(
            duration * Rational::from_integer(index as i64),
        ));
        events.extend(shifted.events);
    }
    PatternStream { events }
}

fn euclid_stream(stream: PatternStream, pulses: u32, steps: u32, rotation: i32) -> PatternStream {
    if stream.events.is_empty() || steps == 0 {
        return stream;
    }
    let Some((origin, end)) = stream.bounds() else {
        return stream;
    };
    let step_duration = (end.0 - origin.0) / Rational::from_integer(steps as i64);
    let pattern = euclid_pattern(pulses, steps, rotation);
    PatternStream {
        events: stream
            .events
            .into_iter()
            .filter(|event| {
                let relative = event.span.start.0 - origin.0;
                let scaled = relative * Rational::from_integer(steps as i64) / (end.0 - origin.0);
                let step_index = scaled.numerator.div_euclid(scaled.denominator) as usize;
                pattern
                    .get(step_index.min(pattern.len().saturating_sub(1)))
                    .copied()
                    .unwrap_or(false)
            })
            .map(|mut event| {
                if event.span.duration.0 > step_duration {
                    event.span.duration = CycleDuration(step_duration);
                }
                event
            })
            .collect(),
    }
}

fn euclid_pattern(pulses: u32, steps: u32, rotation: i32) -> Vec<bool> {
    if steps == 0 {
        return Vec::new();
    }
    if pulses == 0 {
        return vec![false; steps as usize];
    }
    let mut pattern = (0..steps)
        .map(|step| (step * pulses) % steps < pulses)
        .collect::<Vec<_>>();
    if !pattern.is_empty() {
        pattern.rotate_right(rotation.rem_euclid(steps as i32) as usize);
    }
    pattern
}

fn sequence_weight(expr: &crate::domain::AtomExpr) -> Rational {
    expr.modifiers
        .iter()
        .fold(Rational::one(), |weight, modifier| match modifier {
            AtomModifier::Elongate(value) => weight * *value,
            _ => weight,
        })
}

fn deterministic_event_seed(event: &PatternEvent) -> u64 {
    let value_bias = match &event.value {
        EventValue::Note { value, octave } => {
            let text = value.bytes().fold(0u64, |acc, byte| {
                acc.wrapping_mul(31).wrapping_add(byte as u64)
            });
            text.wrapping_add(octave.unwrap_or_default() as u64)
        }
        EventValue::Rest => 17,
        EventValue::Scalar { value } => (value.numerator as u64)
            .wrapping_mul(13)
            .wrapping_add(value.denominator as u64),
    };
    value_bias
        .wrapping_add(event.span.start.0.numerator as u64 * 97)
        .wrapping_add(event.span.start.0.denominator as u64 * 53)
        .wrapping_add(event.span.duration.0.numerator as u64 * 29)
        .wrapping_add(event.span.duration.0.denominator as u64 * 11)
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
