//! Saved envelope, sends, and ordered effects in the existing sound inspector.
use super::sound::{SoundParameter, choice_row, label, projected_sound, spawn_parameter};
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        pipeline::{sound::SoundPaint, ui_projection::EditorUiProjection},
    },
    domain::instrument::{
        InstrumentCompressor, InstrumentDefinition, InstrumentDelay, InstrumentEffect,
        InstrumentReverb,
    },
    infrastructure::ui::widgets::musaic_button,
};
use bevy::prelude::*;
use bevy_ui_widgets::Activate;
use tessera::prelude::{NodeId, Rational};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EffectParameter {
    Cutoff,
    Resonance,
    Amount,
    Wet,
    OutputGain,
}
#[derive(Component, Clone)]
struct SoundActionButton {
    output: NodeId,
    action: SoundAction,
}
#[derive(Clone)]
enum SoundAction {
    ResetEnvelope,
    ToggleDelay,
    ToggleReverb,
    ToggleCompressor,
    Add(InstrumentEffect),
    Remove(usize),
    Move(usize, usize),
}

fn button(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    title: &str,
    action: SoundAction,
) {
    parent
        .spawn(musaic_button(
            Node {
                min_height: px(26),
                padding: UiRect::axes(px(7), px(3)),
                ..default()
            },
            SoundActionButton {
                output: output.clone(),
                action,
            },
            title,
        ))
        .observe(on_action);
}
fn on_action(
    event: On<Activate>,
    buttons: Query<&SoundActionButton>,
    projection: Res<EditorUiProjection>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let Some(sound) = projected_sound(&projection, &button.output) else {
        return;
    };
    let mut instrument = sound.instrument.clone();
    match &button.action {
        SoundAction::ResetEnvelope => {
            instrument.sound.envelope = None;
            instrument.sound.gain = None;
        }
        SoundAction::ToggleDelay => {
            instrument.sound.delay = if instrument.sound.delay.is_some() {
                None
            } else {
                Some(InstrumentDelay::default())
            }
        }
        SoundAction::ToggleReverb => {
            instrument.sound.reverb = if instrument.sound.reverb.is_some() {
                None
            } else {
                Some(InstrumentReverb::default())
            }
        }
        SoundAction::ToggleCompressor => {
            instrument.sound.compressor = if instrument.sound.compressor.is_some() {
                None
            } else {
                Some(InstrumentCompressor::default())
            }
        }
        SoundAction::Add(effect) => instrument.sound.effects.push(effect.clone()),
        SoundAction::Remove(index) => {
            if *index >= instrument.sound.effects.len() {
                return;
            }
            instrument.sound.effects.remove(*index);
        }
        SoundAction::Move(from, to) => {
            if *from >= instrument.sound.effects.len() || *to >= instrument.sound.effects.len() {
                return;
            }
            instrument.sound.effects.swap(*from, *to);
        }
    }
    if instrument.validate().is_ok() {
        bus.write(EditorCommandBus(EditorCommand::SetSound {
            sound: button.output.clone(),
            definition: instrument,
        }));
    }
}

pub(super) fn spawn_sound_design(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    sound: &SoundPaint,
) {
    use SoundParameter as P;
    label(parent, "Envelope", 14.0);
    label(parent, "Times in seconds · note tiles can override", 10.0);
    for (parameter, title, maximum, step, precision) in [
        (P::Gain, "Gain", 4.0, 0.01, 2),
        (P::Attack, "Attack s", 5.0, 0.001, 3),
        (P::Decay, "Decay s", 5.0, 0.001, 3),
        (P::Sustain, "Sustain", 1.0, 0.01, 2),
        (P::Release, "Release s", 5.0, 0.001, 3),
    ] {
        parameter_row(
            parent,
            output,
            &sound.instrument,
            parameter,
            title,
            0.0,
            maximum,
            step,
            precision,
        );
    }
    button(
        parent,
        output,
        "Use source envelope and gain",
        SoundAction::ResetEnvelope,
    );
    label(parent, "Dynamics", 14.0);
    button(
        parent,
        output,
        if sound.instrument.sound.compressor.is_some() {
            "Compressor: on"
        } else {
            "Compressor: off"
        },
        SoundAction::ToggleCompressor,
    );
    if sound.instrument.sound.compressor.is_some() {
        label(parent, "Compression runs before the effect chain", 10.0);
        for (parameter, title, min, max, step, precision) in [
            (P::CompressorThreshold, "Threshold", 0.0, 1.0, 0.01, 2),
            (P::CompressorRatio, "Ratio :1", 1.0, 20.0, 0.1, 1),
            (P::CompressorKnee, "Knee · dB", 0.0, 40.0, 0.1, 1),
            (P::CompressorAttack, "Attack s", 0.001, 1.0, 0.001, 3),
            (P::CompressorRelease, "Release s", 0.001, 2.0, 0.001, 3),
        ] {
            parameter_row(
                parent,
                output,
                &sound.instrument,
                parameter,
                title,
                min,
                max,
                step,
                precision,
            );
        }
    }
    label(parent, "Effects · processed top to bottom", 14.0);
    for (index, effect) in sound.instrument.sound.effects.iter().enumerate() {
        let name = match effect {
            InstrumentEffect::LowPass { .. } => "Low-pass",
            InstrumentEffect::HighPass { .. } => "High-pass",
            InstrumentEffect::Drive { .. } => "Drive",
        };
        choice_row(parent, &format!("{}. {name}", index + 1), |row| {
            if index > 0 {
                button(row, output, "↑", SoundAction::Move(index, index - 1));
            }
            if index + 1 < sound.instrument.sound.effects.len() {
                button(row, output, "↓", SoundAction::Move(index, index + 1));
            }
            button(row, output, "Remove", SoundAction::Remove(index));
        });
        use EffectParameter as E;
        let parameters = match effect {
            InstrumentEffect::LowPass { .. } | InstrumentEffect::HighPass { .. } => vec![
                (E::Cutoff, "Cutoff Hz", 20.0, 20_000.0, 10.0, 0),
                (E::Resonance, "Resonance", 0.0, 1.0, 0.01, 2),
            ],
            InstrumentEffect::Drive { .. } => vec![
                (E::Amount, "Drive", 0.0, 1.0, 0.01, 2),
                (E::Wet, "Wet", 0.0, 1.0, 0.01, 2),
                (E::OutputGain, "Output", 0.0, 2.0, 0.01, 2),
            ],
        };
        for (parameter, title, minimum, maximum, step, precision) in parameters {
            parameter_row(
                parent,
                output,
                &sound.instrument,
                P::Effect(index, parameter),
                title,
                minimum,
                maximum,
                step,
                precision,
            );
        }
    }
    if sound.instrument.sound.effects.len() < cadence::prelude::InsertChain::MAX_EFFECTS {
        choice_row(parent, "Add", |row| {
            button(
                row,
                output,
                "Low-pass",
                SoundAction::Add(InstrumentEffect::LowPass {
                    cutoff_hz: Rational::new(2_000, 1),
                    resonance: Rational::zero(),
                }),
            );
            button(
                row,
                output,
                "High-pass",
                SoundAction::Add(InstrumentEffect::HighPass {
                    cutoff_hz: Rational::new(200, 1),
                    resonance: Rational::zero(),
                }),
            );
            button(
                row,
                output,
                "Drive",
                SoundAction::Add(InstrumentEffect::Drive {
                    amount: Rational::new(1, 3),
                    wet: Rational::one(),
                    output_gain: Rational::one(),
                }),
            );
        });
    }
    label(parent, "Space", 14.0);
    choice_row(parent, "Sends", |row| {
        button(
            row,
            output,
            if sound.instrument.sound.delay.is_some() {
                "Delay: on"
            } else {
                "Delay: off"
            },
            SoundAction::ToggleDelay,
        );
        button(
            row,
            output,
            if sound.instrument.sound.reverb.is_some() {
                "Reverb: on"
            } else {
                "Reverb: off"
            },
            SoundAction::ToggleReverb,
        );
    });
    if sound.instrument.sound.delay.is_some() {
        for (parameter, title, min, max) in [
            (P::DelayAmount, "Delay wet", 0.0, 1.0),
            (P::DelayTime, "Time s", 0.001, 2.0),
            (P::DelayFeedback, "Feedback", 0.0, 1.0),
            (P::DelayDamping, "Damping", 0.0, 1.0),
        ] {
            parameter_row(
                parent,
                output,
                &sound.instrument,
                parameter,
                title,
                min,
                max,
                0.01,
                2,
            );
        }
    }
    if sound.instrument.sound.reverb.is_some() {
        for (parameter, title, min, max) in [
            (P::ReverbAmount, "Reverb wet", 0.0, 1.0),
            (P::ReverbDecay, "Tail s", 0.01, 10.0),
            (P::ReverbDamping, "Damping", 0.0, 1.0),
        ] {
            parameter_row(
                parent,
                output,
                &sound.instrument,
                parameter,
                title,
                min,
                max,
                0.01,
                2,
            );
        }
    }
}
fn parameter_row(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    instrument: &InstrumentDefinition,
    parameter: SoundParameter,
    title: &str,
    min: f32,
    max: f32,
    step: f32,
    precision: i32,
) {
    let value = parameter.value(instrument);
    spawn_parameter(
        parent,
        output,
        parameter,
        title,
        parameter.exact_value(instrument),
        min,
        max.max(value),
        step,
        precision,
    );
}
