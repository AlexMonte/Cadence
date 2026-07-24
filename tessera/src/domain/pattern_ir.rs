use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Sub};

use serde::{Deserialize, Serialize};

use super::container::ContainerId;
use super::program::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct Rational {
    pub numerator: i64,
    pub denominator: i64,
}

impl Rational {
    pub fn new(numerator: i64, denominator: i64) -> Self {
        assert!(denominator != 0, "denominator cannot be zero");
        let sign = if denominator < 0 { -1 } else { 1 };
        let mut numerator = numerator * sign;
        let mut denominator = denominator.abs();
        let gcd = gcd_i64(numerator.abs(), denominator);
        numerator /= gcd;
        denominator /= gcd;
        Self {
            numerator,
            denominator,
        }
    }

    pub fn from_integer(value: i64) -> Self {
        Self::new(value, 1)
    }

    pub fn zero() -> Self {
        Self::from_integer(0)
    }

    pub fn one() -> Self {
        Self::from_integer(1)
    }

    pub fn is_zero(self) -> bool {
        self.numerator == 0
    }

    pub fn is_positive(self) -> bool {
        self > Self::zero()
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.numerator as i128 * other.denominator as i128)
            .cmp(&(other.numerator as i128 * self.denominator as i128))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for Rational {
    type Output = Rational;
    fn add(self, rhs: Self) -> Self::Output {
        Rational::new(
            self.numerator * rhs.denominator + rhs.numerator * self.denominator,
            self.denominator * rhs.denominator,
        )
    }
}

impl Sub for Rational {
    type Output = Rational;
    fn sub(self, rhs: Self) -> Self::Output {
        Rational::new(
            self.numerator * rhs.denominator - rhs.numerator * self.denominator,
            self.denominator * rhs.denominator,
        )
    }
}

impl Mul for Rational {
    type Output = Rational;
    fn mul(self, rhs: Self) -> Self::Output {
        Rational::new(
            self.numerator * rhs.numerator,
            self.denominator * rhs.denominator,
        )
    }
}

impl Div for Rational {
    type Output = Rational;
    fn div(self, rhs: Self) -> Self::Output {
        assert!(!rhs.is_zero(), "cannot divide by zero rational");
        Rational::new(
            self.numerator * rhs.denominator,
            self.denominator * rhs.numerator,
        )
    }
}

fn gcd_i64(a: i64, b: i64) -> i64 {
    if b == 0 { a.max(1) } else { gcd_i64(b, a % b) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct CycleTime(pub Rational);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct CycleDuration(pub Rational);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct CycleSpan {
    pub start: CycleTime,
    pub duration: CycleDuration,
}

impl CycleSpan {
    pub fn new(start: CycleTime, duration: CycleDuration) -> Self {
        Self { start, duration }
    }

    pub fn end(self) -> CycleTime {
        CycleTime(self.start.0 + self.duration.0)
    }

    pub fn shift_by(self, offset: CycleDuration) -> Self {
        Self {
            start: CycleTime(self.start.0 + offset.0),
            duration: self.duration,
        }
    }

    pub fn scale_relative_to(self, origin: CycleTime, factor: Rational) -> Self {
        let relative_start = self.start.0 - origin.0;
        Self {
            start: CycleTime(origin.0 + relative_start * factor),
            duration: CycleDuration(self.duration.0 * factor),
        }
    }

    pub fn reverse_within(self, origin: CycleTime, end: CycleTime) -> Self {
        let event_end = self.end();
        Self {
            start: CycleTime(end.0 - (event_end.0 - origin.0)),
            duration: self.duration,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum EventValue {
    Note {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        octave: Option<i64>,
    },
    Rest,
    Scalar {
        value: Rational,
    },
}

impl EventValue {
    pub fn is_rest(&self) -> bool {
        matches!(self, Self::Rest)
    }

    pub fn is_scalar(&self) -> bool {
        matches!(self, Self::Scalar { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum FieldValue {
    Rational { value: Rational },
    Bool { value: bool },
    Symbol { value: String },
}

impl FieldValue {
    pub fn rational(value: Rational) -> Self {
        Self::Rational { value }
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool { value }
    }

    pub fn symbol(value: impl Into<String>) -> Self {
        Self::Symbol {
            value: value.into(),
        }
    }
}

/// One axis of the 3D audio lattice.
///
/// This is the spatial-audio lattice (where a sound sits in 3D space), distinct
/// from the canvas-layout `SpatialSide` used to lay out nodes on the authoring
/// surface. `x` is lateral, `y` vertical, `z` depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum AxisIr {
    X,
    Y,
    Z,
}

/// An exact lattice position authored in Tessera, lowered to a Cadence `Point3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct Point3Ir {
    pub x: Rational,
    pub y: Rational,
    pub z: Rational,
}

impl Point3Ir {
    pub fn new(x: Rational, y: Rational, z: Rational) -> Self {
        Self { x, y, z }
    }

    pub fn origin() -> Self {
        Self {
            x: Rational::zero(),
            y: Rational::zero(),
            z: Rational::zero(),
        }
    }

    pub fn scaled(self, factor: Point3Ir) -> Self {
        Self {
            x: self.x * factor.x,
            y: self.y * factor.y,
            z: self.z * factor.z,
        }
    }

    pub fn reflected(self, axis: AxisIr) -> Self {
        let neg = |value: Rational| Rational::zero() - value;
        match axis {
            AxisIr::X => Self {
                x: neg(self.x),
                ..self
            },
            AxisIr::Y => Self {
                y: neg(self.y),
                ..self
            },
            AxisIr::Z => Self {
                z: neg(self.z),
                ..self
            },
        }
    }
}

impl Add for Point3Ir {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

/// A spatial trajectory authored in Tessera, lowered to a Cadence
/// `SpatialMotion`. Exact and cycle-relative, mirroring the runtime kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum SpatialMotionIr {
    Static {
        point: Point3Ir,
    },
    Linear {
        start: Point3Ir,
        end: Point3Ir,
    },
    Orbit {
        center: Point3Ir,
        radius: Rational,
        rate: Rational,
        phase: Rational,
    },
}

impl SpatialMotionIr {
    pub fn origin() -> Self {
        Self::Static {
            point: Point3Ir::origin(),
        }
    }

    pub fn is_origin(&self) -> bool {
        matches!(self, Self::Static { point } if *point == Point3Ir::origin())
    }

    pub fn translated(self, offset: Point3Ir) -> Self {
        match self {
            Self::Static { point } => Self::Static {
                point: point + offset,
            },
            Self::Linear { start, end } => Self::Linear {
                start: start + offset,
                end: end + offset,
            },
            Self::Orbit {
                center,
                radius,
                rate,
                phase,
            } => Self::Orbit {
                center: center + offset,
                radius,
                rate,
                phase,
            },
        }
    }

    pub fn scaled(self, factor: Point3Ir) -> Self {
        match self {
            Self::Static { point } => Self::Static {
                point: point.scaled(factor),
            },
            Self::Linear { start, end } => Self::Linear {
                start: start.scaled(factor),
                end: end.scaled(factor),
            },
            Self::Orbit {
                center,
                radius,
                rate,
                phase,
            } => Self::Orbit {
                center: center.scaled(factor),
                radius: radius * factor.x,
                rate,
                phase,
            },
        }
    }

    pub fn reflected(self, axis: AxisIr) -> Self {
        match self {
            Self::Static { point } => Self::Static {
                point: point.reflected(axis),
            },
            Self::Linear { start, end } => Self::Linear {
                start: start.reflected(axis),
                end: end.reflected(axis),
            },
            Self::Orbit {
                center,
                radius,
                rate,
                phase,
            } => Self::Orbit {
                center: center.reflected(axis),
                radius,
                rate: Rational::zero() - rate,
                phase,
            },
        }
    }
}

impl Default for SpatialMotionIr {
    fn default() -> Self {
        Self::origin()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum EventField {
    Gain(FieldValue),
    PostGain(FieldValue),
    Pitch(FieldValue),
    PitchBend(FieldValue),
    PlaybackRate(FieldValue),
    PlaybackStart(FieldValue),
    PlaybackEnd(FieldValue),
    Reverse(FieldValue),
    Attack(FieldValue),
    Decay(FieldValue),
    Sustain(FieldValue),
    Release(FieldValue),
    LowPassCutoff(FieldValue),
    LowPassResonance(FieldValue),
    HighPassCutoff(FieldValue),
    HighPassResonance(FieldValue),
    ReverbSend(FieldValue),
    DelaySend(FieldValue),
    Select(FieldValue),
    Custom { key: String, value: FieldValue },
    Elongate(FieldValue),
    Replicate(FieldValue),
    Degrade(FieldValue),
    RandomChoice,
    Transpose(FieldValue),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct PatternProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct PatternEvent {
    pub span: CycleSpan,
    pub value: EventValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EventField>,
    /// First-class lattice position of this event (the spatial twin of `span`).
    /// Defaults to the lattice origin and lowers to Cadence's `Moment.position`.
    #[serde(default, skip_serializing_if = "SpatialMotionIr::is_origin")]
    pub position: SpatialMotionIr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PatternProvenance>,
}

impl PatternEvent {
    pub fn new(span: CycleSpan, value: EventValue) -> Self {
        Self {
            span,
            value,
            fields: Vec::new(),
            position: SpatialMotionIr::origin(),
            source: None,
        }
    }

    pub fn with_source(mut self, source: Option<PatternProvenance>) -> Self {
        self.source = source;
        self
    }

    pub fn with_fields(mut self, fields: Vec<EventField>) -> Self {
        self.fields = fields;
        self
    }

    pub fn with_position(mut self, position: SpatialMotionIr) -> Self {
        self.position = position;
        self
    }

    pub fn translate_position(mut self, offset: Point3Ir) -> Self {
        self.position = self.position.translated(offset);
        self
    }

    pub fn scale_position(mut self, factor: Point3Ir) -> Self {
        self.position = self.position.scaled(factor);
        self
    }

    pub fn reflect_position(mut self, axis: AxisIr) -> Self {
        self.position = self.position.reflected(axis);
        self
    }

    pub fn shift_by(mut self, offset: CycleDuration) -> Self {
        self.span = self.span.shift_by(offset);
        self
    }

    pub fn scale_relative_to(mut self, origin: CycleTime, factor: Rational) -> Self {
        self.span = self.span.scale_relative_to(origin, factor);
        self
    }

    pub fn reverse_within(mut self, origin: CycleTime, end: CycleTime) -> Self {
        self.span = self.span.reverse_within(origin, end);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum PatternStreamShape {
    Event,
    Control,
    Scalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct PatternStream {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<PatternEvent>,
}

impl PatternStream {
    pub fn new(events: Vec<PatternEvent>) -> Self {
        Self { events }
    }

    pub fn bounds(&self) -> Option<(CycleTime, CycleTime)> {
        let first = self.events.first()?;
        let mut min_start = first.span.start.0;
        let mut max_end = first.span.end().0;
        for event in &self.events {
            if event.span.start.0 < min_start {
                min_start = event.span.start.0;
            }
            let end = event.span.end().0;
            if end > max_end {
                max_end = end;
            }
        }
        Some((CycleTime(min_start), CycleTime(max_end)))
    }

    pub fn duration(&self) -> CycleDuration {
        self.bounds()
            .map(|(start, end)| CycleDuration(end.0 - start.0))
            .unwrap_or(CycleDuration(Rational::zero()))
    }

    pub fn shift_by(&self, offset: CycleDuration) -> Self {
        Self {
            events: self
                .events
                .iter()
                .cloned()
                .map(|event| event.shift_by(offset))
                .collect(),
        }
    }

    pub fn layer(streams: Vec<PatternStream>) -> Self {
        let mut events = Vec::new();
        for stream in streams {
            events.extend(stream.events);
        }
        Self { events }
    }

    pub fn chain(left: PatternStream, right: PatternStream) -> Self {
        let offset = left.duration();
        let mut events = left.normalize_to_origin().events;
        events.extend(right.normalize_to_origin().shift_by(offset).events);
        Self { events }
    }

    pub fn reverse(self) -> Self {
        let Some((origin, end)) = self.bounds() else {
            return self;
        };
        Self {
            events: self
                .events
                .into_iter()
                .map(|event| event.reverse_within(origin, end))
                .collect(),
        }
    }

    pub fn space_shift(self, offset: Point3Ir) -> Self {
        Self {
            events: self
                .events
                .into_iter()
                .map(|event| event.translate_position(offset))
                .collect(),
        }
    }

    pub fn space_scale(self, factor: Point3Ir) -> Self {
        Self {
            events: self
                .events
                .into_iter()
                .map(|event| event.scale_position(factor))
                .collect(),
        }
    }

    pub fn space_reflect(self, axis: AxisIr) -> Self {
        Self {
            events: self
                .events
                .into_iter()
                .map(|event| event.reflect_position(axis))
                .collect(),
        }
    }

    pub fn slow(self, factor: Rational) -> Self {
        self.scale_relative(factor)
    }

    pub fn fast(self, factor: Rational) -> Self {
        if factor <= Rational::zero() {
            return self;
        }
        self.scale_relative(Rational::one() / factor)
    }

    fn scale_relative(self, factor: Rational) -> Self {
        let Some((origin, _)) = self.bounds() else {
            return self;
        };
        Self {
            events: self
                .events
                .into_iter()
                .map(|event| event.scale_relative_to(origin, factor))
                .collect(),
        }
    }

    pub fn normalize_to_origin(self) -> Self {
        let Some((origin, _)) = self.bounds() else {
            return self;
        };
        self.shift_by(CycleDuration(Rational::zero() - origin.0))
    }

    pub fn without_rests(self) -> Self {
        Self {
            events: self
                .events
                .into_iter()
                .filter(|event| !event.value.is_rest())
                .collect(),
        }
    }

    pub fn clip_by_mask(self, mask: &PatternStream) -> Self {
        let mut clipped = Vec::new();
        for event in self.events {
            for mask_event in &mask.events {
                if !mask_event_active(mask_event) {
                    continue;
                }
                let start = if event.span.start.0 > mask_event.span.start.0 {
                    event.span.start.0
                } else {
                    mask_event.span.start.0
                };
                let event_end = event.span.start.0 + event.span.duration.0;
                let mask_end = mask_event.span.start.0 + mask_event.span.duration.0;
                let end = if event_end < mask_end {
                    event_end
                } else {
                    mask_end
                };
                if end > start {
                    clipped.push(PatternEvent {
                        span: CycleSpan {
                            start: CycleTime(start),
                            duration: CycleDuration(end - start),
                        },
                        value: event.value.clone(),
                        fields: event.fields.clone(),
                        position: event.position,
                        source: event.source.clone(),
                    });
                }
            }
        }
        Self { events: clipped }
    }

    pub fn degrade_by_policy(self, keep_probability: Rational, seed: u64) -> Self {
        if keep_probability <= Rational::zero() {
            return Self::default();
        }
        if keep_probability >= Rational::one() {
            return self;
        }
        let threshold =
            ((keep_probability.numerator * 1024) / keep_probability.denominator).clamp(0, 1024);
        Self {
            events: self
                .events
                .into_iter()
                .filter(|event| {
                    let event_seed = flatten_event_seed(event) ^ seed;
                    (event_seed % 1024) >= threshold as u64
                })
                .collect(),
        }
    }
}

fn mask_event_active(event: &PatternEvent) -> bool {
    match event.value {
        EventValue::Scalar { value } => value > Rational::zero(),
        _ => true,
    }
}

fn flatten_event_seed(event: &PatternEvent) -> u64 {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct ControlStream {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<ControlEvent>,
}

impl ControlStream {
    pub fn new(controls: Vec<ControlEvent>) -> Self {
        Self { controls }
    }

    pub fn to_pattern_stream(&self) -> PatternStream {
        PatternStream {
            events: self
                .controls
                .iter()
                .cloned()
                .map(PatternEvent::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct ControlEvent {
    pub span: CycleSpan,
    pub key: ControlKeyIr,
    pub value: ControlValueIr,
}

impl ControlEvent {
    pub fn new(span: CycleSpan, key: ControlKeyIr, value: ControlValueIr) -> Self {
        Self { span, key, value }
    }
}

impl From<ControlEvent> for PatternEvent {
    fn from(control: ControlEvent) -> Self {
        PatternEvent {
            span: control.span,
            value: EventValue::Scalar {
                value: control.value.as_rational_fallback(),
            },
            fields: vec![EventField::Custom {
                key: control.key.as_str().to_string(),
                value: control.value.into_field_value(),
            }],
            position: SpatialMotionIr::origin(),
            source: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum ControlKeyIr {
    Gate,
    Gain,
    PostGain,
    Pitch,
    PitchBend,
    PlaybackRate,
    PlaybackStart,
    PlaybackEnd,
    Reverse,
    Attack,
    Decay,
    Sustain,
    Release,
    LowPassCutoff,
    LowPassResonance,
    HighPassCutoff,
    HighPassResonance,
    ReverbSend,
    DelaySend,
    Select(String),
    Custom(String),
}

impl ControlKeyIr {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Gate => "gate",
            Self::Gain => "gain",
            Self::PostGain => "post_gain",
            Self::Pitch => "pitch",
            Self::PitchBend => "pitch_bend",
            Self::PlaybackRate => "playback_rate",
            Self::PlaybackStart => "playback_start",
            Self::PlaybackEnd => "playback_end",
            Self::Reverse => "reverse",
            Self::Attack => "attack",
            Self::Decay => "decay",
            Self::Sustain => "sustain",
            Self::Release => "release",
            Self::LowPassCutoff => "low_pass_cutoff",
            Self::LowPassResonance => "low_pass_resonance",
            Self::HighPassCutoff => "high_pass_cutoff",
            Self::HighPassResonance => "high_pass_resonance",
            Self::ReverbSend => "reverb_send",
            Self::DelaySend => "delay_send",
            Self::Select(_) => "select",
            Self::Custom(_) => "custom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum ControlValueIr {
    Rational { value: Rational },
    Bool { value: bool },
    Symbol { value: String },
}

impl ControlValueIr {
    pub fn rational(value: Rational) -> Self {
        Self::Rational { value }
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool { value }
    }

    pub fn symbol(value: impl Into<String>) -> Self {
        Self::Symbol {
            value: value.into(),
        }
    }

    pub fn as_rational_fallback(&self) -> Rational {
        match self {
            Self::Rational { value } => *value,
            Self::Bool { value } => {
                if *value {
                    Rational::one()
                } else {
                    Rational::zero()
                }
            }
            Self::Symbol { .. } => Rational::zero(),
        }
    }

    pub fn into_field_value(self) -> FieldValue {
        match self {
            Self::Rational { value } => FieldValue::rational(value),
            Self::Bool { value } => FieldValue::bool(value),
            Self::Symbol { value } => FieldValue::symbol(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct ScalarStream {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<ScalarEvent>,
}

impl ScalarStream {
    pub fn new(values: Vec<ScalarEvent>) -> Self {
        Self { values }
    }

    pub fn to_pattern_stream(&self) -> PatternStream {
        PatternStream {
            events: self
                .values
                .iter()
                .cloned()
                .map(PatternEvent::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct ScalarEvent {
    pub span: CycleSpan,
    pub value: Rational,
}

impl ScalarEvent {
    pub fn new(span: CycleSpan, value: Rational) -> Self {
        Self { span, value }
    }
}

impl From<ScalarEvent> for PatternEvent {
    fn from(value: ScalarEvent) -> Self {
        PatternEvent {
            span: value.span,
            value: EventValue::Scalar { value: value.value },
            fields: Vec::new(),
            position: SpatialMotionIr::origin(),
            source: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum DeduplicateKeyIr {
    Lifecycle,
    WholeSpanAndValue,
    StartAndValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum DeduplicateWinnerIr {
    First,
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct DeduplicatePolicyIr {
    pub key: DeduplicateKeyIr,
    pub winner: DeduplicateWinnerIr,
}

impl DeduplicatePolicyIr {
    pub fn whole_span_and_value() -> Self {
        Self {
            key: DeduplicateKeyIr::WholeSpanAndValue,
            winner: DeduplicateWinnerIr::First,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum PriorityConflictIr {
    SameWholeStartAndValue,
    SameWholeSpanAndValue,
    WholeSpanOverlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct PriorityMergePolicyIr {
    pub conflict: PriorityConflictIr,
}

impl PriorityMergePolicyIr {
    pub fn whole_span_overlap() -> Self {
        Self {
            conflict: PriorityConflictIr::WholeSpanOverlap,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatternNodeIr {
    EventStream(EventStreamNodeIr),
    ControlStream(ControlStreamNodeIr),
    ScalarStream(ScalarStreamNodeIr),
    Merge {
        children: Vec<PatternNodeIr>,
    },
    CycleRoute {
        children: Vec<PatternNodeIr>,
    },
    CycleSlots {
        children: Vec<PatternNodeIr>,
    },
    TimeScale {
        inner: Box<PatternNodeIr>,
        factor: Rational,
    },
    Shift {
        inner: Box<PatternNodeIr>,
        offset: CycleDuration,
    },
    ReflectCycle {
        inner: Box<PatternNodeIr>,
    },
    SpaceShift {
        inner: Box<PatternNodeIr>,
        offset: Point3Ir,
    },
    SpaceScale {
        inner: Box<PatternNodeIr>,
        factor: Point3Ir,
    },
    SpaceReflect {
        inner: Box<PatternNodeIr>,
        axis: AxisIr,
    },
    Degrade {
        inner: Box<PatternNodeIr>,
        keep_probability: Rational,
        seed: u64,
    },
    Deduplicate {
        inner: Box<PatternNodeIr>,
        policy: DeduplicatePolicyIr,
    },
    PriorityMerge {
        children: Vec<PatternNodeIr>,
        policy: PriorityMergePolicyIr,
    },
    WeightedChoice {
        options: Vec<WeightedPatternIr>,
        seed: u64,
    },
    MaskClip {
        source: Box<PatternNodeIr>,
        mask: Box<PatternNodeIr>,
    },
    /// Sequential cycle chaining: each child owns one cycle, concatenated in order.
    Concat {
        children: Vec<PatternNodeIr>,
    },
}

impl PatternNodeIr {
    pub fn event_stream(stream: PatternStream) -> Self {
        Self::EventStream(EventStreamNodeIr { stream })
    }

    pub fn control_stream(stream: ControlStream) -> Self {
        Self::ControlStream(ControlStreamNodeIr { stream })
    }

    pub fn scalar_stream(stream: ScalarStream) -> Self {
        Self::ScalarStream(ScalarStreamNodeIr { stream })
    }

    pub fn merge(children: Vec<PatternNodeIr>) -> Self {
        Self::Merge { children }
    }

    pub fn cycle_route(children: Vec<PatternNodeIr>) -> Self {
        Self::CycleRoute { children }
    }

    pub fn cycle_slots(children: Vec<PatternNodeIr>) -> Self {
        Self::CycleSlots { children }
    }

    pub fn time_scale(inner: PatternNodeIr, factor: Rational) -> Self {
        Self::TimeScale {
            inner: Box::new(inner),
            factor,
        }
    }

    pub fn shift(inner: PatternNodeIr, offset: CycleDuration) -> Self {
        Self::Shift {
            inner: Box::new(inner),
            offset,
        }
    }

    pub fn reflect_cycle(inner: PatternNodeIr) -> Self {
        Self::ReflectCycle {
            inner: Box::new(inner),
        }
    }

    pub fn space_shift(inner: PatternNodeIr, offset: Point3Ir) -> Self {
        Self::SpaceShift {
            inner: Box::new(inner),
            offset,
        }
    }

    pub fn space_scale(inner: PatternNodeIr, factor: Point3Ir) -> Self {
        Self::SpaceScale {
            inner: Box::new(inner),
            factor,
        }
    }

    pub fn space_reflect(inner: PatternNodeIr, axis: AxisIr) -> Self {
        Self::SpaceReflect {
            inner: Box::new(inner),
            axis,
        }
    }

    /// Tidal-style `jux`: layer the base placed hard-left with `right` placed
    /// hard-right along the lateral (`x`) axis. `right` is the already
    /// transformed copy. Position is first-class, so this lowers to a spatial
    /// merge rather than a pan-control lane.
    pub fn jux(base: PatternNodeIr, right: PatternNodeIr) -> Self {
        let left_offset = Point3Ir::new(
            Rational::zero() - Rational::one(),
            Rational::zero(),
            Rational::zero(),
        );
        let right_offset = Point3Ir::new(Rational::one(), Rational::zero(), Rational::zero());
        Self::merge(vec![
            Self::space_shift(base, left_offset),
            Self::space_shift(right, right_offset),
        ])
    }

    pub fn degrade(inner: PatternNodeIr, keep_probability: Rational, seed: u64) -> Self {
        Self::Degrade {
            inner: Box::new(inner),
            keep_probability,
            seed,
        }
    }

    pub fn deduplicate(inner: PatternNodeIr, policy: DeduplicatePolicyIr) -> Self {
        Self::Deduplicate {
            inner: Box::new(inner),
            policy,
        }
    }

    pub fn priority_merge(children: Vec<PatternNodeIr>, policy: PriorityMergePolicyIr) -> Self {
        Self::PriorityMerge { children, policy }
    }

    pub fn weighted_choice(options: Vec<WeightedPatternIr>, seed: u64) -> Self {
        Self::WeightedChoice { options, seed }
    }

    pub fn mask_clip(source: PatternNodeIr, mask: PatternNodeIr) -> Self {
        Self::MaskClip {
            source: Box::new(source),
            mask: Box::new(mask),
        }
    }

    pub fn concat(children: Vec<PatternNodeIr>) -> Self {
        match children.len() {
            0 => Self::event_stream(PatternStream::default()),
            1 => children.into_iter().next().expect("checked length"),
            _ => Self::Concat { children },
        }
    }

    pub fn shape(&self) -> PatternStreamShape {
        match self {
            Self::EventStream(_) => PatternStreamShape::Event,
            Self::ControlStream(_) => PatternStreamShape::Control,
            Self::ScalarStream(_) => PatternStreamShape::Scalar,
            Self::Merge { .. }
            | Self::CycleRoute { .. }
            | Self::CycleSlots { .. }
            | Self::TimeScale { .. }
            | Self::Shift { .. }
            | Self::ReflectCycle { .. }
            | Self::SpaceShift { .. }
            | Self::SpaceScale { .. }
            | Self::SpaceReflect { .. }
            | Self::Degrade { .. }
            | Self::Deduplicate { .. }
            | Self::PriorityMerge { .. }
            | Self::WeightedChoice { .. }
            | Self::MaskClip { .. }
            | Self::Concat { .. } => PatternStreamShape::Event,
        }
    }

    /// Cycle span this node occupies when sequenced in a [`Self::Concat`] chain.
    ///
    /// Matches [`Self::flatten`] / [`PatternStream::chain`] sequencing semantics.
    pub fn duration(&self) -> CycleDuration {
        self.flatten().duration()
    }

    /// Preview flattening. Structural nodes preserve semantics where Cadence depends on them;
    /// `WeightedChoice` and `CycleRoute` still layer all branches for host preview.
    pub fn flatten(&self) -> PatternStream {
        match self {
            Self::EventStream(node) => node.stream.clone(),
            Self::ControlStream(node) => node.stream.to_pattern_stream(),
            Self::ScalarStream(node) => node.stream.to_pattern_stream(),
            Self::Merge { children } => {
                PatternStream::layer(children.iter().map(Self::flatten).collect())
            }
            Self::CycleRoute { children } => {
                PatternStream::layer(children.iter().map(Self::flatten).collect())
            }
            Self::CycleSlots { children } => {
                PatternStream::layer(children.iter().map(Self::flatten).collect())
            }
            Self::TimeScale { inner, factor } => {
                if *factor <= Rational::zero() {
                    inner.flatten()
                } else {
                    inner.flatten().slow(*factor)
                }
            }
            Self::Shift { inner, offset } => inner.flatten().shift_by(*offset),
            Self::ReflectCycle { inner } => inner.flatten().reverse(),
            Self::SpaceShift { inner, offset } => inner.flatten().space_shift(*offset),
            Self::SpaceScale { inner, factor } => inner.flatten().space_scale(*factor),
            Self::SpaceReflect { inner, axis } => inner.flatten().space_reflect(*axis),
            Self::Degrade {
                inner,
                keep_probability,
                seed,
            } => inner.flatten().degrade_by_policy(*keep_probability, *seed),
            Self::Deduplicate { inner, .. } => inner.flatten(),
            Self::PriorityMerge { children, .. } => {
                PatternStream::layer(children.iter().map(Self::flatten).collect())
            }
            Self::WeightedChoice { options, .. } => {
                PatternStream::layer(options.iter().map(|option| option.node.flatten()).collect())
            }
            Self::MaskClip { source, mask } => source.flatten().clip_by_mask(&mask.flatten()),
            Self::Concat { children } => children
                .iter()
                .fold(PatternStream::default(), |acc, child| {
                    PatternStream::chain(acc, child.flatten())
                }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightedPatternIr {
    pub weight: Rational,
    pub node: Box<PatternNodeIr>,
}

impl WeightedPatternIr {
    pub fn new(weight: Rational, node: PatternNodeIr) -> Self {
        assert!(
            weight > Rational::zero(),
            "weighted pattern weight must be positive"
        );
        Self {
            weight,
            node: Box::new(node),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventStreamNodeIr {
    pub stream: PatternStream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStreamNodeIr {
    pub stream: ControlStream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScalarStreamNodeIr {
    pub stream: ScalarStream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternOutput {
    pub id: NodeId,
    pub root: PatternNodeIr,
}

impl PatternOutput {
    pub fn new(id: NodeId, root: PatternNodeIr) -> Self {
        Self { id, root }
    }

    pub fn shape(&self) -> PatternStreamShape {
        self.root.shape()
    }

    pub fn events(&self) -> Vec<PatternEvent> {
        self.root.flatten().events
    }

    pub fn stream(&self) -> PatternStream {
        self.root.flatten()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PatternIr {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<PatternOutput>,
}

impl PatternIr {
    pub fn new(outputs: Vec<PatternOutput>) -> Self {
        Self { outputs }
    }

    pub fn flat_outputs(&self) -> Vec<FlatPatternOutput> {
        self.outputs
            .iter()
            .map(|output| FlatPatternOutput {
                id: output.id.clone(),
                events: output.events(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlatPatternOutput {
    pub id: NodeId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<PatternEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: i64, duration: i64) -> CycleSpan {
        CycleSpan {
            start: CycleTime(Rational::from_integer(start)),
            duration: CycleDuration(Rational::from_integer(duration)),
        }
    }

    fn note(start: i64, duration: i64, value: &str) -> PatternEvent {
        PatternEvent::new(
            span(start, duration),
            EventValue::Note {
                value: value.into(),
                octave: None,
            },
        )
    }

    #[test]
    fn pattern_output_exposes_flat_events_from_structured_root() {
        let left = PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")]));
        let right = PatternNodeIr::event_stream(PatternStream::new(vec![note(1, 1, "b")]));
        let output =
            PatternOutput::new(NodeId::new("out"), PatternNodeIr::merge(vec![left, right]));
        let events = output.events();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn time_scale_node_flattens_by_scaling_stream_time() {
        let node = PatternNodeIr::time_scale(
            PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")])),
            Rational::from_integer(2),
        );
        let stream = node.flatten();
        assert_eq!(
            stream.events[0].span.duration,
            CycleDuration(Rational::from_integer(2))
        );
    }

    #[test]
    fn control_stream_has_control_shape_and_flat_scalar_view() {
        let control = ControlEvent::new(
            span(0, 1),
            ControlKeyIr::Gain,
            ControlValueIr::rational(Rational::new(1, 2)),
        );
        let node = PatternNodeIr::control_stream(ControlStream::new(vec![control]));
        assert_eq!(node.shape(), PatternStreamShape::Control);
        assert_eq!(node.flatten().events.len(), 1);
    }

    #[test]
    fn structural_degrade_applies_keep_probability_on_flatten() {
        let node = PatternNodeIr::degrade(
            PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")])),
            Rational::one(),
            42,
        );
        assert_eq!(node.flatten().events.len(), 1);
    }

    #[test]
    fn duration_matches_flatten_for_single_note() {
        let node = PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")]));
        assert_eq!(node.duration(), CycleDuration(Rational::one()));
        assert_eq!(node.duration(), node.flatten().duration());
    }

    #[test]
    fn duration_concat_sums_child_spans() {
        let left = PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")]));
        let right = PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "b")]));
        let node = PatternNodeIr::concat(vec![left, right]);
        assert_eq!(node.duration(), CycleDuration(Rational::from_integer(2)));
    }

    #[test]
    fn duration_nested_concat_matches_stretched_segment() {
        let segment = PatternNodeIr::concat(vec![
            PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")])),
            PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")])),
            PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")])),
            PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")])),
        ]);
        assert_eq!(segment.duration(), CycleDuration(Rational::from_integer(4)));
        let node = PatternNodeIr::concat(vec![segment.clone(), segment]);
        assert_eq!(node.duration(), CycleDuration(Rational::from_integer(8)));
    }

    #[test]
    fn duration_time_scale_matches_host_handoff_chain() {
        let slow_a = PatternNodeIr::time_scale(
            PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "a")])),
            Rational::from_integer(3),
        );
        let b = PatternNodeIr::event_stream(PatternStream::new(vec![note(0, 1, "b")]));
        let node = PatternNodeIr::concat(vec![slow_a, b]);
        assert_eq!(node.duration(), CycleDuration(Rational::from_integer(4)));
        let events = node.flatten().events;
        assert_eq!(events[0].span.duration.0, Rational::from_integer(3));
        assert_eq!(events[1].span.start.0, Rational::from_integer(3));
    }
}
