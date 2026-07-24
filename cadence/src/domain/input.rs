//! Normalized live note and control input values.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::control::{ControlKey, ControlMap, ControlValue, SignedUnitValue, UnitValue};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Stable identifier for an input source such as a device or port.
pub struct InputSourceId(String);

impl InputSourceId {
    /// Creates an input-source identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for InputSourceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for InputSourceId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// MIDI-style channel number in the range `0..16`.
pub struct InputChannel(u8);

impl InputChannel {
    /// Creates an input channel.
    ///
    /// Returns `None` when `value >= 16`.
    #[must_use]
    pub fn new(value: u8) -> Option<Self> {
        (value < 16).then_some(Self(value))
    }

    /// Returns the raw channel number.
    #[must_use]
    pub fn value(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// MIDI note number in the range `0..=127`.
pub struct NoteNumber(u8);

impl NoteNumber {
    /// Creates a note number.
    ///
    /// Returns `None` when `value > 127`.
    #[must_use]
    pub fn new(value: u8) -> Option<Self> {
        (value <= 127).then_some(Self(value))
    }

    /// Returns the raw note number.
    #[must_use]
    pub fn value(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// MIDI velocity in the range `0..=127`.
pub struct Velocity(u8);

impl Velocity {
    /// Creates a velocity value.
    ///
    /// Returns `None` when `value > 127`.
    #[must_use]
    pub fn new(value: u8) -> Option<Self> {
        (value <= 127).then_some(Self(value))
    }

    /// Returns the raw velocity value.
    #[must_use]
    pub fn value(self) -> u8 {
        self.0
    }

    /// Converts velocity into a unit-range scalar.
    #[must_use]
    pub fn as_unit(self) -> UnitValue {
        UnitValue::new(self.0 as f64 / 127.0).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Canonical live input event used by the runtime.
pub enum InputEvent {
    /// Note-on or note-off input.
    Note(NoteInput),
    /// Control-lane input.
    Control(ControlInput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Note-specific live input.
pub enum NoteInput {
    /// Starts a note with velocity.
    On {
        /// Note number being pressed.
        note: NoteNumber,
        /// Velocity of the key press.
        velocity: Velocity,
    },
    /// Releases a note.
    Off {
        /// Note number being released.
        note: NoteNumber,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Normalized control input after validation.
pub struct ControlInput {
    /// Control lane being changed.
    pub key: ControlKey,
    /// New value for the control lane.
    pub value: ControlValue,
    /// Source that produced the event.
    pub source: InputSourceId,
    /// Optional channel associated with the event.
    pub channel: Option<InputChannel>,
}

impl ControlInput {
    /// Creates a validated control input event.
    ///
    /// # Panics
    ///
    /// Panics if `value` is not valid for `key`.
    #[must_use]
    pub fn new(
        key: ControlKey,
        value: ControlValue,
        source: impl Into<InputSourceId>,
        channel: Option<InputChannel>,
    ) -> Self {
        value
            .validate_for(&key)
            .expect("control input value must match the control key");
        Self {
            key,
            value,
            source: source.into(),
            channel,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Raw MIDI-style message accepted by the normalizer.
pub enum MidiMessage {
    /// Note-on message.
    NoteOn {
        /// Message channel.
        channel: InputChannel,
        /// Note number.
        note: NoteNumber,
        /// Note velocity.
        velocity: Velocity,
    },
    /// Note-off message.
    NoteOff {
        /// Message channel.
        channel: InputChannel,
        /// Note number.
        note: NoteNumber,
    },
    /// Control-change message.
    ControlChange {
        /// Message channel.
        channel: InputChannel,
        /// Controller number.
        controller: u8,
        /// Controller value.
        value: u8,
    },
    /// Pitch-bend message.
    PitchBend {
        /// Message channel.
        channel: InputChannel,
        /// Signed pitch-bend amount in standard MIDI range.
        value: i16,
    },
    /// Channel-pressure / aftertouch message.
    ChannelPressure {
        /// Message channel.
        channel: InputChannel,
        /// Pressure value.
        value: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// How a raw MIDI value should be interpreted.
pub enum MidiValueKind {
    /// Convert to `false/true` using the threshold.
    BoolThreshold(u8),
    /// Convert to a `UnitValue`.
    Unipolar,
    /// Convert to a `SignedUnitValue`.
    Bipolar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// How a MIDI message should be mapped into the runtime.
pub enum MidiBinding {
    /// Drop the message.
    Ignore,
    /// Treat it as note input.
    Note,
    /// Translate it into a control event.
    Control {
        /// Target control lane.
        key: ControlKey,
        /// Conversion mode for the raw value.
        kind: MidiValueKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Mapping from MIDI controllers and gestures to normalized input events.
pub struct MidiBindings {
    cc_bindings: BTreeMap<u8, MidiBinding>,
    pitch_bend: MidiBinding,
    channel_pressure: MidiBinding,
}

impl MidiBindings {
    /// Returns a practical default mapping for a common keyboard controller.
    #[must_use]
    pub fn common_keyboard() -> Self {
        let mut bindings = Self {
            cc_bindings: BTreeMap::new(),
            pitch_bend: MidiBinding::Control {
                key: ControlKey::PitchBend,
                kind: MidiValueKind::Bipolar,
            },
            channel_pressure: MidiBinding::Control {
                key: ControlKey::Expression,
                kind: MidiValueKind::Unipolar,
            },
        };

        bindings = bindings.with_cc_binding(
            1,
            MidiBinding::Control {
                key: ControlKey::ModWheel,
                kind: MidiValueKind::Unipolar,
            },
        );
        bindings = bindings.with_cc_binding(
            7,
            MidiBinding::Control {
                key: ControlKey::Expression,
                kind: MidiValueKind::Unipolar,
            },
        );
        bindings.with_cc_binding(
            64,
            MidiBinding::Control {
                key: ControlKey::SustainPedal,
                kind: MidiValueKind::BoolThreshold(64),
            },
        )
    }

    /// Adds or replaces one CC binding.
    #[must_use]
    pub fn with_cc_binding(mut self, controller: u8, binding: MidiBinding) -> Self {
        self.cc_bindings.insert(controller, binding);
        self
    }

    /// Replaces the pitch-bend binding.
    #[must_use]
    pub fn with_pitch_bend_binding(mut self, binding: MidiBinding) -> Self {
        self.pitch_bend = binding;
        self
    }

    /// Replaces the channel-pressure binding.
    #[must_use]
    pub fn with_channel_pressure_binding(mut self, binding: MidiBinding) -> Self {
        self.channel_pressure = binding;
        self
    }

    /// Translates one raw MIDI message into a normalized input event.
    ///
    /// Returns `None` when no binding applies.
    #[must_use]
    pub fn translate(
        &self,
        source: impl Into<InputSourceId>,
        message: MidiMessage,
    ) -> Option<InputEvent> {
        let source = source.into();

        match message {
            MidiMessage::NoteOn { note, velocity, .. } if velocity.value() == 0 => {
                Some(InputEvent::Note(NoteInput::Off { note }))
            }
            MidiMessage::NoteOn { note, velocity, .. } => {
                Some(InputEvent::Note(NoteInput::On { note, velocity }))
            }
            MidiMessage::NoteOff { note, .. } => Some(InputEvent::Note(NoteInput::Off { note })),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => {
                let binding = self.cc_bindings.get(&controller)?;
                translate_binding(binding, source, Some(channel), value)
            }
            MidiMessage::PitchBend { channel, value } => {
                translate_pitch_bend(&self.pitch_bend, source, channel, value)
            }
            MidiMessage::ChannelPressure { channel, value } => {
                translate_binding(&self.channel_pressure, source, Some(channel), value)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Sample trigger to emit for a note, plus per-note control overrides.
pub struct SampleNoteBinding {
    sample: String,
    controls: ControlMap,
}

impl SampleNoteBinding {
    /// Creates a binding that triggers the named sample.
    #[must_use]
    pub fn new(sample: impl Into<String>) -> Self {
        Self {
            sample: sample.into(),
            controls: ControlMap::new(),
        }
    }

    /// Adds a control override to the binding.
    ///
    /// # Panics
    ///
    /// Panics if `value` is not valid for `key`.
    #[must_use]
    pub fn with_control(mut self, key: ControlKey, value: ControlValue) -> Self {
        value
            .validate_for(&key)
            .expect("sample note binding control must match the control key");
        self.controls.insert(key, value);
        self
    }

    /// Returns the sample name.
    #[must_use]
    pub fn sample(&self) -> &str {
        &self.sample
    }

    /// Returns the per-note control overrides.
    #[must_use]
    pub fn controls(&self) -> &ControlMap {
        &self.controls
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Rule for resolving a note number to a sample binding.
pub enum SampleNoteRule {
    /// Match one exact note.
    Exact {
        /// Note that must match exactly.
        note: NoteNumber,
        /// Binding to use.
        binding: SampleNoteBinding,
    },
    /// Match an inclusive note range.
    Range {
        /// Inclusive start note.
        start: NoteNumber,
        /// Inclusive end note.
        end: NoteNumber,
        /// Binding to use.
        binding: SampleNoteBinding,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
/// Ordered set of sample-note resolution rules.
pub struct SampleNoteMap {
    rules: Vec<SampleNoteRule>,
}

impl SampleNoteMap {
    /// Returns an empty note map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an exact-match rule.
    #[must_use]
    pub fn exact(mut self, note: NoteNumber, binding: SampleNoteBinding) -> Self {
        self.rules.push(SampleNoteRule::Exact { note, binding });
        self
    }

    /// Adds an inclusive range rule.
    ///
    /// # Panics
    ///
    /// Panics if `start > end`.
    #[must_use]
    pub fn range(mut self, start: NoteNumber, end: NoteNumber, binding: SampleNoteBinding) -> Self {
        assert!(
            start.value() <= end.value(),
            "note range start must be <= end"
        );
        self.rules.push(SampleNoteRule::Range {
            start,
            end,
            binding,
        });
        self
    }

    /// Resolves the first rule that matches `note`.
    #[must_use]
    pub fn resolve(&self, note: NoteNumber) -> Option<&SampleNoteBinding> {
        self.rules.iter().find_map(|rule| match rule {
            SampleNoteRule::Exact {
                note: exact,
                binding,
            } if *exact == note => Some(binding),
            SampleNoteRule::Range {
                start,
                end,
                binding,
            } if (start.value()..=end.value()).contains(&note.value()) => Some(binding),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
/// Set of notes that are currently held or sustained.
pub struct HeldNotes(BTreeSet<NoteNumber>);

impl HeldNotes {
    /// Marks a note as active.
    pub fn note_on(&mut self, note: NoteNumber) {
        self.0.insert(note);
    }

    /// Marks a note as inactive.
    pub fn note_off(&mut self, note: NoteNumber) {
        self.0.remove(&note);
    }

    /// Returns `true` when the note is currently active.
    #[must_use]
    pub fn contains(&self, note: NoteNumber) -> bool {
        self.0.contains(&note)
    }

    /// Iterates over active notes.
    pub fn iter(&self) -> impl Iterator<Item = NoteNumber> + '_ {
        self.0.iter().copied()
    }
}

fn translate_binding(
    binding: &MidiBinding,
    source: InputSourceId,
    channel: Option<InputChannel>,
    value: u8,
) -> Option<InputEvent> {
    let MidiBinding::Control { key, kind } = binding else {
        return None;
    };

    let control_value = match kind {
        MidiValueKind::BoolThreshold(threshold) => ControlValue::Bool(value >= *threshold),
        MidiValueKind::Unipolar => {
            ControlValue::Unipolar(UnitValue::new(value as f64 / 127.0).unwrap())
        }
        MidiValueKind::Bipolar => {
            ControlValue::Bipolar(SignedUnitValue::new((value as f64 / 127.0) * 2.0 - 1.0).unwrap())
        }
    };

    Some(InputEvent::Control(ControlInput::new(
        key.clone(),
        control_value,
        source,
        channel,
    )))
}

fn translate_pitch_bend(
    binding: &MidiBinding,
    source: InputSourceId,
    channel: InputChannel,
    value: i16,
) -> Option<InputEvent> {
    let MidiBinding::Control { key, kind } = binding else {
        return None;
    };

    let normalized = (value as f64 / 8192.0).clamp(-1.0, 1.0);
    let control_value = match kind {
        MidiValueKind::Bipolar => ControlValue::Bipolar(SignedUnitValue::new(normalized).unwrap()),
        MidiValueKind::Unipolar => {
            ControlValue::Unipolar(UnitValue::new((normalized + 1.0) / 2.0).unwrap())
        }
        MidiValueKind::BoolThreshold(threshold) => {
            ControlValue::Bool((((normalized + 1.0) / 2.0) * 127.0).round() as u8 >= *threshold)
        }
    };

    Some(InputEvent::Control(ControlInput::new(
        key.clone(),
        control_value,
        source,
        Some(channel),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_bindings_translate_common_keyboard_messages() {
        let bindings = MidiBindings::common_keyboard();
        let note = NoteNumber::new(60).unwrap();
        let velocity = Velocity::new(100).unwrap();
        let channel = InputChannel::new(0).unwrap();

        assert_eq!(
            bindings.translate(
                "keyboard",
                MidiMessage::NoteOn {
                    channel,
                    note,
                    velocity,
                }
            ),
            Some(InputEvent::Note(NoteInput::On { note, velocity }))
        );
        assert!(matches!(
            bindings.translate(
                "keyboard",
                MidiMessage::ControlChange {
                    channel,
                    controller: 7,
                    value: 127,
                }
            ),
            Some(InputEvent::Control(ControlInput {
                key: ControlKey::Expression,
                ..
            }))
        ));
    }

    #[test]
    fn sample_note_map_resolves_exact_and_range_rules() {
        let c3 = NoteNumber::new(47).unwrap();
        let c4 = NoteNumber::new(60).unwrap();
        let map = SampleNoteMap::new()
            .exact(c4, SampleNoteBinding::new("snare"))
            .range(
                NoteNumber::new(36).unwrap(),
                NoteNumber::new(47).unwrap(),
                SampleNoteBinding::new("kick"),
            );

        assert_eq!(map.resolve(c4).unwrap().sample(), "snare");
        assert_eq!(map.resolve(c3).unwrap().sample(), "kick");
    }

    #[test]
    fn held_notes_track_note_lifecycle() {
        let note = NoteNumber::new(64).unwrap();
        let mut held = HeldNotes::default();

        held.note_on(note);
        assert!(held.contains(note));
        held.note_off(note);
        assert!(!held.contains(note));
    }

    #[test]
    fn custom_cc_binding_can_target_custom_keys() {
        let bindings = MidiBindings::common_keyboard().with_cc_binding(
            74,
            MidiBinding::Control {
                key: ControlKey::Custom(crate::domain::control::Symbol::from("brightness")),
                kind: MidiValueKind::Unipolar,
            },
        );

        assert!(matches!(
            bindings.translate(
                "keyboard",
                MidiMessage::ControlChange {
                    channel: InputChannel::new(0).unwrap(),
                    controller: 74,
                    value: 64,
                }
            ),
            Some(InputEvent::Control(ControlInput {
                key: ControlKey::Custom(_),
                ..
            }))
        ));
    }
}
