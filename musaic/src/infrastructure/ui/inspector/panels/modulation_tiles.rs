//! Complete continuous-control tiles, edited by stable node identity.
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        session::MusaicProject,
    },
    domain::document::{AtomValue, DocumentNodeKind},
    infrastructure::ui::{
        theme::MusaicUiTheme,
        widgets::{exact_number, musaic_button},
    },
};
use bevy::{input_focus::InputFocus, prelude::*};
use bevy_ui_widgets::{Activate, ValueChange};
use tessera::prelude::{
    AtomModifier, ModulationParameters, ModulationWaveform, NodeId, ParameterKey, Rational,
};

#[path = "modulation_seed.rs"]
mod seed;
use seed::SeedInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operand {
    Rate,
    Phase,
    Minimum,
    Maximum,
}
impl Operand {
    fn read(self, value: ModulationParameters) -> Rational {
        match self {
            Self::Rate => value.rate,
            Self::Phase => value.phase,
            Self::Minimum => value.minimum,
            Self::Maximum => value.maximum,
        }
    }
    fn write(self, value: &mut ModulationParameters, next: Rational) {
        match self {
            Self::Rate => value.rate = next,
            Self::Phase => value.phase = next,
            Self::Minimum => value.minimum = next,
            Self::Maximum => value.maximum = next,
        }
    }
}
#[derive(Component)]
pub(crate) struct ModulationOperand {
    node: NodeId,
    operand: Operand,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Target(ParameterKey),
    Waveform(ModulationWaveform),
    Reset,
}
#[derive(Component)]
pub(crate) struct ModulationChoice {
    node: NodeId,
    choice: Choice,
}

const TARGETS: [(ParameterKey, &str); 5] = [
    (ParameterKey::Gain, "Gain"),
    (ParameterKey::Velocity, "Velocity"),
    (ParameterKey::PlaybackRate, "Sample speed"),
    (ParameterKey::LowPassCutoff, "Low-pass cutoff"),
    (ParameterKey::Transpose, "Transpose"),
];
const WAVES: [(ModulationWaveform, &str); 8] = [
    (ModulationWaveform::Sine, "Sine"),
    (ModulationWaveform::Saw, "Saw"),
    (ModulationWaveform::Triangle, "Triangle"),
    (ModulationWaveform::Square, "Square"),
    (ModulationWaveform::SmoothNoise, "Smooth noise"),
    (ModulationWaveform::SteppedNoise, "Stepped noise"),
    (ModulationWaveform::Random, "Random"),
    (ModulationWaveform::Ramp, "Ramp"),
];
fn row() -> Node {
    Node {
        width: percent(100),
        flex_wrap: FlexWrap::Wrap,
        column_gap: px(4),
        row_gap: px(4),
        ..default()
    }
}
fn label(parent: &mut ChildSpawnerCommands<'_>, text: &str) {
    super::tile_presentation::inspector_label(parent, text, 11.0, true);
}
fn selected(choice: Choice, parameter: ParameterKey, value: ModulationParameters) -> bool {
    match choice {
        Choice::Target(target) => target == parameter,
        Choice::Waveform(waveform) => waveform == value.waveform,
        Choice::Reset => false,
    }
}
fn background(active: bool) -> Color {
    if active {
        Color::srgb(0.886, 0.933, 0.898)
    } else {
        MusaicUiTheme::default_dark().chrome.panel_bg
    }
}
fn button(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    choice: Choice,
    title: &str,
    parameter: ParameterKey,
    value: ModulationParameters,
) {
    parent
        .spawn(musaic_button(
            Node {
                min_height: px(26),
                padding: UiRect::axes(px(7), px(3)),
                border: UiRect::all(px(1)),
                ..default()
            },
            (
                ModulationChoice {
                    node: node.clone(),
                    choice,
                },
                BackgroundColor(background(selected(choice, parameter, value))),
            ),
            title,
        ))
        .observe(choose);
}
pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, node: &NodeId, modifier: &AtomModifier) {
    let AtomModifier::Modulation { parameter, value } = *modifier else {
        return;
    };
    label(parent, "Modulate");
    parent.spawn(row()).with_children(|row| {
        for (target, title) in TARGETS {
            button(row, node, Choice::Target(target), title, parameter, value);
        }
    });
    label(parent, "Changing the target resets its range.");
    parent.spawn(row()).with_children(|row| {
        for (waveform, title) in WAVES {
            button(
                row,
                node,
                Choice::Waveform(waveform),
                title,
                parameter,
                value,
            );
        }
    });
    let units = match parameter {
        ParameterKey::Gain => "Range · gain multiplier",
        ParameterKey::Velocity => "Range · note intensity (0–1), sampled when each note starts",
        ParameterKey::PlaybackRate => "Range · sample-speed multiplier",
        ParameterKey::LowPassCutoff => "Range · cutoff in Hz",
        ParameterKey::Transpose => "Range · semitones",
        _ => "Range",
    };
    label(parent, units);
    for (operand, title) in [
        (Operand::Rate, "Rate · per cycle"),
        (Operand::Phase, "Phase · cycles"),
        (Operand::Minimum, "Minimum"),
        (Operand::Maximum, "Maximum"),
    ] {
        parent.spawn(row()).with_children(|row| {
            label(row, title);
            let field = exact_number::spawn(
                row,
                operand.read(value),
                ModulationOperand {
                    node: node.clone(),
                    operand,
                },
            );
            row.commands().entity(field).observe(exact_change);
        });
    }
    seed::spawn(parent, node, value.seed);
    label(
        parent,
        "Enter applies · Esc cancels. Seed chooses the noise variation.",
    );
    button(
        parent,
        node,
        Choice::Reset,
        "Reset modulation",
        parameter,
        value,
    );
}
fn current(project: &MusaicProject, node: &NodeId) -> Option<(ParameterKey, ModulationParameters)> {
    let DocumentNodeKind::Atom(atom) = &project.document.graph.node(node)?.kind else {
        return None;
    };
    let AtomValue::Modifier(AtomModifier::Modulation { parameter, value }) = atom.atom else {
        return None;
    };
    Some((parameter, value))
}
fn command(
    node: &NodeId,
    parameter: ParameterKey,
    value: ModulationParameters,
) -> Result<EditorCommand, &'static str> {
    value.validate_for(parameter)?;
    Ok(EditorCommand::SetAtomValue {
        node: node.clone(),
        value: AtomValue::Modifier(AtomModifier::Modulation { parameter, value }),
    })
}
fn choose(
    event: On<Activate>,
    buttons: Query<&ModulationChoice>,
    project: Res<MusaicProject>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let Some((mut parameter, mut value)) = current(&project, &button.node) else {
        return;
    };
    match button.choice {
        Choice::Target(next) if next != parameter => {
            parameter = next;
            let defaults = ModulationParameters::for_parameter(next);
            value.minimum = defaults.minimum;
            value.maximum = defaults.maximum;
        }
        Choice::Target(_) => return,
        Choice::Waveform(waveform) => value.waveform = waveform,
        Choice::Reset => value = ModulationParameters::for_parameter(parameter),
    }
    if let Ok(command) = command(&button.node, parameter, value) {
        bus.write(EditorCommandBus(command));
    }
}
fn exact_change(
    event: On<ValueChange<Rational>>,
    fields: Query<&ModulationOperand>,
    project: Res<MusaicProject>,
    mut texts: Query<&mut Text, With<exact_number::ExactNumber>>,
    mut focus: ResMut<InputFocus>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(field) = fields.get(event.source) else {
        return;
    };
    let Some((parameter, mut value)) = current(&project, &field.node) else {
        return;
    };
    field.operand.write(&mut value, event.value);
    match command(&field.node, parameter, value) {
        Ok(command) => {
            bus.write(EditorCommandBus(command));
        }
        Err(message) => {
            if let Ok(mut text) = texts.get_mut(event.source) {
                exact_number::reject(&mut text, &mut focus, event.source, message);
            }
        }
    }
}
pub(crate) fn sync(
    project: Res<MusaicProject>,
    focus: Res<InputFocus>,
    mut exact: Query<
        (
            Entity,
            &ModulationOperand,
            &mut exact_number::ExactNumber,
            &mut Text,
        ),
        Without<SeedInput>,
    >,
    mut seeds: Query<(Entity, &mut SeedInput, &mut Text), Without<ModulationOperand>>,
    mut buttons: Query<(&ModulationChoice, &mut BackgroundColor)>,
) {
    for (entity, binding, mut field, mut text) in &mut exact {
        if let Some((_, value)) = current(&project, &binding.node) {
            exact_number::update(
                &mut field,
                &mut text,
                binding.operand.read(value),
                focus.0 == Some(entity),
            );
        }
    }
    for (entity, mut field, mut text) in &mut seeds {
        if let Some((_, value)) = current(&project, &field.node) {
            seed::update(&mut field, &mut text, value.seed, focus.0 == Some(entity));
        }
    }
    for (button, mut color) in &mut buttons {
        if let Some((parameter, value)) = current(&project, &button.node) {
            let next = background(selected(button.choice, parameter, value));
            if color.0 != next {
                color.0 = next;
            }
        }
    }
}

#[cfg(test)]
#[path = "modulation_tiles_tests.rs"]
mod tests;
