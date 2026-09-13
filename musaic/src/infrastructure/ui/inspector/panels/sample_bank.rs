//! Imported banks remain ordered references to project-owned recordings.
use super::*;
use crate::{
    domain::project::samples::{SampleBankDefinition, SamplePitchZone, SampleVariant},
    infrastructure::ui::{InspectorButtonAction, on_inspector_button_activated},
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
};

fn row() -> Node {
    Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: px(4),
        row_gap: px(4),
        align_items: AlignItems::Center,
        width: percent(100),
        ..default()
    }
}

fn button(
    parent: &mut ChildSpawnerCommands<'_>,
    text: impl Into<String>,
    sample: SampleId,
    definition: Option<SampleBankDefinition>,
) {
    parent
        .spawn(musaic_button(
            Node {
                min_height: px(26),
                padding: UiRect::axes(px(6), px(3)),
                border: UiRect::all(px(1)),
                ..default()
            },
            InspectorButtonAction(EditorCommand::SetSampleBank { sample, definition }),
            text,
        ))
        .observe(on_inspector_button_activated);
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, sample: &SampleOptionsPaint) {
    label(parent, "Imported sample bank");
    if let Some(lead) = &sample.member_of {
        label(
            parent,
            &format!(
                "This recording belongs to {lead}. Select that lead recording to edit the bank."
            ),
        );
        return;
    }
    let Some(bank) = &sample.bank else {
        button(
            parent,
            "Create bank from imported samples",
            sample.sample,
            Some(SampleBankDefinition::new(sample.sample)),
        );
        return;
    };
    label(
        parent,
        "Each variant is an imported recording. Notes choose by pitch, Bank, or Variant tiles. Recording gain above applies to the whole bank.",
    );
    for (index, variant) in bank.variants.iter().enumerate() {
        parent
            .spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                padding: UiRect::all(px(5)),
                border: UiRect::all(px(1)),
                ..default()
            })
            .with_children(|card| {
                let name = sample
                    .available_members
                    .iter()
                    .find(|choice| choice.id == variant.sample)
                    .map(|choice| choice.label.clone())
                    .unwrap_or_else(|| format!("Missing sample {}", variant.sample.0));
                label(
                    card,
                    &format!(
                        "Variant {index} · {name}{}",
                        if index == 0 { " · lead" } else { "" }
                    ),
                );
                card.spawn(row()).with_children(|row| {
                    if index > 1 {
                        let mut next = bank.clone();
                        next.variants.swap(index, index - 1);
                        button(row, "↑", sample.sample, Some(next));
                    }
                    if index > 0 && index + 1 < bank.variants.len() {
                        let mut next = bank.clone();
                        next.variants.swap(index, index + 1);
                        button(row, "↓", sample.sample, Some(next));
                    }
                    if index > 0 {
                        let mut next = bank.clone();
                        next.variants.remove(index);
                        button(row, "Remove from bank", sample.sample, Some(next));
                    }
                });
                field(card, sample.sample, bank, index, Field::Bank);
                card.spawn(row()).with_children(|row| {
                    let mut zone = bank.clone();
                    zone.variants[index].pitch_zone = if variant.pitch_zone.is_some() {
                        None
                    } else {
                        Some(SamplePitchZone {
                            low: 0.0,
                            high: 127.0,
                        })
                    };
                    button(
                        row,
                        if variant.pitch_zone.is_some() {
                            "Pitch range: on"
                        } else {
                            "Pitch range: off"
                        },
                        sample.sample,
                        Some(zone),
                    );
                    let mut choke = bank.clone();
                    choke.variants[index].hat_choke = !variant.hat_choke;
                    button(
                        row,
                        if variant.hat_choke {
                            "Hat choke: on"
                        } else {
                            "Hat choke: off"
                        },
                        sample.sample,
                        Some(choke),
                    );
                    let mut limit = bank.clone();
                    limit.variants[index].playback_limit_ms = if variant.playback_limit_ms.is_some()
                    {
                        None
                    } else {
                        Some(1000)
                    };
                    button(
                        row,
                        if variant.playback_limit_ms.is_some() {
                            "Limit: on"
                        } else {
                            "Limit: off"
                        },
                        sample.sample,
                        Some(limit),
                    );
                });
                if variant.pitch_zone.is_some() {
                    field(card, sample.sample, bank, index, Field::Low);
                    field(card, sample.sample, bank, index, Field::High);
                }
                if variant.playback_limit_ms.is_some() {
                    field(card, sample.sample, bank, index, Field::Limit);
                }
            });
    }
    label(parent, "Add an imported recording");
    for choice in sample.available_members.iter().filter(|choice| {
        choice.available
            && !bank
                .variants
                .iter()
                .any(|variant| variant.sample == choice.id)
    }) {
        let mut next = bank.clone();
        next.variants.push(SampleVariant::new(choice.id));
        button(
            parent,
            format!("+ {}", choice.label),
            sample.sample,
            Some(next),
        );
    }
    button(
        parent,
        "Dissolve bank · keep recordings",
        sample.sample,
        None,
    );
    label(
        parent,
        "Click fields to type · Enter saves · Esc cancels. A blank Bank field has no label. Hat choke cuts off other hat-group voices.",
    );
}

#[derive(Clone, Copy)]
enum Field {
    Bank,
    Low,
    High,
    Limit,
}
impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::Bank => "Bank",
            Self::Low => "Lowest MIDI pitch",
            Self::High => "Highest MIDI pitch",
            Self::Limit => "Limit (ms)",
        }
    }
    fn value(self, variant: &SampleVariant) -> String {
        match self {
            Self::Bank => variant.bank.clone().unwrap_or_default(),
            Self::Low => variant
                .pitch_zone
                .as_ref()
                .map(|zone| zone.low.to_string())
                .unwrap_or_default(),
            Self::High => variant
                .pitch_zone
                .as_ref()
                .map(|zone| zone.high.to_string())
                .unwrap_or_default(),
            Self::Limit => variant
                .playback_limit_ms
                .map(|value| value.to_string())
                .unwrap_or_default(),
        }
    }
    fn apply(self, variant: &mut SampleVariant, text: &str) -> Result<(), &'static str> {
        let text = text.trim();
        match self {
            Self::Bank => {
                if text.len() > 64 || text.chars().any(char::is_control) {
                    return Err("Use a bank label of at most 64 characters");
                }
                variant.bank = (!text.is_empty()).then(|| text.to_owned());
            }
            Self::Low | Self::High => {
                let pitch = text
                    .parse::<f64>()
                    .map_err(|_| "Enter a MIDI pitch from 0 to 127")?;
                if !pitch.is_finite() || !(0.0..=127.0).contains(&pitch) {
                    return Err("Enter a MIDI pitch from 0 to 127");
                }
                let zone = variant
                    .pitch_zone
                    .as_mut()
                    .ok_or("Enable the pitch range first")?;
                if matches!(self, Self::Low) {
                    if pitch > zone.high {
                        return Err("Lowest pitch must not exceed highest pitch");
                    }
                    zone.low = pitch;
                } else {
                    if pitch < zone.low {
                        return Err("Highest pitch must not be below lowest pitch");
                    }
                    zone.high = pitch;
                }
            }
            Self::Limit => {
                let millis = text
                    .parse::<u32>()
                    .map_err(|_| "Enter a positive whole number of milliseconds")?;
                if millis == 0 {
                    return Err("Enter a positive whole number of milliseconds");
                }
                variant.playback_limit_ms = Some(millis);
            }
        }
        Ok(())
    }
}

#[derive(Component)]
struct BankInput {
    sample: SampleId,
    definition: SampleBankDefinition,
    index: usize,
    field: Field,
    original: String,
    draft: String,
    selected: bool,
}

fn field(
    parent: &mut ChildSpawnerCommands<'_>,
    sample: SampleId,
    definition: &SampleBankDefinition,
    index: usize,
    field: Field,
) {
    let theme = MusaicUiTheme::default_dark();
    let original = field.value(&definition.variants[index]);
    parent
        .spawn((
            Node {
                min_height: px(28),
                width: percent(100),
                border: UiRect::all(px(1)),
                padding: UiRect::all(px(5)),
                ..default()
            },
            TabIndex(0),
            Interaction::default(),
            Text::new(format!("{}: {original}", field.name())),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(theme.chrome.text_main),
            ThemeFontColor(TEXT_MAIN),
            ThemedText,
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.button_border),
            BankInput {
                sample,
                definition: definition.clone(),
                index,
                field,
                draft: original.clone(),
                original,
                selected: true,
            },
        ))
        .observe(focus_field)
        .observe(type_field);
}

fn focus_field(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut BankInput, &mut Text, &mut BorderColor)>,
) {
    let Ok((mut input, mut text, mut border)) = fields.get_mut(event.entity) else {
        return;
    };
    focus.set(event.entity);
    input.selected = true;
    text.0 = format!("{}: [{}]", input.field.name(), input.draft);
    *border = BorderColor::all(Color::srgb(0.16, 0.42, 0.31));
    event.propagate(false);
}

fn type_field(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut BankInput, &mut Text, &mut BorderColor)>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let Ok((mut input, mut text, mut border)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    let key = event.input.key_code;
    keyboard.clear_just_pressed(key);
    let control = [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]
    .into_iter()
    .any(|key| keyboard.pressed(key));
    match key {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            let mut next = input.definition.clone();
            match input
                .field
                .apply(&mut next.variants[input.index], &input.draft)
            {
                Ok(()) => {
                    bus.write(EditorCommandBus(EditorCommand::SetSampleBank {
                        sample: input.sample,
                        definition: Some(next),
                    }));
                    input.selected = false;
                    focus.clear();
                }
                Err(message) => {
                    text.0 = format!("{}: {} · {message}", input.field.name(), input.draft);
                    *border = BorderColor::all(Color::srgb(0.7, 0.2, 0.18));
                    event.propagate(false);
                    return;
                }
            }
        }
        KeyCode::Escape => {
            input.draft = input.original.clone();
            input.selected = false;
            focus.clear();
        }
        KeyCode::KeyA if control => input.selected = true,
        KeyCode::Backspace | KeyCode::Delete => {
            if input.selected {
                input.draft.clear();
            } else {
                input.draft.pop();
            }
            input.selected = false;
        }
        _ if !control => {
            if let Some(value) =
                event
                    .input
                    .text
                    .as_deref()
                    .or_else(|| match &event.input.logical_key {
                        Key::Character(value) => Some(value.as_str()),
                        _ => None,
                    })
            {
                if input.selected {
                    input.draft.clear();
                    input.selected = false;
                }
                for character in value.chars().filter(|character| !character.is_control()) {
                    if input.draft.len() + character.len_utf8() <= 64 {
                        input.draft.push(character);
                    }
                }
            }
        }
        _ => {}
    }
    text.0 = format!(
        "{}: {}{}",
        input.field.name(),
        input.draft,
        if focus.0 == Some(event.focused_entity) {
            "│"
        } else {
            ""
        }
    );
    if focus.0 != Some(event.focused_entity) {
        *border = BorderColor::all(MusaicUiTheme::default_dark().chrome.button_border);
    }
    event.propagate(false);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variant_fields_preserve_other_settings_and_reject_invalid_ranges() {
        let mut variant = SampleVariant::new(SampleId(3));
        variant.pitch_zone = Some(SamplePitchZone {
            low: 0.0,
            high: 127.0,
        });
        variant.hat_choke = true;
        Field::Bank.apply(&mut variant, " soft piano ").unwrap();
        Field::Low.apply(&mut variant, "59.5").unwrap();
        Field::High.apply(&mut variant, "72.25").unwrap();
        Field::Limit.apply(&mut variant, "120").unwrap();
        assert_eq!(variant.bank.as_deref(), Some("soft piano"));
        assert!(variant.hat_choke);
        assert_eq!(variant.playback_limit_ms, Some(120));
        for (field, value) in [
            (Field::Low, "80"),
            (Field::High, "20"),
            (Field::Low, "NaN"),
            (Field::High, "inf"),
            (Field::Limit, "0"),
            (Field::Limit, "1.5"),
        ] {
            let before = variant.clone();
            assert!(field.apply(&mut variant, value).is_err());
            assert_eq!(variant, before);
        }
        Field::Bank.apply(&mut variant, "").unwrap();
        assert_eq!(variant.bank, None);
    }
}
