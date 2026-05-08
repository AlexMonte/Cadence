use crate::domain::{
    common::GridPos,
    program::{
        CadenceControlKey, CadenceControlValueExpr, CadencePatternExpr, CadenceSourceKind,
        CadenceValueExpr,
    },
};

use super::semantic::{PatternValueAtom, PitchAtom, ScaleSpec, SemanticNode, SemanticPattern};
use super::surface::SurfaceSequenceMode;

pub fn lower_pattern_values(
    pattern: &SemanticPattern<PatternValueAtom>,
    site: Option<GridPos>,
) -> CadencePatternExpr {
    lower_node(&pattern.root, &mut |atom| match atom {
        PatternValueAtom::Scalar(text) => CadencePatternExpr::Source {
            source: CadenceSourceKind::Pulse,
            value: CadenceValueExpr::text(text.clone()),
            site,
        },
    })
}

pub fn transpose_pitch_pattern(
    input: CadencePatternExpr,
    offsets: &SemanticPattern<PitchAtom>,
) -> CadencePatternExpr {
    lower_node(&offsets.root, &mut |atom| match atom {
        PitchAtom::Offset(offset) => transpose_pattern(input.clone(), *offset),
        _ => CadencePatternExpr::Silence,
    })
}

pub fn apply_scale_to_pattern_expr(
    input: CadencePatternExpr,
    spec: ScaleSpec,
) -> CadencePatternExpr {
    apply_scale_to_pattern(input, spec)
}

fn lower_node<A>(
    node: &SemanticNode<A>,
    atom_lower: &mut impl FnMut(&A) -> CadencePatternExpr,
) -> CadencePatternExpr {
    match node {
        SemanticNode::Rest => CadencePatternExpr::Silence,
        SemanticNode::Atom(atom) => atom_lower(atom),
        SemanticNode::Sequence { mode, children } => lower_sequence(*mode, children, atom_lower),
        SemanticNode::Stack(children) | SemanticNode::Chord(children) => merge_expr(
            children
                .iter()
                .map(|child| lower_node(child, atom_lower))
                .collect(),
        ),
        SemanticNode::Alternate { inner } => match lower_node(inner, atom_lower) {
            CadencePatternExpr::CycleSlots { branches } => {
                CadencePatternExpr::CycleRoute { branches }
            }
            CadencePatternExpr::Silence => CadencePatternExpr::Silence,
            branch => CadencePatternExpr::CycleRoute {
                branches: vec![branch],
            },
        },
        SemanticNode::Repeat { inner, times } => {
            let branch = lower_node(inner, atom_lower);
            sequence_expr(
                SurfaceSequenceMode::Cycle,
                (0..*times).map(|_| branch.clone()).collect(),
            )
        }
        SemanticNode::Fast { inner, factor } => CadencePatternExpr::Fast {
            input: Box::new(lower_node(inner, atom_lower)),
            factor: CadenceValueExpr::number(*factor),
        },
        SemanticNode::Slow { inner, factor } => CadencePatternExpr::Slow {
            input: Box::new(lower_node(inner, atom_lower)),
            factor: CadenceValueExpr::number(*factor),
        },
        SemanticNode::Elongate { inner, .. } => lower_node(inner, atom_lower),
    }
}

fn lower_sequence<A>(
    mode: SurfaceSequenceMode,
    children: &[SemanticNode<A>],
    atom_lower: &mut impl FnMut(&A) -> CadencePatternExpr,
) -> CadencePatternExpr {
    let weighted = children
        .iter()
        .map(sequence_child_weight)
        .collect::<Vec<_>>();
    let has_weighted_child = weighted.iter().any(|(_, weight)| *weight > 1);
    if !has_weighted_child {
        return sequence_expr(
            mode,
            children
                .iter()
                .map(|child| lower_node(child, atom_lower))
                .collect(),
        );
    }

    match mode {
        SurfaceSequenceMode::Cycle => {
            let total_weight = weighted
                .iter()
                .map(|(_, weight)| *weight)
                .sum::<u32>()
                .max(1);
            let mut offset = 0_u32;
            let mut branches = Vec::new();
            for (child, weight) in weighted {
                let lowered = lower_node(child, atom_lower);
                let factor = total_weight as f64 / f64::from(weight);
                let scaled = if (factor - 1.0).abs() < f64::EPSILON {
                    lowered
                } else {
                    CadencePatternExpr::Fast {
                        input: Box::new(lowered),
                        factor: CadenceValueExpr::number(factor),
                    }
                };
                let shifted = if offset == 0 {
                    scaled
                } else {
                    CadencePatternExpr::Shift {
                        input: Box::new(scaled),
                        offset: CadenceValueExpr::number(
                            f64::from(offset) / f64::from(total_weight),
                        ),
                    }
                };
                branches.push(shifted);
                offset = offset.saturating_add(weight);
            }
            merge_expr(branches)
        }
        SurfaceSequenceMode::Chain => {
            let mut branches = Vec::new();
            for (child, weight) in weighted {
                let lowered = lower_node(child, atom_lower);
                for _ in 0..weight {
                    branches.push(lowered.clone());
                }
            }
            sequence_expr(mode, branches)
        }
    }
}

fn sequence_child_weight<A>(child: &SemanticNode<A>) -> (&SemanticNode<A>, u32) {
    match child {
        SemanticNode::Elongate { inner, weight } => (inner.as_ref(), *weight),
        other => (other, 1),
    }
}

fn sequence_expr(
    mode: SurfaceSequenceMode,
    mut branches: Vec<CadencePatternExpr>,
) -> CadencePatternExpr {
    match branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => branches.pop().expect("single branch exists"),
        _ => match mode {
            SurfaceSequenceMode::Cycle => CadencePatternExpr::CycleSlots { branches },
            SurfaceSequenceMode::Chain => CadencePatternExpr::CycleRoute { branches },
        },
    }
}

fn merge_expr(mut branches: Vec<CadencePatternExpr>) -> CadencePatternExpr {
    match branches.len() {
        0 => CadencePatternExpr::Silence,
        1 => branches.pop().expect("single branch exists"),
        _ => CadencePatternExpr::Merge { branches },
    }
}

fn transpose_pattern(pattern: CadencePatternExpr, offset: f64) -> CadencePatternExpr {
    match pattern {
        CadencePatternExpr::Silence
        | CadencePatternExpr::Argument { .. }
        | CadencePatternExpr::Source { .. } => pattern,
        CadencePatternExpr::Merge { branches } => CadencePatternExpr::Merge {
            branches: branches
                .into_iter()
                .map(|branch| transpose_pattern(branch, offset))
                .collect(),
        },
        CadencePatternExpr::CycleRoute { branches } => CadencePatternExpr::CycleRoute {
            branches: branches
                .into_iter()
                .map(|branch| transpose_pattern(branch, offset))
                .collect(),
        },
        CadencePatternExpr::CycleSlots { branches } => CadencePatternExpr::CycleSlots {
            branches: branches
                .into_iter()
                .map(|branch| transpose_pattern(branch, offset))
                .collect(),
        },
        CadencePatternExpr::ReflectCycle { input } => CadencePatternExpr::ReflectCycle {
            input: Box::new(transpose_pattern(*input, offset)),
        },
        CadencePatternExpr::Mask { input, by } => CadencePatternExpr::Mask {
            input: Box::new(transpose_pattern(*input, offset)),
            by: Box::new(transpose_pattern(*by, offset)),
        },
        CadencePatternExpr::Gain { input, amount } => CadencePatternExpr::Gain {
            input: Box::new(transpose_pattern(*input, offset)),
            amount,
        },
        CadencePatternExpr::Fast { input, factor } => CadencePatternExpr::Fast {
            input: Box::new(transpose_pattern(*input, offset)),
            factor,
        },
        CadencePatternExpr::Slow { input, factor } => CadencePatternExpr::Slow {
            input: Box::new(transpose_pattern(*input, offset)),
            factor,
        },
        CadencePatternExpr::Shift {
            input,
            offset: shift,
        } => CadencePatternExpr::Shift {
            input: Box::new(transpose_pattern(*input, offset)),
            offset: shift,
        },
        CadencePatternExpr::Control { input, key, value } => CadencePatternExpr::Control {
            input: Box::new(transpose_pattern(*input, offset)),
            key,
            value: if key == CadenceControlKey::Pitch {
                match value {
                    CadenceControlValueExpr::Scalar { value } => CadenceControlValueExpr::Scalar {
                        value: match value {
                            CadenceValueExpr::Text { value } => {
                                crate::domain::script::parse_pitch_text(&value)
                                    .map(|pitch| CadenceValueExpr::number(pitch + offset))
                                    .unwrap_or(CadenceValueExpr::text(value))
                            }
                            CadenceValueExpr::Number { value } => {
                                CadenceValueExpr::number(value + offset)
                            }
                            other => other,
                        },
                    },
                    CadenceControlValueExpr::Pattern { expr } => CadenceControlValueExpr::Pattern {
                        expr: Box::new(transpose_pattern(*expr, offset)),
                    },
                    other => other,
                }
            } else {
                match value {
                    CadenceControlValueExpr::Pattern { expr } => CadenceControlValueExpr::Pattern {
                        expr: Box::new(transpose_pattern(*expr, offset)),
                    },
                    other => other,
                }
            },
        },
        CadencePatternExpr::Call { call } => CadencePatternExpr::Call { call },
        CadencePatternExpr::State { state } => CadencePatternExpr::State { state },
    }
}

fn apply_scale_to_pattern(pattern: CadencePatternExpr, spec: ScaleSpec) -> CadencePatternExpr {
    match pattern {
        CadencePatternExpr::Silence
        | CadencePatternExpr::Argument { .. }
        | CadencePatternExpr::Source { .. } => pattern,
        CadencePatternExpr::Merge { branches } => CadencePatternExpr::Merge {
            branches: branches
                .into_iter()
                .map(|branch| apply_scale_to_pattern(branch, spec))
                .collect(),
        },
        CadencePatternExpr::CycleRoute { branches } => CadencePatternExpr::CycleRoute {
            branches: branches
                .into_iter()
                .map(|branch| apply_scale_to_pattern(branch, spec))
                .collect(),
        },
        CadencePatternExpr::CycleSlots { branches } => CadencePatternExpr::CycleSlots {
            branches: branches
                .into_iter()
                .map(|branch| apply_scale_to_pattern(branch, spec))
                .collect(),
        },
        CadencePatternExpr::ReflectCycle { input } => CadencePatternExpr::ReflectCycle {
            input: Box::new(apply_scale_to_pattern(*input, spec)),
        },
        CadencePatternExpr::Mask { input, by } => CadencePatternExpr::Mask {
            input: Box::new(apply_scale_to_pattern(*input, spec)),
            by: Box::new(apply_scale_to_pattern(*by, spec)),
        },
        CadencePatternExpr::Gain { input, amount } => CadencePatternExpr::Gain {
            input: Box::new(apply_scale_to_pattern(*input, spec)),
            amount,
        },
        CadencePatternExpr::Fast { input, factor } => CadencePatternExpr::Fast {
            input: Box::new(apply_scale_to_pattern(*input, spec)),
            factor,
        },
        CadencePatternExpr::Slow { input, factor } => CadencePatternExpr::Slow {
            input: Box::new(apply_scale_to_pattern(*input, spec)),
            factor,
        },
        CadencePatternExpr::Shift { input, offset } => CadencePatternExpr::Shift {
            input: Box::new(apply_scale_to_pattern(*input, spec)),
            offset,
        },
        CadencePatternExpr::Control { input, key, value } => CadencePatternExpr::Control {
            input: Box::new(apply_scale_to_pattern(*input, spec)),
            key,
            value: if key == CadenceControlKey::Pitch {
                match value {
                    CadenceControlValueExpr::Scalar { value } => CadenceControlValueExpr::Scalar {
                        value: match value {
                            CadenceValueExpr::Text { value } => value
                                .trim()
                                .parse::<i32>()
                                .ok()
                                .map(|degree| {
                                    CadenceValueExpr::number(
                                        scale_degree_to_pitch(degree, spec) as f64
                                    )
                                })
                                .unwrap_or(CadenceValueExpr::text(value)),
                            other => other,
                        },
                    },
                    CadenceControlValueExpr::Pattern { expr } => CadenceControlValueExpr::Pattern {
                        expr: Box::new(apply_scale_to_pattern(*expr, spec)),
                    },
                    other => other,
                }
            } else {
                match value {
                    CadenceControlValueExpr::Pattern { expr } => CadenceControlValueExpr::Pattern {
                        expr: Box::new(apply_scale_to_pattern(*expr, spec)),
                    },
                    other => other,
                }
            },
        },
        CadencePatternExpr::Call { call } => CadencePatternExpr::Call { call },
        CadencePatternExpr::State { state } => CadencePatternExpr::State { state },
    }
}

fn scale_degree_to_pitch(degree: i32, spec: ScaleSpec) -> i32 {
    let octave = degree.div_euclid(7);
    let degree_index = degree.rem_euclid(7) as usize;
    spec.root_midi + octave * 12 + spec.intervals[degree_index]
}
