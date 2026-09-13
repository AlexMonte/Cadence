//! Harmonic and Euclidean settings use the same owned-atom edit command as other tiles.
use bevy::prelude::*;
use bevy_ui_widgets::ValueChange;
use tessera::prelude::{AtomModifier as M, EuclidPatternParameters, NodeId, Rational, ScaleMode};

use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        session::MusaicProject,
    },
    domain::document::{AtomValue, DocumentNodeKind},
    infrastructure::ui::{
        InspectorButtonAction, on_inspector_button_activated,
        widgets::{exact_number, musaic_button},
    },
};

#[derive(Debug, Clone, Copy)]
enum RhythmLane {
    Pulses,
    Steps,
    Rotations,
}

#[derive(Component)]
struct RhythmOperand {
    node: NodeId,
    lane: RhythmLane,
    index: usize,
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, node: &NodeId, modifier: &M) {
    match modifier {
        M::Scale(scale) => {
            label(
                parent,
                &format!("{} · {}", scale.root_label(), scale.mode.label()),
            );
            label(
                parent,
                "Place on a number or number pattern. 0 is the root; negative degrees descend.",
            );
            row(parent, |parent| {
                for (delta, title) in [
                    (-12, "− Octave"),
                    (-1, "− Semitone"),
                    (1, "+ Semitone"),
                    (12, "+ Octave"),
                ] {
                    let root = i16::from(scale.root) + delta;
                    if (0..=127).contains(&root) {
                        choice(
                            parent,
                            node,
                            M::Scale(tessera::prelude::ScaleParameters {
                                root: root as u8,
                                ..*scale
                            }),
                            title,
                        );
                    }
                }
            });
            row(parent, |parent| {
                for mode in ScaleMode::ALL {
                    if mode != scale.mode {
                        choice(
                            parent,
                            node,
                            M::Scale(tessera::prelude::ScaleParameters { mode, ..*scale }),
                            mode.label(),
                        );
                    }
                }
            });
        }
        M::EuclidPattern(pattern) => {
            label(
                parent,
                "Each input advances once per cycle and repeats independently.",
            );
            for (lane, title, values) in [
                (
                    RhythmLane::Pulses,
                    "Pulses per cycle",
                    pattern
                        .pulses
                        .iter()
                        .map(|v| i64::from(*v))
                        .collect::<Vec<_>>(),
                ),
                (
                    RhythmLane::Steps,
                    "Steps per cycle",
                    pattern.steps.iter().map(|v| i64::from(*v)).collect(),
                ),
                (
                    RhythmLane::Rotations,
                    "Rotation per cycle",
                    pattern.rotations.iter().map(|v| i64::from(*v)).collect(),
                ),
            ] {
                label(parent, title);
                row(parent, |parent| {
                    for (index, value) in values.iter().enumerate() {
                        let entity = exact_number::spawn(
                            parent,
                            Rational::from_integer(*value),
                            RhythmOperand {
                                node: node.clone(),
                                lane,
                                index,
                            },
                        );
                        parent.commands().entity(entity).observe(edit_operand);
                    }
                    if values.len() < EuclidPatternParameters::MAX_PERIOD {
                        let mut next = pattern.clone();
                        match lane {
                            RhythmLane::Pulses => next.pulses.push(*next.pulses.last().unwrap()),
                            RhythmLane::Steps => next.steps.push(*next.steps.last().unwrap()),
                            RhythmLane::Rotations => {
                                next.rotations.push(*next.rotations.last().unwrap())
                            }
                        }
                        if next.validate().is_ok() {
                            choice(parent, node, M::EuclidPattern(next), "+ Value");
                        }
                    }
                    if values.len() > 1 {
                        let mut next = pattern.clone();
                        match lane {
                            RhythmLane::Pulses => {
                                next.pulses.pop();
                            }
                            RhythmLane::Steps => {
                                next.steps.pop();
                            }
                            RhythmLane::Rotations => {
                                next.rotations.pop();
                            }
                        }
                        if next.validate().is_ok() {
                            choice(parent, node, M::EuclidPattern(next), "− Last");
                        }
                    }
                });
            }
            if pattern.pulses.len() == 1 && pattern.steps.len() == 1 && pattern.rotations.len() == 1
            {
                choice(
                    parent,
                    node,
                    M::EuclidRot {
                        pulses: pattern.pulses[0],
                        steps: pattern.steps[0],
                        rotation: pattern.rotations[0],
                    },
                    "Use fixed rhythm",
                );
            }
        }
        M::Euclid { pulses, steps } => choice(
            parent,
            node,
            M::EuclidPattern(EuclidPatternParameters {
                pulses: vec![*pulses],
                steps: vec![*steps],
                rotations: vec![0],
            }),
            "Vary inputs by cycle",
        ),
        M::EuclidRot {
            pulses,
            steps,
            rotation,
        } => choice(
            parent,
            node,
            M::EuclidPattern(EuclidPatternParameters {
                pulses: vec![*pulses],
                steps: vec![*steps],
                rotations: vec![*rotation],
            }),
            "Vary inputs by cycle",
        ),
        _ => {}
    }
}

fn row(parent: &mut ChildSpawnerCommands<'_>, build: impl FnOnce(&mut ChildSpawnerCommands<'_>)) {
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(4),
            row_gap: px(4),
            ..default()
        })
        .with_children(build);
}

fn label(parent: &mut ChildSpawnerCommands<'_>, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font_size: 12.,
            ..default()
        },
        TextColor(
            crate::infrastructure::ui::theme::MusaicUiTheme::default_dark()
                .chrome
                .text_main,
        ),
    ));
}

fn choice(parent: &mut ChildSpawnerCommands<'_>, node: &NodeId, modifier: M, title: &str) {
    parent
        .spawn(musaic_button(
            Node {
                min_height: px(28),
                padding: UiRect::horizontal(px(6)),
                ..default()
            },
            InspectorButtonAction(EditorCommand::SetAtomValue {
                node: node.clone(),
                value: AtomValue::Modifier(modifier),
            }),
            title,
        ))
        .observe(on_inspector_button_activated);
}

fn write_operand(
    pattern: &mut EuclidPatternParameters,
    lane: RhythmLane,
    index: usize,
    value: Rational,
) -> Result<(), &'static str> {
    if value.denominator != 1 {
        return Err("Use a whole number.");
    }
    match lane {
        RhythmLane::Pulses => {
            *pattern
                .pulses
                .get_mut(index)
                .ok_or("Cycle value no longer exists.")? =
                u32::try_from(value.numerator).map_err(|_| "Pulses must be nonnegative.")?
        }
        RhythmLane::Steps => {
            *pattern
                .steps
                .get_mut(index)
                .ok_or("Cycle value no longer exists.")? =
                u32::try_from(value.numerator).map_err(|_| "Steps must be positive.")?
        }
        RhythmLane::Rotations => {
            *pattern
                .rotations
                .get_mut(index)
                .ok_or("Cycle value no longer exists.")? =
                i32::try_from(value.numerator).map_err(|_| "Rotation is too large.")?
        }
    }
    pattern.validate()
}

fn edit_operand(
    event: On<ValueChange<Rational>>,
    bindings: Query<&RhythmOperand>,
    project: Res<MusaicProject>,
    mut fields: Query<&mut Text, With<exact_number::ExactNumber>>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(binding) = bindings.get(event.source) else {
        return;
    };
    let Some(DocumentNodeKind::Atom(atom)) = project
        .document
        .graph
        .node(&binding.node)
        .map(|node| &node.kind)
    else {
        return;
    };
    let AtomValue::Modifier(M::EuclidPattern(mut pattern)) = atom.atom.clone() else {
        return;
    };
    match write_operand(&mut pattern, binding.lane, binding.index, event.value) {
        Ok(()) => {
            bus.write(EditorCommandBus(EditorCommand::SetAtomValue {
                node: binding.node.clone(),
                value: AtomValue::Modifier(M::EuclidPattern(pattern)),
            }));
        }
        Err(message) => {
            if let Ok(mut text) = fields.get_mut(event.source) {
                exact_number::reject(&mut text, &mut focus, event.source, message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cycle_input_edit_checks_the_combined_rhythm_and_accepts_negative_rotation() {
        let mut pattern = EuclidPatternParameters::default();
        write_operand(
            &mut pattern,
            RhythmLane::Rotations,
            1,
            Rational::from_integer(-2),
        )
        .unwrap();
        assert_eq!(pattern.rotations, [0, -2]);
        assert!(
            write_operand(
                &mut pattern.clone(),
                RhythmLane::Pulses,
                0,
                Rational::from_integer(9)
            )
            .is_err()
        );
        assert!(
            write_operand(
                &mut pattern.clone(),
                RhythmLane::Steps,
                0,
                Rational::from_integer(0)
            )
            .is_err()
        );
        assert!(
            write_operand(
                &mut pattern.clone(),
                RhythmLane::Pulses,
                0,
                Rational::new(3, 2)
            )
            .is_err()
        );
        assert!(write_operand(&mut pattern, RhythmLane::Rotations, 9, Rational::zero()).is_err());
    }
}
