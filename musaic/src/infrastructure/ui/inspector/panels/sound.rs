//! Sound controls consume projected data and emit editor commands only.

use bevy::{input_focus::InputFocus, prelude::*, ui::widget::Text as UiText};
use bevy_feathers::{controls::SliderProps, theme::ThemedText};
use bevy_ui_widgets::{
    Activate, SliderPrecision, SliderRange, SliderStep, SliderValue, ValueChange,
};
use tessera::prelude::{NodeId, Rational};

use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        pipeline::{
            sound::SoundPaint,
            ui_projection::{EditorUiProjection, InspectorPanelPaint},
        },
    },
    domain::instrument::{InstrumentDefinition, InstrumentSource, SynthPreset, Waveform},
    infrastructure::ui::{
        InspectorButtonAction, on_inspector_button_activated,
        theme::MusaicUiTheme,
        widgets::{exact_number, musaic_button, slider},
    },
};

#[derive(Component)]
#[cfg(not(target_arch = "wasm32"))]
struct ImportOutputSample(NodeId);

#[derive(Component)]
struct OutputSourceButton {
    output: NodeId,
    source: InstrumentSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SoundParameter {
    Rate,
    Start,
    End,
    Gain,
    Attack,
    Decay,
    Sustain,
    Release,
    DelayAmount,
    DelayTime,
    DelayFeedback,
    DelayDamping,
    ReverbAmount,
    ReverbDecay,
    ReverbDamping,
    CompressorThreshold,
    CompressorRatio,
    CompressorKnee,
    CompressorAttack,
    CompressorRelease,
    Effect(usize, super::instrument_shape::EffectParameter),
}

#[derive(Component)]
pub(crate) struct SoundParameterEditor {
    output: NodeId,
    parameter: SoundParameter,
}

#[derive(Component)]
pub(crate) struct SoundParameterExact(SoundParameterEditor);

#[derive(Component)]
pub(crate) struct ReverseSoundButton(NodeId);

#[derive(Component)]
pub(crate) struct WaveformRegionShade {
    output: NodeId,
    before: bool,
}

pub(super) fn label(parent: &mut ChildSpawnerCommands<'_>, text: impl Into<String>, size: f32) {
    parent.spawn((
        UiText::new(text.into()),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(MusaicUiTheme::default_dark().chrome.text_main),
        bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
        ThemedText,
    ));
}

fn sound_button(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    current: &InstrumentDefinition,
    source: InstrumentSource,
    name: &str,
) -> Entity {
    let theme = MusaicUiTheme::default_dark();
    let active = current.source == source;
    parent
        .spawn(musaic_button(
            Node {
                min_height: px(26.0),
                padding: UiRect::axes(px(7.0), px(3.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(4.0)),
                ..default()
            },
            (
                BackgroundColor(if active {
                    Color::srgb(0.84, 0.91, 0.86)
                } else {
                    theme.chrome.panel_bg
                }),
                BorderColor::all(if active {
                    Color::srgb(0.16, 0.42, 0.31)
                } else {
                    theme.chrome.button_border
                }),
                OutputSourceButton {
                    output: output.clone(),
                    source,
                },
            ),
            name,
        ))
        .observe(on_source_change)
        .id()
}

fn on_source_change(
    event: On<Activate>,
    buttons: Query<&OutputSourceButton>,
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
    instrument.source = button.source.clone();
    bus.write(EditorCommandBus(EditorCommand::SetSound {
        sound: button.output.clone(),
        definition: instrument,
    }));
}

pub(super) fn choice_row(
    parent: &mut ChildSpawnerCommands<'_>,
    title: &str,
    content: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: px(4.0),
            row_gap: px(4.0),
            ..default()
        })
        .with_children(|row| {
            label(row, title, 11.0);
            content(row);
        });
}

pub(crate) fn spawn_sound_panel(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    sound: &SoundPaint,
) {
    label(parent, "Sound", 14.0);
    label(
        parent,
        "Exact fields accept numbers or fractions · Enter to apply",
        10.0,
    );
    choice_row(parent, "Synth", |row| {
        for (waveform, name) in [
            (Waveform::Sine, "Sine"),
            (Waveform::Triangle, "Triangle"),
            (Waveform::Saw, "Saw"),
            (Waveform::Square, "Square"),
            (Waveform::Noise, "Noise"),
        ] {
            sound_button(
                row,
                output,
                &sound.instrument,
                InstrumentSource::Synth(waveform),
                name,
            );
        }
    });
    choice_row(parent, "Presets", |row| {
        for (preset, name) in [
            (SynthPreset::Bass, "Bass"),
            (SynthPreset::Pad, "Pad"),
            (SynthPreset::Percussion, "Percussion"),
        ] {
            sound_button(
                row,
                output,
                &sound.instrument,
                InstrumentSource::Preset(preset),
                name,
            );
        }
    });
    choice_row(parent, "Drums", |row| {
        sound_button(
            row,
            output,
            &sound.instrument,
            InstrumentSource::Kit,
            "Kit (named hits)",
        );
        for (name, title) in [
            ("bd", "Kick"),
            ("sd", "Snare"),
            ("hh", "Closed hat"),
            ("oh", "Open hat"),
        ] {
            sound_button(
                row,
                output,
                &sound.instrument,
                InstrumentSource::Drum(name.into()),
                title,
            );
        }
    });
    parent
        .spawn(Node {
            display: Display::Flex,
            column_gap: px(6.0),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            #[cfg(not(target_arch = "wasm32"))]
            row.spawn(musaic_button(
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(px(8.0)),
                    ..default()
                },
                ImportOutputSample(output.clone()),
                "Import WAV / instrument…",
            ))
            .observe(on_import_output_sample);
            row.spawn(musaic_button(
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(px(8.0)),
                    ..default()
                },
                InspectorButtonAction(EditorCommand::AuditionSound {
                    sound: output.clone(),
                }),
                "Preview sound",
            ))
            .observe(on_inspector_button_activated);
        });
    if !sound.samples.is_empty() {
        let filter = super::name_filter::spawn(parent, "Search samples…");
        parent
            .spawn((
                Node {
                    max_height: px(90.0),
                    min_height: px(28.0),
                    flex_shrink: 0.0,
                    display: Display::Flex,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(4.0),
                    row_gap: px(4.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                super::name_filter::FilterList(filter),
            ))
            .observe(on_sample_list_scroll)
            .with_children(|list| {
                for sample in &sound.samples {
                    let title = if sample.available {
                        sample.label.clone()
                    } else {
                        format!("{} · missing", sample.label)
                    };
                    let button = sound_button(
                        list,
                        output,
                        &sound.instrument,
                        InstrumentSource::Sample(sample.id),
                        &title,
                    );
                    list.commands()
                        .entity(button)
                        .insert(super::name_filter::FilterItem::new(filter, &title));
                }
            });
    }
    label(parent, &sound.details, 11.0);
    if let Some(message) = &sound.diagnostic {
        parent.spawn((
            UiText::new(message.clone()),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgb(0.65, 0.20, 0.16)),
        ));
    }
    if let Some(sample) = &sound.sample_options {
        super::sample_options::spawn_sample_options_panel(parent, output, sample);
    }
    if !sound.waveform.is_empty() {
        spawn_waveform(parent, output, sound);
    }
    if !matches!(
        sound.instrument.source,
        InstrumentSource::Synth(_) | InstrumentSource::Preset(_)
    ) {
        label(parent, "Sample speed changes pitch", 11.0);
        spawn_parameter(
            parent,
            output,
            SoundParameter::Rate,
            "Speed",
            sound.instrument.rate,
            0.05,
            value(sound.instrument.rate).max(4.0),
            0.05,
            2,
        );
        spawn_parameter(
            parent,
            output,
            SoundParameter::Start,
            "Start",
            sound.instrument.start,
            0.0,
            0.99,
            0.01,
            2,
        );
        spawn_parameter(
            parent,
            output,
            SoundParameter::End,
            "End",
            sound.instrument.end,
            0.01,
            1.0,
            0.01,
            2,
        );
        parent
            .spawn(musaic_button(
                Node {
                    min_height: px(26.0),
                    padding: UiRect::horizontal(px(8.0)),
                    ..default()
                },
                ReverseSoundButton(output.clone()),
                reverse_label(sound.instrument.reverse),
            ))
            .observe(on_reverse_sound);
    }
    super::super::view_memory::section(
        parent,
        output,
        super::super::view_memory::SectionKind::SoundShaping,
        |body| super::instrument_shape::spawn_sound_design(body, output, sound),
    );
    super::super::view_memory::section(
        parent,
        output,
        super::super::view_memory::SectionKind::SavedSounds,
        |body| super::sound_library::spawn_library(body, output, sound),
    );
}

pub(super) fn spawn_parameter(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    parameter: SoundParameter,
    title: &str,
    current: Rational,
    min: f32,
    max: f32,
    step: f32,
    precision: i32,
) {
    parent
        .spawn(Node {
            display: Display::Flex,
            align_items: AlignItems::Center,
            column_gap: px(7.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                UiText::new(title.to_owned()),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                Node {
                    width: px(66.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                TextColor(MusaicUiTheme::default_dark().chrome.text_main),
                bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
                ThemedText,
            ));
            row.spawn(slider(
                SliderProps {
                    value: value(current),
                    min: min.min(value(current)),
                    max: max.max(value(current)),
                },
                (
                    crate::infrastructure::ui::widgets::ExactSlider,
                    SoundParameterEditor {
                        output: output.clone(),
                        parameter,
                    },
                    SliderStep(step),
                    SliderPrecision(precision),
                ),
            ))
            .with_children(|slider| {
                let field = exact_number::spawn_inline(
                    slider,
                    current,
                    SoundParameterExact(SoundParameterEditor {
                        output: output.clone(),
                        parameter,
                    }),
                );
                slider
                    .commands()
                    .entity(field)
                    .observe(on_sound_exact_change);
            })
            .observe(on_sound_parameter_change)
            .observe(on_sound_edit_start)
            .observe(on_sound_edit_end);
        });
}

fn spawn_waveform(parent: &mut ChildSpawnerCommands<'_>, output: &NodeId, sound: &SoundPaint) {
    parent
        .spawn((
            Node {
                height: px(52.0),
                width: percent(100.0),
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.97, 0.97, 0.94)),
            BorderColor::all(Color::srgb(0.74, 0.79, 0.73)),
        ))
        .with_children(|waveform| {
            waveform.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(50.0),
                    width: percent(100.0),
                    height: px(1.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.74, 0.79, 0.73)),
            ));
            let count = sound.waveform.len() as f32;
            for (index, peak) in sound.waveform.iter().enumerate() {
                let height = *peak as f32 / u16::MAX as f32 * 90.0;
                waveform.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(index as f32 * 100.0 / count),
                        top: percent(50.0 - height * 0.5),
                        width: percent(80.0 / count),
                        height: percent(height),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.16, 0.42, 0.31)),
                ));
            }
            for before in [true, false] {
                waveform.spawn((
                    WaveformRegionShade {
                        output: output.clone(),
                        before,
                    },
                    region_shade_node(&sound.instrument, before),
                    BackgroundColor(Color::srgba(0.97, 0.97, 0.94, 0.78)),
                ));
            }
        });
}

fn region_shade_node(instrument: &InstrumentDefinition, before: bool) -> Node {
    Node {
        position_type: PositionType::Absolute,
        top: px(0.0),
        bottom: px(0.0),
        left: if before {
            percent(0.0)
        } else {
            percent(value(instrument.end) * 100.0)
        },
        width: percent(if before {
            value(instrument.start) * 100.0
        } else {
            (1.0 - value(instrument.end)) * 100.0
        }),
        ..default()
    }
}

fn value(rational: Rational) -> f32 {
    rational.numerator as f32 / rational.denominator as f32
}
fn reverse_label(reverse: bool) -> &'static str {
    if reverse {
        "Reverse: on"
    } else {
        "Reverse: off"
    }
}

pub(super) fn projected_sound<'a>(
    projection: &'a EditorUiProjection,
    output: &NodeId,
) -> Option<&'a SoundPaint> {
    projection
        .inspector
        .panels
        .iter()
        .zip(&projection.inspector_paint.panels)
        .find_map(|(kind, paint)| match (kind, paint) {
            (
                crate::application::editor::InspectorPanelKind::TileInspectPanel { node },
                InspectorPanelPaint::TileInspect {
                    sound: Some(sound), ..
                },
            ) if node == output => Some(sound.as_ref()),
            _ => None,
        })
}

fn on_sound_parameter_change(
    event: On<ValueChange<f32>>,
    editors: Query<&SoundParameterEditor>,
    projection: Res<EditorUiProjection>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(editor) = editors.get(event.source) else {
        return;
    };
    let Some(sound) = projected_sound(&projection, &editor.output) else {
        return;
    };
    if !event.value.is_finite() {
        return;
    }
    let mut instrument = sound.instrument.clone();
    let changed = Rational::new((event.value * 1000.0).round() as i64, 1000);
    editor.parameter.set(&mut instrument, changed);
    if instrument.validate().is_ok() {
        bus.write(EditorCommandBus(EditorCommand::SetSound {
            sound: editor.output.clone(),
            definition: instrument,
        }));
    }
}

fn on_sound_exact_change(
    event: On<ValueChange<Rational>>,
    editors: Query<&SoundParameterExact>,
    projection: Res<EditorUiProjection>,
    mut texts: Query<&mut Text>,
    mut focus: ResMut<InputFocus>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(SoundParameterExact(editor)) = editors.get(event.source) else {
        return;
    };
    let Some(sound) = projected_sound(&projection, &editor.output) else {
        return;
    };
    let mut instrument = sound.instrument.clone();
    editor.parameter.set(&mut instrument, event.value);
    match instrument.validate() {
        Ok(()) if instrument != sound.instrument => {
            bus.write(EditorCommandBus(EditorCommand::SetSound {
                sound: editor.output.clone(),
                definition: instrument,
            }));
        }
        Ok(()) => {}
        Err(message) => {
            if let Ok(mut text) = texts.get_mut(event.source) {
                exact_number::reject(&mut text, &mut focus, event.source, &message);
            }
        }
    }
}

fn on_sound_edit_start(
    event: On<Pointer<DragStart>>,
    editors: Query<&SoundParameterEditor>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if let Ok(editor) = editors.get(event.entity) {
        bus.write(EditorCommandBus(EditorCommand::BeginSoundEdit {
            sound: editor.output.clone(),
        }));
    }
}
fn on_sound_edit_end(_: On<Pointer<DragEnd>>, mut bus: MessageWriter<EditorCommandBus>) {
    bus.write(EditorCommandBus(EditorCommand::EndSoundEdit));
}

fn on_reverse_sound(
    event: On<Activate>,
    buttons: Query<&ReverseSoundButton>,
    projection: Res<EditorUiProjection>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let Some(sound) = projected_sound(&projection, &button.0) else {
        return;
    };
    let mut instrument = sound.instrument.clone();
    instrument.reverse = !instrument.reverse;
    bus.write(EditorCommandBus(EditorCommand::SetSound {
        sound: button.0.clone(),
        definition: instrument,
    }));
}

#[cfg(not(target_arch = "wasm32"))]
fn on_import_output_sample(
    event: On<Activate>,
    buttons: Query<&ImportOutputSample>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("WAV or instrument bank", &["wav", "json"])
        .pick_file()
    {
        bus.write(EditorCommandBus(EditorCommand::ImportSample {
            path,
            sound: Some(button.0.clone()),
        }));
    }
}

pub(super) fn on_sample_list_scroll(
    mut event: On<Pointer<Scroll>>,
    mut lists: Query<(&ComputedNode, &mut ScrollPosition)>,
) {
    let Ok((computed, mut position)) = lists.get_mut(event.entity) else {
        return;
    };
    let scale = match event.unit {
        bevy::input::mouse::MouseScrollUnit::Line => 28.0,
        bevy::input::mouse::MouseScrollUnit::Pixel => 1.0,
    };
    let maximum = ((computed.content_size().y - computed.size().y)
        * computed.inverse_scale_factor())
    .max(0.0);
    position.y = (position.y - event.y * scale).clamp(0.0, maximum);
    event.propagate(false);
}

pub(crate) fn sync_sound_controls(
    projection: Res<EditorUiProjection>,
    editors: Query<(Entity, &SoundParameterEditor, &SliderValue, &SliderRange)>,
    mut reverse: Query<(
        &ReverseSoundButton,
        &mut crate::infrastructure::ui::widgets::ButtonLabel,
    )>,
    mut shades: Query<(&WaveformRegionShade, &mut Node)>,
    mut exact: Query<(
        Entity,
        &SoundParameterExact,
        &mut exact_number::ExactNumber,
        &mut Text,
    )>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    if !projection.is_changed() {
        return;
    }
    for (entity, editor, current, range) in &editors {
        if let Some(sound) = projected_sound(&projection, &editor.output) {
            let target = editor.parameter.value(&sound.instrument);
            if current.0 != target {
                commands.entity(entity).insert(SliderValue(target));
            }
            if target < range.start() || target > range.end() {
                commands.entity(entity).insert(SliderRange::new(
                    range.start().min(target),
                    range.end().max(target),
                ));
            }
        }
    }
    for (entity, SoundParameterExact(editor), mut field, mut text) in &mut exact {
        if let Some(sound) = projected_sound(&projection, &editor.output) {
            exact_number::update(
                &mut field,
                &mut text,
                editor.parameter.exact_value(&sound.instrument),
                focus.0 == Some(entity),
            );
        }
    }
    for (button, mut text) in &mut reverse {
        if let Some(sound) = projected_sound(&projection, &button.0) {
            text.0 = reverse_label(sound.instrument.reverse).into();
        }
    }
    for (shade, mut node) in &mut shades {
        if let Some(sound) = projected_sound(&projection, &shade.output) {
            *node = region_shade_node(&sound.instrument, shade.before);
        }
    }
}

impl SoundParameter {
    pub(super) fn value(self, instrument: &InstrumentDefinition) -> f32 {
        value(self.exact_value(instrument))
    }
    pub(super) fn exact_value(self, instrument: &InstrumentDefinition) -> Rational {
        use SoundParameter as P;
        let envelope = instrument.effective_envelope();
        let delay = instrument.sound.delay.clone().unwrap_or_default();
        let reverb = instrument.sound.reverb.clone().unwrap_or_default();
        let compressor = instrument.sound.compressor.clone().unwrap_or_default();
        match self {
            P::Rate => instrument.rate,
            P::Start => instrument.start,
            P::End => instrument.end,
            P::Gain => instrument.effective_gain(),
            P::Attack => envelope.attack,
            P::Decay => envelope.decay,
            P::Sustain => envelope.sustain,
            P::Release => envelope.release,
            P::DelayAmount => delay.amount,
            P::DelayTime => delay.time,
            P::DelayFeedback => delay.feedback,
            P::DelayDamping => delay.damping,
            P::ReverbAmount => reverb.amount,
            P::ReverbDecay => reverb.decay,
            P::ReverbDamping => reverb.damping,
            P::CompressorThreshold => compressor.threshold,
            P::CompressorRatio => compressor.ratio,
            P::CompressorKnee => compressor.knee_db,
            P::CompressorAttack => compressor.attack,
            P::CompressorRelease => compressor.release,
            P::Effect(index, parameter) => {
                let mut copy = instrument.sound.effects.get(index).cloned();
                copy.as_mut()
                    .and_then(|effect| effect_parameter(effect, parameter).copied())
                    .unwrap_or(Rational::zero())
            }
        }
    }
    fn set(self, instrument: &mut InstrumentDefinition, changed: Rational) {
        use SoundParameter as P;
        match self {
            P::Rate => instrument.rate = changed,
            P::Start => instrument.start = changed,
            P::End => instrument.end = changed,
            P::Gain => instrument.sound.gain = Some(changed),
            P::Attack | P::Decay | P::Sustain | P::Release => {
                let mut envelope = instrument.effective_envelope();
                match self {
                    P::Attack => envelope.attack = changed,
                    P::Decay => envelope.decay = changed,
                    P::Sustain => envelope.sustain = changed,
                    P::Release => envelope.release = changed,
                    _ => unreachable!(),
                }
                instrument.sound.envelope = Some(envelope);
            }
            P::DelayAmount | P::DelayTime | P::DelayFeedback | P::DelayDamping => {
                if let Some(delay) = &mut instrument.sound.delay {
                    match self {
                        P::DelayAmount => delay.amount = changed,
                        P::DelayTime => delay.time = changed,
                        P::DelayFeedback => delay.feedback = changed,
                        P::DelayDamping => delay.damping = changed,
                        _ => unreachable!(),
                    }
                }
            }
            P::ReverbAmount | P::ReverbDecay | P::ReverbDamping => {
                if let Some(reverb) = &mut instrument.sound.reverb {
                    match self {
                        P::ReverbAmount => reverb.amount = changed,
                        P::ReverbDecay => reverb.decay = changed,
                        P::ReverbDamping => reverb.damping = changed,
                        _ => unreachable!(),
                    }
                }
            }
            P::CompressorThreshold
            | P::CompressorRatio
            | P::CompressorKnee
            | P::CompressorAttack
            | P::CompressorRelease => {
                if let Some(compressor) = &mut instrument.sound.compressor {
                    match self {
                        P::CompressorThreshold => compressor.threshold = changed,
                        P::CompressorRatio => compressor.ratio = changed,
                        P::CompressorKnee => compressor.knee_db = changed,
                        P::CompressorAttack => compressor.attack = changed,
                        P::CompressorRelease => compressor.release = changed,
                        _ => unreachable!(),
                    }
                }
            }
            P::Effect(index, parameter) => {
                if let Some(value) = instrument
                    .sound
                    .effects
                    .get_mut(index)
                    .and_then(|effect| effect_parameter(effect, parameter))
                {
                    *value = changed;
                }
            }
        }
    }
}
fn effect_parameter(
    effect: &mut crate::domain::instrument::InstrumentEffect,
    parameter: super::instrument_shape::EffectParameter,
) -> Option<&mut Rational> {
    use super::instrument_shape::EffectParameter as P;
    use crate::domain::instrument::InstrumentEffect as F;
    match (effect, parameter) {
        (F::LowPass { cutoff_hz, .. } | F::HighPass { cutoff_hz, .. }, P::Cutoff) => {
            Some(cutoff_hz)
        }
        (F::LowPass { resonance, .. } | F::HighPass { resonance, .. }, P::Resonance) => {
            Some(resonance)
        }
        (F::Drive { amount, .. }, P::Amount) => Some(amount),
        (F::Drive { wet, .. }, P::Wet) => Some(wet),
        (F::Drive { output_gain, .. }, P::OutputGain) => Some(output_gain),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            editor::{EditorPlugin, SelectionMode},
            history::CommandHistory,
            pipeline::PlaybackPlugin,
            session::MusaicProject,
        },
        domain::{document::DocumentNodeKind, instrument::InstrumentEffect},
        infrastructure::app::{AppState, TransportMode},
    };

    fn send(app: &mut App, command: EditorCommand) {
        app.world_mut().write_message(EditorCommandBus(command));
        app.update();
    }
    pub(super) fn refresh_test_projection(world: &mut World) {
        let project = world.resource::<MusaicProject>();
        let output = project
            .document
            .graph
            .nodes_on_surface(project.document.root_surface)
            .into_iter()
            .find(|(_, node)| matches!(&node.kind, DocumentNodeKind::Sound(_)))
            .unwrap()
            .1
            .id
            .clone();
        let sound = crate::application::pipeline::sound::sound_paint(project, &output).unwrap();
        let mut projection = world.resource_mut::<EditorUiProjection>();
        projection.inspector = crate::application::editor::InspectorLayout::single(
            crate::application::editor::InspectorPanelKind::TileInspectPanel { node: output },
        );
        projection.inspector_paint.panels = vec![InspectorPanelPaint::TileInspect {
            title: "Output".into(),
            selected: Default::default(),
            code: Default::default(),
            description: String::new(),
            editable_atom: None,
            compound: None,
            ports: None,
            sound: Some(Box::new(sound)),
            panel_title: "Output".into(),
        }];
    }
    #[test]
    fn sound_controls_emit_undoable_edits_and_source_changes_preserve_design() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_state::<TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((EditorPlugin, PlaybackPlugin));
        app.insert_state(AppState::Editor)
            .add_systems(Last, refresh_test_projection);
        let mut project = MusaicProject::demo();
        let output = project
            .document
            .graph
            .nodes_on_surface(project.document.root_surface)
            .into_iter()
            .find(|(_, node)| matches!(&node.kind, DocumentNodeKind::Sound(_)))
            .unwrap()
            .1
            .id
            .clone();
        let mut instrument = InstrumentDefinition::default();
        instrument.sound.effects = vec![
            InstrumentEffect::LowPass {
                cutoff_hz: Rational::new(500, 1),
                resonance: Rational::zero(),
            },
            InstrumentEffect::Drive {
                amount: Rational::new(1, 2),
                wet: Rational::one(),
                output_gain: Rational::one(),
            },
        ];
        project
            .document
            .graph
            .set_sound_definition(&output, instrument.clone())
            .unwrap();
        app.insert_resource(project);
        send(
            &mut app,
            EditorCommand::SelectNode {
                node: output.clone(),
                mode: SelectionMode::Replace,
            },
        );
        let sound = crate::application::pipeline::sound::sound_paint(
            app.world().resource::<MusaicProject>(),
            &output,
        )
        .unwrap();
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|parent| spawn_sound_panel(parent, &output, &sound));
        app.world_mut().flush();
        let attack = app
            .world_mut()
            .query::<(Entity, &SoundParameterEditor)>()
            .iter(app.world())
            .find(|(_, editor)| matches!(editor.parameter, SoundParameter::Attack))
            .unwrap()
            .0;
        send(
            &mut app,
            EditorCommand::BeginSoundEdit {
                sound: output.clone(),
            },
        );
        for value in [0.1_f32, 0.2, 0.3] {
            app.world_mut().trigger(ValueChange {
                source: attack,
                value,
            });
            app.update();
        }
        send(&mut app, EditorCommand::EndSoundEdit);
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .sound_definition(&output)
                .unwrap()
                .effective_envelope()
                .attack,
            Rational::new(3, 10)
        );
        assert_eq!(
            app.world().resource::<CommandHistory>().undo_len(),
            1,
            "a slider gesture is one undo step"
        );
        send(&mut app, EditorCommand::Undo);
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .sound_definition(&output)
                .unwrap(),
            &instrument
        );
        send(&mut app, EditorCommand::Redo);
        let up = app
            .world_mut()
            .query::<(Entity, &crate::infrastructure::ui::widgets::ButtonLabel)>()
            .iter(app.world())
            .find(|(_, label)| label.0 == "↑")
            .unwrap()
            .0;
        app.world_mut().trigger(Activate { entity: up });
        app.update();
        assert!(matches!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .sound_definition(&output)
                .unwrap()
                .sound
                .effects[0],
            InstrumentEffect::Drive { .. }
        ));
        send(&mut app, EditorCommand::Undo);
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .sound_definition(&output)
                .unwrap()
                .sound
                .effects,
            instrument.sound.effects
        );
        // Sound source buttons must read current projected settings at click time.
        let square = app
            .world_mut()
            .query::<(Entity, &crate::infrastructure::ui::widgets::ButtonLabel)>()
            .iter(app.world())
            .find(|(_, label)| label.0 == "Square")
            .unwrap()
            .0;
        app.world_mut().trigger(Activate { entity: square });
        app.update();
        let selected = app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .sound_definition(&output)
            .unwrap();
        assert_eq!(selected.source, InstrumentSource::Synth(Waveform::Square));
        assert_eq!(selected.sound.effects, instrument.sound.effects);
        assert_eq!(selected.effective_envelope().attack, Rational::new(3, 10));
        let save = app
            .world_mut()
            .query::<(Entity, &crate::infrastructure::ui::widgets::ButtonLabel)>()
            .iter(app.world())
            .find(|(_, label)| label.0 == "Save sound")
            .unwrap()
            .0;
        app.world_mut().trigger(Activate { entity: save });
        app.update();
        let project = app.world().resource::<MusaicProject>();
        assert_eq!(
            project.sound_library["Sound 1"],
            *project.document.graph.sound_definition(&output).unwrap(),
            "saving uses the most recent Sound tile definition"
        );
    }
}

#[cfg(test)]
#[path = "sound_exact_tests.rs"]
mod exact_tests;
