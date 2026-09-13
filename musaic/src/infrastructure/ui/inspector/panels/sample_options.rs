//! Imported recording metadata is shared by all Sound tiles using it.
use crate::infrastructure::ui::widgets::slider;
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        pipeline::{sound::SampleOptionsPaint, ui_projection::EditorUiProjection},
    },
    domain::project::samples::{SampleId, SampleImportOptions},
    infrastructure::ui::{theme::MusaicUiTheme, widgets::musaic_button},
};
use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::{
    controls::SliderProps,
    theme::{ThemeFontColor, ThemedText},
    tokens::TEXT_MAIN,
};
use bevy_ui_widgets::{Activate, SliderPrecision, SliderStep, SliderValue, ValueChange};
use tessera::prelude::NodeId;

#[path = "bank_tile.rs"]
mod bank_tile;
#[path = "sample_bank.rs"]
mod sample_bank;
#[path = "sample_loop.rs"]
mod sample_loop;
pub(crate) use bank_tile::{spawn_bank_tile, sync_bank_tile_choices};

#[derive(Clone, Copy)]
enum Parameter {
    Root,
    Gain,
}
#[derive(Component)]
pub(crate) struct SampleOptionSlider {
    output: NodeId,
    parameter: Parameter,
}
#[derive(Component)]
pub(crate) struct SampleOptionValue {
    output: NodeId,
    parameter: Parameter,
}
#[derive(Component)]
pub(crate) struct SampleOptionExact {
    output: NodeId,
    parameter: Parameter,
}
#[derive(Component)]
struct ToggleRoot(NodeId);
#[derive(Component)]
#[cfg(not(target_arch = "wasm32"))]
struct Relink(SampleId);

fn label(parent: &mut ChildSpawnerCommands<'_>, text: &str) {
    parent.spawn((
        UiText::new(text),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(MusaicUiTheme::default_dark().chrome.text_main),
        ThemeFontColor(TEXT_MAIN),
        ThemedText,
    ));
}
pub(crate) fn spawn_sample_options_panel(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    sample: &SampleOptionsPaint,
) {
    label(parent, "Recording settings · shared by all uses");
    parent
        .spawn(Node {
            display: Display::Flex,
            column_gap: px(6.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(musaic_button(
                Node {
                    min_height: px(26.0),
                    padding: UiRect::axes(px(8.0), px(3.0)),
                    ..default()
                },
                ToggleRoot(output.clone()),
                if sample.options.root_pitch.is_some() {
                    "Pitched: on"
                } else {
                    "Pitched: off"
                },
            ))
            .observe(toggle_root);
            #[cfg(not(target_arch = "wasm32"))]
            row.spawn(musaic_button(
                Node {
                    min_height: px(26.0),
                    padding: UiRect::axes(px(8.0), px(3.0)),
                    ..default()
                },
                Relink(sample.sample),
                "Relink WAV…",
            ))
            .observe(relink);
        });
    for (parameter, name, max, step) in [
        (Parameter::Root, "Root", 127.0, 1.0),
        (Parameter::Gain, "Gain", 16.0, 0.01),
    ] {
        if matches!(parameter, Parameter::Root) && sample.options.root_pitch.is_none() {
            continue;
        }
        parent
            .spawn(Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    UiText::new(name),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    Node {
                        width: px(40.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    TextColor(MusaicUiTheme::default_dark().chrome.text_main),
                    ThemeFontColor(TEXT_MAIN),
                    ThemedText,
                ));
                row.spawn(slider(
                    SliderProps {
                        value: value(&sample.options, parameter),
                        min: 0.0,
                        max,
                    },
                    (
                        crate::infrastructure::ui::widgets::ExactSlider,
                        SampleOptionSlider {
                            output: output.clone(),
                            parameter,
                        },
                        SliderStep(step),
                        SliderPrecision(2),
                    ),
                ))
                .with_children(|slider| {
                    let field = crate::infrastructure::ui::widgets::exact_number::spawn_inline(
                        slider,
                        exact_value(&sample.options, parameter),
                        SampleOptionExact {
                            output: output.clone(),
                            parameter,
                        },
                    );
                    slider.commands().entity(field).observe(exact_change);
                })
                .observe(change)
                .observe(begin)
                .observe(end);
            });
    }
    sample_loop::spawn(parent, output, sample);
    sample_bank::spawn(parent, sample);
}
fn projected<'a>(
    projection: &'a EditorUiProjection,
    output: &NodeId,
) -> Option<&'a SampleOptionsPaint> {
    super::sound::projected_sound(projection, output)?
        .sample_options
        .as_ref()
}
fn value(options: &SampleImportOptions, parameter: Parameter) -> f32 {
    match parameter {
        Parameter::Root => options.root_pitch.unwrap_or(60.0) as f32,
        Parameter::Gain => options.default_gain.unwrap_or(1.0),
    }
}
fn value_label(options: &SampleImportOptions, parameter: Parameter) -> String {
    match parameter {
        Parameter::Gain => format!("{:.0}%", value(options, parameter) * 100.0),
        Parameter::Root => {
            let pitch = options.root_pitch.unwrap_or(60.0);
            let midi = pitch.round() as usize;
            format!(
                "{}{} · {:.0}",
                [
                    "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"
                ][midi % 12],
                midi as i32 / 12 - 1,
                pitch
            )
        }
    }
}

fn exact_value(options: &SampleImportOptions, parameter: Parameter) -> tessera::prelude::Rational {
    let value = match parameter {
        Parameter::Root => options.root_pitch.unwrap_or(60.0),
        Parameter::Gain => f64::from(options.default_gain.unwrap_or(1.0)),
    };
    tessera::prelude::Rational::new((value * 1_000_000.0).round() as i64, 1_000_000)
}

fn exact_options(
    options: &SampleImportOptions,
    parameter: Parameter,
    value: tessera::prelude::Rational,
) -> Result<SampleImportOptions, String> {
    let value = value.numerator as f64 / value.denominator as f64;
    let mut options = options.clone();
    match parameter {
        Parameter::Root => options.root_pitch = Some(value),
        Parameter::Gain => options.default_gain = Some(value as f32),
    }
    options.validate()?;
    Ok(options)
}

fn exact_change(
    event: On<ValueChange<tessera::prelude::Rational>>,
    editors: Query<&SampleOptionExact>,
    projection: Res<EditorUiProjection>,
    mut texts: Query<&mut Text>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(editor) = editors.get(event.source) else {
        return;
    };
    let Some(sample) = projected(&projection, &editor.output) else {
        return;
    };
    match exact_options(&sample.options, editor.parameter, event.value) {
        Ok(options) => {
            bus.write(EditorCommandBus(EditorCommand::SetSampleOptions {
                sample: sample.sample,
                options,
            }));
        }
        Err(message) => {
            if let Ok(mut text) = texts.get_mut(event.source) {
                crate::infrastructure::ui::widgets::exact_number::reject(
                    &mut text,
                    &mut focus,
                    event.source,
                    &message,
                );
            }
        }
    }
}
fn change(
    event: On<ValueChange<f32>>,
    editors: Query<&SampleOptionSlider>,
    projection: Res<EditorUiProjection>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(editor) = editors.get(event.source) else {
        return;
    };
    let Some(sample) = projected(&projection, &editor.output) else {
        return;
    };
    if !event.value.is_finite() {
        return;
    }
    let mut options = sample.options.clone();
    match editor.parameter {
        Parameter::Root => options.root_pitch = Some(f64::from(event.value.round())),
        Parameter::Gain => options.default_gain = Some((event.value * 100.0).round() / 100.0),
    }
    if options.validate().is_ok() {
        bus.write(EditorCommandBus(EditorCommand::SetSampleOptions {
            sample: sample.sample,
            options,
        }));
    }
}
fn begin(
    event: On<Pointer<DragStart>>,
    editors: Query<&SampleOptionSlider>,
    projection: Res<EditorUiProjection>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if let Ok(editor) = editors.get(event.entity) {
        if let Some(sample) = projected(&projection, &editor.output) {
            bus.write(EditorCommandBus(EditorCommand::BeginSampleOptionsEdit {
                sample: sample.sample,
            }));
        }
    }
}
fn end(_: On<Pointer<DragEnd>>, mut bus: MessageWriter<EditorCommandBus>) {
    bus.write(EditorCommandBus(EditorCommand::EndSampleOptionsEdit));
}
fn toggle_root(
    event: On<Activate>,
    buttons: Query<&ToggleRoot>,
    projection: Res<EditorUiProjection>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let Some(sample) = projected(&projection, &button.0) else {
        return;
    };
    let mut options = sample.options.clone();
    options.root_pitch = if options.root_pitch.is_some() {
        None
    } else {
        Some(60.0)
    };
    bus.write(EditorCommandBus(EditorCommand::SetSampleOptions {
        sample: sample.sample,
        options,
    }));
}
#[cfg(not(target_arch = "wasm32"))]
fn relink(event: On<Activate>, buttons: Query<&Relink>, mut bus: MessageWriter<EditorCommandBus>) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("WAV audio", &["wav"])
        .pick_file()
    {
        bus.write(EditorCommandBus(EditorCommand::RelinkSample {
            sample: button.0,
            path,
        }));
    }
}
pub(crate) fn sync_sample_options_controls(
    projection: Res<EditorUiProjection>,
    editors: Query<(Entity, &SampleOptionSlider, &SliderValue)>,
    mut labels: Query<(&SampleOptionValue, &mut Text), Without<SampleOptionExact>>,
    mut exact: Query<
        (
            Entity,
            &SampleOptionExact,
            &mut crate::infrastructure::ui::widgets::exact_number::ExactNumber,
            &mut Text,
        ),
        Without<SampleOptionValue>,
    >,
    focus: Res<bevy::input_focus::InputFocus>,
    mut commands: Commands,
) {
    if !projection.is_changed() {
        return;
    }
    for (entity, editor, current) in &editors {
        if let Some(sample) = projected(&projection, &editor.output) {
            let target = value(&sample.options, editor.parameter);
            if current.0 != target {
                commands.entity(entity).insert(SliderValue(target));
            }
        }
    }
    for (editor, mut label) in &mut labels {
        if let Some(sample) = projected(&projection, &editor.output) {
            label.0 = value_label(&sample.options, editor.parameter);
        }
    }
    for (entity, editor, mut field, mut text) in &mut exact {
        if let Some(sample) = projected(&projection, &editor.output) {
            crate::infrastructure::ui::widgets::exact_number::update(
                &mut field,
                &mut text,
                exact_value(&sample.options, editor.parameter),
                focus.0 == Some(entity),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_recording_values_preserve_fractional_tuning_and_other_metadata() {
        use tessera::prelude::Rational;
        let original = SampleImportOptions {
            sustain_loop: None,
            root_pitch: Some(60.0),
            default_gain: Some(0.5),
        };
        let tuned = exact_options(&original, Parameter::Root, Rational::new(121, 2)).unwrap();
        assert_eq!(tuned.root_pitch, Some(60.5));
        assert_eq!(tuned.default_gain, original.default_gain);
        let quieter = exact_options(&tuned, Parameter::Gain, Rational::new(1, 8)).unwrap();
        assert_eq!(quieter.default_gain, Some(0.125));
        assert_eq!(quieter.root_pitch, tuned.root_pitch);
        for (parameter, value) in [
            (Parameter::Root, Rational::from_integer(128)),
            (Parameter::Gain, Rational::from_integer(17)),
            (Parameter::Gain, Rational::new(-1, 2)),
        ] {
            assert!(exact_options(&original, parameter, value).is_err());
        }
    }
}
