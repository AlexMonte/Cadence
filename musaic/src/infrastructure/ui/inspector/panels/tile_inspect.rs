//! Tile inspect panel: description, port compass, connect action.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::ThemedText;

use tessera::prelude::NodeId;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::editor::ConnectionEndpointView;
use crate::application::pipeline::scene_sync::surface_content::OwnedCompoundView;
use crate::application::pipeline::ui_projection::{EditorUiProjection, InspectorPanelPaint};
use crate::domain::document::AtomValue;
use crate::infrastructure::ui::widgets::slider;
use bevy_feathers::controls::SliderProps;
use bevy_ui_widgets::{SliderPrecision, SliderStep, SliderValue, ValueChange};

#[derive(Component)]
pub(crate) struct AtomValueEditor {
    node: NodeId,
    template: AtomValue,
}

pub(crate) use super::tile_presentation::inspector_label;
use super::tile_presentation::{
    GlyphPart, InspectorTileGlyph, group_glyph, spawn_container_contents, spawn_owned_groups,
    spawn_selected_header,
};

use crate::adapter::load_up::UiSpriteAssets;
use crate::infrastructure::ui::port_glyphs::spawn_inspector_port_compass;
use crate::infrastructure::ui::widgets::musaic_button;
use crate::infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated};

/// Shares the selected header's live value binding with the connection compass.
pub(crate) fn selected_tile_glyph(node: &NodeId) -> impl Component {
    InspectorTileGlyph {
        node: node.clone(),
        part: GlyphPart::Selected,
    }
}

pub(crate) fn spawn_tile_inspect_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    heading: (&str, &str),
    code: &crate::application::tricks::TileCodePaint,
    selected: &crate::application::pipeline::selected_tile::SelectedTilePaint,
    editable_atom: Option<&AtomValue>,
    compound: Option<&OwnedCompoundView>,
) {
    spawn_selected_header(panel, node, heading.0, heading.1, selected, compound);
    super::tricks::spawn_shared(panel, code);
    if let Some(compound) = compound.filter(|compound| compound.groups.len() > 1) {
        spawn_owned_groups(panel, compound);
    }
    spawn_container_contents(panel, node, &selected.content);
    if let Some((control, bindings)) = &selected.flow {
        super::flow::spawn(panel, node, control, bindings);
    }

    let owned = compound
        .and_then(|c| c.groups.get(c.selected_group))
        .and_then(|g| g.owned_value.as_ref());
    let control = owned
        .map(|v| (&v.node, &v.atom))
        .or_else(|| editable_atom.map(|a| (node, a)));
    if let Some((
        target,
        AtomValue::Modifier(modifier @ tessera::prelude::AtomModifier::Modulation { .. }),
    )) = control
    {
        super::modulation_tiles::spawn(panel, target, modifier);
    }
    if let Some((target, atom)) = control.filter(|(_, atom)| {
        !matches!(
            atom,
            AtomValue::Modifier(tessera::prelude::AtomModifier::Modulation { .. })
        )
    }) {
        if let Some(spec) = numeric_editor(atom) {
            panel.spawn((
                UiText::new(spec.label),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(
                    crate::infrastructure::ui::theme::MusaicUiTheme::default()
                        .chrome
                        .text_main,
                ),
                bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
                ThemedText,
            ));
            panel
                .spawn(slider(
                    SliderProps {
                        value: spec.value,
                        min: spec.min,
                        max: spec.max,
                    },
                    (
                        crate::infrastructure::ui::widgets::ExactSlider,
                        AtomValueEditor {
                            node: target.clone(),
                            template: atom.clone(),
                        },
                        SliderStep(spec.step),
                        SliderPrecision(spec.precision),
                    ),
                ))
                .with_children(|slider| {
                    if let Some(value) = exact_atom_number(atom) {
                        let field = crate::infrastructure::ui::widgets::exact_number::spawn_inline(
                            slider,
                            value,
                            ExactAtomBinding {
                                node: target.clone(),
                            },
                        );
                        slider.commands().entity(field).observe(exact_atom_changed);
                    }
                })
                .observe(on_atom_value_change)
                .observe(on_atom_edit_start)
                .observe(on_atom_edit_end);
        }
        match atom {
            AtomValue::Modifier(modifier) => {
                super::effect_tiles::spawn(panel, target, modifier);
                if let tessera::prelude::AtomModifier::SampleBank(label) = modifier {
                    super::sample_options::spawn_bank_tile(panel, target, label);
                }
                spawn_rhythm_editor(panel, target, modifier);
                super::musical_patterns::spawn(panel, target, modifier);
                if let tessera::prelude::AtomModifier::Slice { index, count } = modifier {
                    spawn_slice_editor(panel, target, *index, *count);
                }
                if let Some(tessera::prelude::FieldValue::Bool { value }) =
                    modifier.parameter_value()
                {
                    if let Ok(next) =
                        modifier.with_parameter_value(tessera::prelude::FieldValue::bool(!value))
                    {
                        atom_choice(
                            panel,
                            target,
                            AtomValue::Modifier(next),
                            &format!(
                                "{}: {} · turn {}",
                                modifier
                                    .parameter_key()
                                    .map(|key| key.spec().label)
                                    .unwrap_or("Enabled"),
                                if value { "on" } else { "off" },
                                if value { "off" } else { "on" }
                            ),
                        );
                    }
                }
                if let Some(key) = modifier.parameter_key() {
                    let spec = key.spec();
                    if let Some(tessera::prelude::FieldValue::Symbol { value }) =
                        modifier.parameter_value()
                    {
                        panel.spawn((
                            UiText::new(format!("{}: {value}", spec.label)),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(
                                crate::infrastructure::ui::theme::MusaicUiTheme::default()
                                    .chrome
                                    .text_main,
                            ),
                            bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
                            ThemedText,
                        ));
                    }
                    if let Some(default) = spec.default {
                        if let Ok(next) = modifier.with_parameter_value(default) {
                            atom_choice(
                                panel,
                                target,
                                AtomValue::Modifier(next),
                                "Reset to default",
                            );
                        }
                    }
                }
            }
            AtomValue::NoteName(_) => {
                use crate::domain::document::NoteName as N;
                panel.spawn((
                    UiText::new("Pitch"),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(
                        crate::infrastructure::ui::theme::MusaicUiTheme::default()
                            .chrome
                            .text_main,
                    ),
                    bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
                    ThemedText,
                ));
                panel
                    .spawn(Node {
                        display: Display::Flex,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|row| {
                        for note in [N::C, N::D, N::E, N::F, N::G, N::A, N::B] {
                            atom_choice(
                                row,
                                target,
                                AtomValue::NoteName(note),
                                &format!("{note:?}"),
                            );
                        }
                    });
            }
            AtomValue::DrumHit(_) => {
                use crate::domain::document::DrumHit;
                panel.spawn((
                    UiText::new("Drum hit"),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(
                        crate::infrastructure::ui::theme::MusaicUiTheme::default()
                            .chrome
                            .text_main,
                    ),
                    bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
                    ThemedText,
                ));
                panel
                    .spawn(Node {
                        display: Display::Flex,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|row| {
                        for hit in DrumHit::ALL {
                            atom_choice(row, target, AtomValue::DrumHit(hit), hit.label());
                        }
                    });
            }
            AtomValue::Accidental(_) => {
                use crate::domain::document::Accidental as A;
                panel.spawn((
                    UiText::new("Accidental"),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(
                        crate::infrastructure::ui::theme::MusaicUiTheme::default()
                            .chrome
                            .text_main,
                    ),
                    bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
                    ThemedText,
                ));
                panel
                    .spawn(Node {
                        display: Display::Flex,
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|row| {
                        for (value, label) in [(A::Flat, "♭ Flat"), (A::Sharp, "♯ Sharp")] {
                            atom_choice(row, target, AtomValue::Accidental(value), label);
                        }
                        row.spawn(musaic_button(
                            Node::default(),
                            InspectorButtonAction(EditorCommand::EditTiles(
                                crate::application::command::editing::TileEdit::RemoveAccidental {
                                    node: target.clone(),
                                },
                            )),
                            "Remove accidental",
                        ))
                        .observe(on_inspector_button_activated);
                    });
            }
            _ => {}
        }
    }
}

pub(crate) fn spawn_connections(
    panel: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    selected: &crate::application::pipeline::selected_tile::SelectedTilePaint,
    ports: Option<&ConnectionEndpointView>,
    images: &Assets<Image>,
    ui_sprites: Option<&UiSpriteAssets>,
) {
    if let Some(view) = ports {
        if selected.flow.is_none() {
            spawn_inspector_port_compass(panel, images, ui_sprites, node, view, &selected.glyph);
        }

        panel
            .spawn(musaic_button(
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                crate::infrastructure::ui::connection_search::ConnectionSource(node.clone()),
                "Connect to tile…",
            ))
            .observe(crate::infrastructure::ui::connection_search::open);
        panel
            .spawn(musaic_button(
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                crate::infrastructure::ui::connection_search::ConnectionSource(node.clone()),
                "Draw cable",
            ))
            .observe(crate::infrastructure::ui::connection_search::point);
    }
}

fn atom_choice(panel: &mut ChildSpawnerCommands<'_>, node: &NodeId, value: AtomValue, label: &str) {
    panel
        .spawn(musaic_button(
            Node {
                min_height: px(28),
                padding: UiRect::horizontal(px(6)),
                ..default()
            },
            InspectorButtonAction(EditorCommand::SetAtomValue {
                node: node.clone(),
                value,
            }),
            label,
        ))
        .observe(on_inspector_button_activated);
}

fn spawn_slice_editor(
    panel: &mut ChildSpawnerCommands<'_>,
    target: &NodeId,
    index: u32,
    count: u32,
) {
    panel.spawn((
        UiText::new(format!("Slice {} of {count}", index + 1)),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(
            crate::infrastructure::ui::theme::MusaicUiTheme::default()
                .chrome
                .text_main,
        ),
        bevy_feathers::theme::ThemeFontColor(bevy_feathers::tokens::TEXT_MAIN),
        ThemedText,
    ));
    panel
        .spawn(Node {
            display: Display::Flex,
            column_gap: px(4),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            for (next, label) in [
                (index.checked_sub(1), "Previous slice"),
                (index.checked_add(1).filter(|v| *v < count), "Next slice"),
            ] {
                if let Some(index) = next {
                    atom_choice(
                        row,
                        target,
                        AtomValue::Modifier(tessera::prelude::AtomModifier::Slice { index, count }),
                        label,
                    );
                }
            }
        });
    panel
        .spawn(Node {
            display: Display::Flex,
            column_gap: px(4),
            row_gap: px(4),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            for next_count in [1, 2, 4, 8, 16, 32, 64] {
                if next_count != count {
                    atom_choice(
                        row,
                        target,
                        AtomValue::Modifier(tessera::prelude::AtomModifier::Slice {
                            index: index.min(next_count - 1),
                            count: next_count,
                        }),
                        &format!("{next_count} slices"),
                    );
                }
            }
        });
}

/// Multi-number rhythm tiles keep all of their operands on the same authored tile.
fn spawn_rhythm_editor(
    panel: &mut ChildSpawnerCommands<'_>,
    target: &NodeId,
    modifier: &tessera::prelude::AtomModifier,
) {
    use tessera::prelude::AtomModifier as M;
    let (pulses, steps, rotation) = match modifier {
        M::Euclid { pulses, steps } => (*pulses, *steps, None),
        M::EuclidRot {
            pulses,
            steps,
            rotation,
        } => (*pulses, *steps, Some(*rotation)),
        _ => return,
    };
    let build = |pulses, steps, rotation| match rotation {
        Some(rotation) => M::EuclidRot {
            pulses,
            steps,
            rotation,
        },
        None => M::Euclid { pulses, steps },
    };
    panel.spawn((
        UiText::new(format!(
            "{pulses} pulses / {steps} steps{}",
            rotation
                .map(|r| format!(" · rotation {r}"))
                .unwrap_or_default()
        )),
        TextFont {
            font_size: 13.0,
            ..default()
        },
    ));
    panel
        .spawn(Node {
            display: Display::Flex,
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(4),
            row_gap: px(4),
            ..default()
        })
        .with_children(|row| {
            for (next, label) in [
                (
                    pulses.checked_sub(1).map(|p| build(p, steps, rotation)),
                    "− Pulse",
                ),
                (
                    pulses
                        .checked_add(1)
                        .filter(|p| *p <= steps)
                        .map(|p| build(p, steps, rotation)),
                    "+ Pulse",
                ),
                (
                    steps
                        .checked_sub(1)
                        .filter(|s| *s > 0 && *s >= pulses)
                        .map(|s| build(pulses, s, rotation)),
                    "− Step",
                ),
                (
                    steps
                        .checked_add(1)
                        .filter(|s| *s <= 1024)
                        .map(|s| build(pulses, s, rotation)),
                    "+ Step",
                ),
                (
                    rotation
                        .and_then(|r| r.checked_sub(1))
                        .map(|r| build(pulses, steps, Some(r))),
                    "← Rotate",
                ),
                (
                    rotation
                        .and_then(|r| r.checked_add(1))
                        .map(|r| build(pulses, steps, Some(r))),
                    "Rotate →",
                ),
            ] {
                if let Some(next) = next {
                    atom_choice(row, target, AtomValue::Modifier(next), label);
                }
            }
        });
}

struct NumericEditorSpec {
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    precision: i32,
    label: String,
}

fn numeric_editor(atom: &AtomValue) -> Option<NumericEditorSpec> {
    use tessera::prelude::{AtomModifier as M, FieldValue, ParameterUnit};
    let scalar =
        |value: tessera::prelude::Rational| value.numerator as f32 / value.denominator as f32;
    let rhythm: Option<(f32, f32, f32, f32, i32, &str)> = match atom {
        AtomValue::Modifier(M::Elongate(value)) => {
            Some((scalar(*value), 0.25, 8.0, 0.25, 2, "Duration / weight"))
        }
        AtomValue::Modifier(M::Replicate(value)) => {
            Some((*value as f32, 1.0, 16.0, 1.0, 0, "Repeat count"))
        }
        AtomValue::Modifier(M::Degrade(value)) => Some((
            value.map(scalar).unwrap_or(0.5),
            0.0,
            1.0,
            0.05,
            2,
            "Drop probability",
        )),
        _ => None,
    };
    if let Some((value, min, max, step, precision, label)) = rhythm {
        return Some(NumericEditorSpec {
            value,
            min: min.min(value),
            max: max.max(value),
            step,
            precision,
            label: label.into(),
        });
    }
    match atom {
        AtomValue::Number(_) | AtomValue::Ratio(_) => {
            let value = scalar(atom.numeric_rational()?);
            Some(NumericEditorSpec {
                value,
                min: value.min(-16.0),
                max: value.max(16.0),
                step: 0.25,
                precision: 2,
                label: "Value".into(),
            })
        }
        AtomValue::Octave(value) => Some(NumericEditorSpec {
            value: f32::from(*value),
            min: -1.0,
            max: 9.0,
            step: 1.0,
            precision: 0,
            label: "Octave".into(),
        }),
        AtomValue::Modifier(modifier) => {
            let key = modifier.parameter_key()?;
            let spec = key.spec();
            let FieldValue::Rational { value } = modifier.parameter_value()? else {
                return None;
            };
            let (min, max) = spec.editor_range?;
            let (step, precision, unit) = match spec.unit {
                ParameterUnit::Hertz => (10.0, 0, "Hz"),
                ParameterUnit::VariantIndex => (1.0, 0, "index"),
                ParameterUnit::Semitones => (1.0, 0, "semitones"),
                ParameterUnit::Seconds => (0.01, 2, "seconds"),
                ParameterUnit::DurationRatio => (0.25, 2, "× duration"),
                ParameterUnit::RateRatio => (0.25, 2, "× rate"),
                ParameterUnit::SourcePosition => (0.01, 2, "source position"),
                ParameterUnit::UnitLevel => (0.01, 2, "0–1"),
                ParameterUnit::PitchBendAmount => (0.05, 2, "−1 to 1 · ±2 semitones"),
                ParameterUnit::StereoPosition => (0.05, 2, "left −1 · center 0 · right 1"),
                ParameterUnit::LinearGain => (0.05, 2, "× level"),
                ParameterUnit::EffectSettings
                | ParameterUnit::Boolean
                | ParameterUnit::BankName
                | ParameterUnit::SliceSelection => return None,
            };
            let value = scalar(value);
            Some(NumericEditorSpec {
                value,
                min: scalar(min).min(value),
                max: scalar(max).max(value),
                step,
                precision,
                label: format!("{} ({unit})", spec.label),
            })
        }
        _ => None,
    }
}

fn edited_numeric_value(template: &AtomValue, value: f32) -> Option<AtomValue> {
    if !value.is_finite() {
        return None;
    }
    if matches!(template, AtomValue::Octave(_)) {
        return Some(AtomValue::Octave(value.round().clamp(-1.0, 9.0) as i8));
    }
    let rational = tessera::prelude::Rational::new((f64::from(value) * 100.0).round() as i64, 100);
    use tessera::prelude::AtomModifier as M;
    match template {
        AtomValue::Modifier(M::Elongate(_)) if value > 0.0 => {
            return Some(AtomValue::Modifier(M::Elongate(rational)));
        }
        AtomValue::Modifier(M::Replicate(_))
            if rational.denominator == 1 && (1..=65_536).contains(&rational.numerator) =>
        {
            return Some(AtomValue::Modifier(M::Replicate(rational.numerator as u32)));
        }
        AtomValue::Modifier(M::Degrade(_)) if (0.0..=1.0).contains(&value) => {
            return Some(AtomValue::Modifier(M::Degrade(Some(rational))));
        }
        _ => {}
    }
    if matches!(template, AtomValue::Modifier(_)) {
        return template
            .with_owned_parameter_value(tessera::prelude::FieldValue::rational(rational))
            .ok();
    }
    match template {
        AtomValue::Number(_) | AtomValue::Ratio(_) if rational.denominator == 1 => {
            i32::try_from(rational.numerator)
                .ok()
                .map(AtomValue::Number)
        }
        AtomValue::Number(_) | AtomValue::Ratio(_) => Some(AtomValue::Ratio(rational)),
        _ => None,
    }
}

fn on_atom_value_change(
    event: On<ValueChange<f32>>,
    editors: Query<&AtomValueEditor>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(editor) = editors.get(event.source) else {
        return;
    };
    let Some(value) = edited_numeric_value(&editor.template, event.value) else {
        return;
    };
    bus.write(EditorCommandBus(EditorCommand::SetAtomValue {
        node: editor.node.clone(),
        value,
    }));
}

fn on_atom_edit_start(
    event: On<Pointer<DragStart>>,
    editors: Query<&AtomValueEditor>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if let Ok(editor) = editors.get(event.entity) {
        bus.write(EditorCommandBus(EditorCommand::BeginAtomEdit {
            node: editor.node.clone(),
        }));
    }
}

fn on_atom_edit_end(_: On<Pointer<DragEnd>>, mut bus: MessageWriter<EditorCommandBus>) {
    bus.write(EditorCommandBus(EditorCommand::EndAtomEdit));
}

/// Controls consume the same projected document values as the rest of the inspector.
pub(crate) fn sync_atom_value_controls(
    projection: Res<EditorUiProjection>,
    editors: Query<(Entity, &AtomValueEditor, &SliderValue)>,
    mut group_labels: Query<(
        &InspectorTileGlyph,
        Option<&mut Text>,
        Option<&mut crate::infrastructure::ui::widgets::ButtonLabel>,
    )>,
    mut commands: Commands,
) {
    if !projection.is_changed() {
        return;
    }
    for (entity, editor, current) in &editors {
        let value = projection
            .inspector
            .panels
            .iter()
            .zip(&projection.inspector_paint.panels)
            .find_map(|(kind, paint)| match (kind, paint) {
                (
                    crate::application::editor::InspectorPanelKind::TileInspectPanel { node },
                    InspectorPanelPaint::TileInspect {
                        editable_atom: Some(atom),
                        ..
                    },
                ) if node == &editor.node => numeric_editor(atom).map(|spec| spec.value),
                (
                    _,
                    InspectorPanelPaint::TileInspect {
                        compound: Some(compound),
                        ..
                    },
                ) => compound
                    .groups
                    .iter()
                    .filter_map(|g| g.owned_value.as_ref())
                    .find(|v| v.node == editor.node)
                    .and_then(|v| numeric_editor(&v.atom).map(|spec| spec.value)),
                _ => None,
            });
        if let Some(value) = value {
            if current.0 != value {
                commands.entity(entity).insert(SliderValue(value));
            }
        }
    }
    for (label, mut text, mut button_label) in &mut group_labels {
        for (kind, paint) in projection
            .inspector
            .panels
            .iter()
            .zip(&projection.inspector_paint.panels)
        {
            if let InspectorPanelPaint::TileInspect {
                selected, compound, ..
            } = paint
            {
                let value = match label.part {
                    GlyphPart::CompoundSummary => compound
                        .as_ref()
                        .map(super::tile_presentation::compound_summary),
                    GlyphPart::CompoundPitch => compound
                        .as_ref()
                        .map(super::tile_presentation::compound_pitch),
                    GlyphPart::Selected => match kind {
                        crate::application::editor::InspectorPanelKind::TileInspectPanel {
                            node,
                        } if node == &label.node => Some(selected.glyph.clone()),
                        _ => None,
                    },
                    part => compound
                        .as_ref()
                        .and_then(|compound| {
                            compound
                                .groups
                                .iter()
                                .find(|group| group.owner == label.node)
                        })
                        .map(|group| group_glyph(group, part)),
                };
                if let Some(value) = value {
                    if let Some(text) = text.as_mut() {
                        if text.0 != value {
                            text.0.clone_from(&value);
                        }
                    }
                    if let Some(text) = button_label.as_mut() {
                        if text.0 != value {
                            text.0 = value;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera::prelude::{AtomModifier as M, ParameterKey, Rational};

    #[test]
    fn owned_sound_slider_keeps_the_parameter_role_and_validates_the_value() {
        let length = AtomValue::Modifier(M::Legato(Rational::one()));
        assert_eq!(
            edited_numeric_value(&length, 1.5),
            Some(AtomValue::Modifier(M::Legato(Rational::new(3, 2))))
        );
        assert_eq!(edited_numeric_value(&length, 0.0), None);
        let sustain = AtomValue::Modifier(M::Sustain(Rational::one()));
        assert_eq!(
            edited_numeric_value(&sustain, 0.25),
            Some(AtomValue::Modifier(M::Sustain(Rational::new(1, 4))))
        );
        assert_eq!(edited_numeric_value(&sustain, 1.5), None);
        let variant = AtomValue::Modifier(M::SampleVariant(2));
        assert_eq!(
            edited_numeric_value(&variant, 7.0),
            Some(AtomValue::Modifier(M::SampleVariant(7)))
        );
        assert_eq!(edited_numeric_value(&variant, 1.5), None);
        assert_eq!(edited_numeric_value(&sustain, f32::NAN), None);
        assert_eq!(
            edited_numeric_value(&AtomValue::Number(2), 1.5),
            Some(AtomValue::Ratio(Rational::new(3, 2)))
        );
    }

    #[test]
    fn owned_sound_slider_uses_catalog_ranges_and_excludes_boolean_and_symbol_values() {
        for modifier in [
            M::Gain(Rational::one()),
            M::Attack(Rational::new(1, 10)),
            M::Decay(Rational::new(1, 4)),
            M::Release(Rational::new(3, 4)),
            M::Transpose(Rational::from_integer(12)),
            M::Pan(Rational::new(-1, 2)),
            M::HighPassCutoff(Rational::from_integer(400)),
            M::HighPassResonance(Rational::new(1, 2)),
            M::Legato(Rational::one()),
            M::Sustain(Rational::one()),
            M::LowPassCutoff(Rational::from_integer(900)),
            M::LowPassResonance(Rational::new(1, 4)),
            M::SampleVariant(7),
        ] {
            let range = modifier
                .parameter_key()
                .unwrap()
                .spec()
                .editor_range
                .unwrap();
            let spec = numeric_editor(&AtomValue::Modifier(modifier)).unwrap();
            assert_eq!(
                spec.min,
                range.0.numerator as f32 / range.0.denominator as f32
            );
            assert_eq!(
                spec.max,
                range.1.numerator as f32 / range.1.denominator as f32
            );
        }
        assert!(numeric_editor(&AtomValue::Modifier(M::Gate(true))).is_none());
        assert!(numeric_editor(&AtomValue::Modifier(M::SampleBank("drums".into()))).is_none());
        let cutoff = numeric_editor(&AtomValue::Modifier(M::LowPassCutoff(
            Rational::from_integer(25_000),
        )))
        .unwrap();
        assert_eq!(
            cutoff.max, 25_000.0,
            "an imported valid value stays reachable even above the suggested range"
        );
        assert!(cutoff.label.contains("Hz"));
        assert_eq!(
            ParameterKey::Gate.spec().default,
            Some(tessera::prelude::FieldValue::bool(true))
        );
    }
}

#[derive(Component)]
pub(crate) struct ExactAtomBinding {
    node: NodeId,
}
fn exact_atom_value(
    atom: &AtomValue,
    value: tessera::prelude::Rational,
) -> Result<AtomValue, &'static str> {
    use tessera::prelude::{AtomModifier as M, FieldValue, Rational};
    Ok(match atom {
        AtomValue::Number(_) | AtomValue::Ratio(_) => {
            if value.denominator == 1 {
                i32::try_from(value.numerator)
                    .map(AtomValue::Number)
                    .unwrap_or(AtomValue::Ratio(value))
            } else {
                AtomValue::Ratio(value)
            }
        }
        AtomValue::Octave(_) => {
            if value.denominator != 1 || !(-1..=9).contains(&value.numerator) {
                return Err("Octave must be a whole number from -1 to 9");
            }
            AtomValue::Octave(value.numerator as i8)
        }
        AtomValue::Modifier(M::Elongate(_)) => {
            if value <= Rational::zero() {
                return Err("Weight must be positive");
            }
            AtomValue::Modifier(M::Elongate(value))
        }
        AtomValue::Modifier(M::Replicate(_)) => {
            if value.denominator != 1 || !(1..=1024).contains(&value.numerator) {
                return Err("Repeat count must be a whole number from 1 to 1024");
            }
            AtomValue::Modifier(M::Replicate(value.numerator as u32))
        }
        AtomValue::Modifier(M::Degrade(_)) => {
            if value < Rational::zero() || value > Rational::one() {
                return Err("Probability must be between 0 and 1");
            }
            AtomValue::Modifier(M::Degrade(Some(value)))
        }
        AtomValue::Modifier(modifier) => {
            AtomValue::Modifier(modifier.with_parameter_value(FieldValue::rational(value))?)
        }
        _ => return Err("This tile does not take a numeric value"),
    })
}
fn exact_atom_number(atom: &AtomValue) -> Option<tessera::prelude::Rational> {
    use tessera::prelude::{AtomModifier as M, Rational};
    match atom {
        AtomValue::Octave(v) => Some(Rational::from_integer(i64::from(*v))),
        AtomValue::Modifier(M::Elongate(v)) => Some(*v),
        AtomValue::Modifier(M::Replicate(v)) => Some(Rational::from_integer(i64::from(*v))),
        AtomValue::Modifier(M::Degrade(v)) => Some(v.unwrap_or(Rational::new(1, 2))),
        _ => atom
            .numeric_rational()
            .or_else(|| atom.owned_numeric_rational()),
    }
}
fn exact_atom_changed(
    event: On<bevy_ui_widgets::ValueChange<tessera::prelude::Rational>>,
    bindings: Query<&ExactAtomBinding>,
    project: Res<crate::application::session::MusaicProject>,
    mut fields: Query<
        &mut Text,
        With<crate::infrastructure::ui::widgets::exact_number::ExactNumber>,
    >,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(binding) = bindings.get(event.source) else {
        return;
    };
    let Some(crate::domain::document::DocumentNodeKind::Atom(atom)) =
        project.document.graph.node(&binding.node).map(|n| &n.kind)
    else {
        return;
    };
    match exact_atom_value(&atom.atom, event.value) {
        Ok(value) => {
            bus.write(EditorCommandBus(EditorCommand::SetAtomValue {
                node: binding.node.clone(),
                value,
            }));
        }
        Err(error) => {
            if let Ok(mut text) = fields.get_mut(event.source) {
                crate::infrastructure::ui::widgets::exact_number::reject(
                    &mut text,
                    &mut focus,
                    event.source,
                    error,
                );
            }
        }
    }
}
pub(crate) fn sync_exact_atoms(
    project: Res<crate::application::session::MusaicProject>,
    focus: Res<bevy::input_focus::InputFocus>,
    mut fields: Query<(
        Entity,
        &ExactAtomBinding,
        &mut crate::infrastructure::ui::widgets::exact_number::ExactNumber,
        &mut Text,
    )>,
) {
    for (entity, binding, mut field, mut text) in &mut fields {
        if let Some(crate::domain::document::DocumentNodeKind::Atom(atom)) =
            project.document.graph.node(&binding.node).map(|n| &n.kind)
        {
            if let Some(value) = exact_atom_number(&atom.atom) {
                crate::infrastructure::ui::widgets::exact_number::update(
                    &mut field,
                    &mut text,
                    value,
                    focus.0 == Some(entity),
                );
            }
        }
    }
}
