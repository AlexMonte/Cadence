//! Canonical Cadence host IR.
//!
//! This is the semantic truth for the replacement architecture. Debug strings
//! and runtime voices are derived from this typed program, not the other way
//! around.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::common::{GridPos, PortType};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadenceSourceKind {
    Sample,
    Synth,
    Note,
    Pulse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadenceSynthSource {
    Sine,
    Triangle,
    Saw,
    Square,
}

impl CadenceSynthSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Triangle => "triangle",
            Self::Saw => "saw",
            Self::Square => "square",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "sine" => Some(Self::Sine),
            "triangle" => Some(Self::Triangle),
            "saw" => Some(Self::Saw),
            "square" => Some(Self::Square),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadenceValueExpr {
    Number {
        value: f64,
    },
    Text {
        value: String,
    },
    SynthSource {
        source: CadenceSynthSource,
    },
    Bool {
        value: bool,
    },
    Bundle {
        #[serde(default)]
        items: Vec<Option<CadenceValueExpr>>,
    },
    Argument {
        slot: u8,
        label: String,
    },
}

impl CadenceValueExpr {
    pub fn number(value: f64) -> Self {
        Self::Number { value }
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::Text {
            value: value.into(),
        }
    }

    pub fn synth_source(source: CadenceSynthSource) -> Self {
        Self::SynthSource { source }
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool { value }
    }

    pub fn bundle(items: Vec<Option<CadenceValueExpr>>) -> Self {
        Self::Bundle { items }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadenceStateExpr {
    DelayCell {
        default: Box<CadencePatternExpr>,
        feedback: Box<CadencePatternExpr>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadenceCallArg {
    Value { value: CadenceValueExpr },
    Pattern { pattern: CadencePatternExpr },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceCallExpr {
    pub trick_id: String,
    pub trick_name: String,
    #[serde(default)]
    pub arguments: Vec<CadenceCallBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceCallBinding {
    pub param: String,
    pub arg: CadenceCallArg,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceSubgraphInput {
    pub slot: u8,
    pub pos: GridPos,
    pub label: String,
    pub port_type: PortType,
    pub required: bool,
    pub is_receiver: bool,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceSubgraphSignature {
    #[serde(default)]
    pub inputs: Vec<CadenceSubgraphInput>,
    pub output_pos: GridPos,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_type: Option<PortType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadencePatternExpr {
    Silence,
    Argument {
        slot: u8,
        label: String,
    },
    Source {
        source: CadenceSourceKind,
        value: CadenceValueExpr,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        site: Option<GridPos>,
    },
    Merge {
        #[serde(default)]
        branches: Vec<CadencePatternExpr>,
    },
    CycleRoute {
        #[serde(default)]
        branches: Vec<CadencePatternExpr>,
    },
    CycleSlots {
        #[serde(default)]
        branches: Vec<CadencePatternExpr>,
    },
    ReflectCycle {
        input: Box<CadencePatternExpr>,
    },
    Mask {
        input: Box<CadencePatternExpr>,
        by: Box<CadencePatternExpr>,
    },
    Gain {
        input: Box<CadencePatternExpr>,
        amount: CadenceValueExpr,
    },
    Fast {
        input: Box<CadencePatternExpr>,
        factor: CadenceValueExpr,
    },
    Slow {
        input: Box<CadencePatternExpr>,
        factor: CadenceValueExpr,
    },
    Shift {
        input: Box<CadencePatternExpr>,
        offset: CadenceValueExpr,
    },
    Control {
        input: Box<CadencePatternExpr>,
        key: CadenceControlKey,
        value: CadenceControlValueExpr,
    },
    Call {
        call: CadenceCallExpr,
    },
    State {
        state: CadenceStateExpr,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceFunction {
    pub trick_id: String,
    pub trick_name: String,
    pub signature: CadenceSubgraphSignature,
    pub body: CadencePatternExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceOutput {
    pub root: GridPos,
    pub expr: CadencePatternExpr,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CadenceProgram {
    #[serde(default)]
    pub functions: Vec<CadenceFunction>,
    #[serde(default)]
    pub outputs: Vec<CadenceOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadenceControlKey {
    Gate,
    SampleBank,
    SampleVariant,
    Pitch,
    Velocity,
    Legato,
    Attack,
    Decay,
    Sustain,
    Release,
    Gain,
    Pan,
    PlaybackRate,
    ClipLength,
    PlaybackStart,
    PlaybackEnd,
    Reverse,
    LowPassCutoff,
    LowPassResonance,
    HighPassCutoff,
    HighPassResonance,
    PostGain,
    ReverbSend,
    DelaySend,
    Compressor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadenceControlValueExpr {
    Scalar {
        value: CadenceValueExpr,
    },
    Choice {
        value: CadenceValueExpr,
    },
    Bool {
        value: bool,
    },
    Reverb {
        amount: CadenceValueExpr,
        decay: CadenceValueExpr,
        damping: CadenceValueExpr,
    },
    Delay {
        amount: CadenceValueExpr,
        time: CadenceValueExpr,
        feedback: CadenceValueExpr,
        damping: CadenceValueExpr,
    },
    Compressor {
        threshold: CadenceValueExpr,
        ratio: CadenceValueExpr,
        attack: CadenceValueExpr,
        release: CadenceValueExpr,
    },
    /// A time-varying control value driven by a pattern expression.
    ///
    /// Each step in the projected pattern contributes one scalar value to the
    /// control track at runtime, enabling per-event parameter modulation
    /// (e.g. a gain or pitch sequence). The host projects `expr` to a
    /// one-cycle window and maps each event's label to a [`ControlValue`].
    Pattern {
        /// The pattern expression whose projected events supply per-step values.
        expr: Box<CadencePatternExpr>,
    },
}

impl CadenceProgram {
    pub fn render_debug(&self) -> Vec<String> {
        self.outputs
            .iter()
            .map(|output| render_pattern_expr(&output.expr))
            .collect()
    }

    pub fn sample_selectors(&self) -> Vec<String> {
        let mut selectors = Vec::new();
        for output in &self.outputs {
            collect_sample_selectors(&output.expr, &mut selectors);
        }
        selectors.sort();
        selectors.dedup();
        selectors
    }
}

pub fn render_value_expr(expr: &CadenceValueExpr) -> String {
    match expr {
        CadenceValueExpr::Number { value } => {
            if value.fract() == 0.0 {
                (*value as i64).to_string()
            } else {
                value.to_string()
            }
        }
        CadenceValueExpr::Text { value } => format!("{value:?}"),
        CadenceValueExpr::SynthSource { source } => source.as_str().to_string(),
        CadenceValueExpr::Bool { value } => value.to_string(),
        CadenceValueExpr::Bundle { items } => {
            let rendered = items
                .iter()
                .map(|item| {
                    item.as_ref()
                        .map(render_value_expr)
                        .unwrap_or_else(|| "_".to_string())
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{rendered}]")
        }
        CadenceValueExpr::Argument { slot, label } => format!("${slot}:{label}"),
    }
}

pub fn render_pattern_expr(expr: &CadencePatternExpr) -> String {
    match expr {
        CadencePatternExpr::Silence => "silence".to_string(),
        CadencePatternExpr::Argument { slot, label } => format!("${slot}:{label}"),
        CadencePatternExpr::Source { source, value, .. } => {
            let head = match source {
                CadenceSourceKind::Sample => "sample",
                CadenceSourceKind::Synth => "synth",
                CadenceSourceKind::Note => "note",
                CadenceSourceKind::Pulse => "pulse",
            };
            format!("{head}({})", render_value_expr(value))
        }
        CadencePatternExpr::Merge { branches } => render_branches("overlay", branches),
        CadencePatternExpr::CycleRoute { branches } => render_branches("arrange", branches),
        CadencePatternExpr::CycleSlots { branches } => render_branches("polymeter", branches),
        CadencePatternExpr::ReflectCycle { input } => {
            format!("mirror({})", render_pattern_expr(input))
        }
        CadencePatternExpr::Mask { input, by } => format!(
            "mask({}, {})",
            render_pattern_expr(input),
            render_pattern_expr(by)
        ),
        CadencePatternExpr::Gain { input, amount } => format!(
            "gain({}, {})",
            render_value_expr(amount),
            render_pattern_expr(input)
        ),
        CadencePatternExpr::Fast { input, factor } => format!(
            "speed({}, {})",
            render_value_expr(factor),
            render_pattern_expr(input)
        ),
        CadencePatternExpr::Slow { input, factor } => format!(
            "stretch({}, {})",
            render_value_expr(factor),
            render_pattern_expr(input)
        ),
        CadencePatternExpr::Shift { input, offset } => format!(
            "shift({}, {})",
            render_value_expr(offset),
            render_pattern_expr(input)
        ),
        CadencePatternExpr::Control { input, key, value } => format!(
            "{}({}, {})",
            render_control_key(*key),
            render_control_value_expr(value),
            render_pattern_expr(input)
        ),
        CadencePatternExpr::Call { call } => {
            let args = call
                .arguments
                .iter()
                .map(|binding| match &binding.arg {
                    CadenceCallArg::Value { value } => {
                        format!("{}={}", binding.param, render_value_expr(value))
                    }
                    CadenceCallArg::Pattern { pattern } => {
                        format!("{}={}", binding.param, render_pattern_expr(pattern))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({args})", call.trick_name)
        }
        CadencePatternExpr::State { state } => match state {
            CadenceStateExpr::DelayCell { default, feedback } => format!(
                "delay(default={}, feedback={})",
                render_pattern_expr(default),
                render_pattern_expr(feedback)
            ),
        },
    }
}

fn collect_sample_selectors(expr: &CadencePatternExpr, selectors: &mut Vec<String>) {
    match expr {
        CadencePatternExpr::Silence => {}
        CadencePatternExpr::Argument { .. } => {}
        CadencePatternExpr::Source {
            source: CadenceSourceKind::Sample,
            value: CadenceValueExpr::Text { value },
            ..
        } => selectors.push(value.clone()),
        CadencePatternExpr::Source { .. } => {}
        CadencePatternExpr::Merge { branches }
        | CadencePatternExpr::CycleRoute { branches }
        | CadencePatternExpr::CycleSlots { branches } => {
            for branch in branches {
                collect_sample_selectors(branch, selectors);
            }
        }
        CadencePatternExpr::ReflectCycle { input } => collect_sample_selectors(input, selectors),
        CadencePatternExpr::Mask { input, by } => {
            collect_sample_selectors(input, selectors);
            collect_sample_selectors(by, selectors);
        }
        CadencePatternExpr::Gain { input, .. }
        | CadencePatternExpr::Fast { input, .. }
        | CadencePatternExpr::Slow { input, .. }
        | CadencePatternExpr::Shift { input, .. }
        | CadencePatternExpr::Control { input, .. } => collect_sample_selectors(input, selectors),
        CadencePatternExpr::Call { call } => {
            for binding in &call.arguments {
                if let CadenceCallArg::Pattern { pattern } = &binding.arg {
                    collect_sample_selectors(pattern, selectors);
                }
            }
        }
        CadencePatternExpr::State { state } => match state {
            CadenceStateExpr::DelayCell { default, feedback } => {
                collect_sample_selectors(default, selectors);
                collect_sample_selectors(feedback, selectors);
            }
        },
    }
}

fn render_branches(name: &str, branches: &[CadencePatternExpr]) -> String {
    let rendered = branches
        .iter()
        .map(render_pattern_expr)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({rendered})")
}

fn render_control_key(key: CadenceControlKey) -> &'static str {
    match key {
        CadenceControlKey::Gate => "gate",
        CadenceControlKey::SampleBank => "sample_bank",
        CadenceControlKey::SampleVariant => "sample_variant",
        CadenceControlKey::Pitch => "note",
        CadenceControlKey::Velocity => "velocity",
        CadenceControlKey::Legato => "legato",
        CadenceControlKey::Attack => "attack",
        CadenceControlKey::Decay => "decay",
        CadenceControlKey::Sustain => "sustain",
        CadenceControlKey::Release => "release",
        CadenceControlKey::Gain => "gain",
        CadenceControlKey::Pan => "pan",
        CadenceControlKey::PlaybackRate => "playback_rate",
        CadenceControlKey::ClipLength => "clip",
        CadenceControlKey::PlaybackStart => "playback_start",
        CadenceControlKey::PlaybackEnd => "playback_end",
        CadenceControlKey::Reverse => "reverse",
        CadenceControlKey::LowPassCutoff => "lowpass_cutoff",
        CadenceControlKey::LowPassResonance => "lowpass_resonance",
        CadenceControlKey::HighPassCutoff => "highpass_cutoff",
        CadenceControlKey::HighPassResonance => "highpass_resonance",
        CadenceControlKey::PostGain => "postgain",
        CadenceControlKey::ReverbSend => "reverb_send",
        CadenceControlKey::DelaySend => "delay",
        CadenceControlKey::Compressor => "compressor",
    }
}

fn render_control_value_expr(expr: &CadenceControlValueExpr) -> String {
    match expr {
        CadenceControlValueExpr::Scalar { value } | CadenceControlValueExpr::Choice { value } => {
            render_value_expr(value)
        }
        CadenceControlValueExpr::Bool { value } => value.to_string(),
        CadenceControlValueExpr::Reverb {
            amount,
            decay,
            damping,
        } => format!(
            "amount={}, decay={}, damping={}",
            render_value_expr(amount),
            render_value_expr(decay),
            render_value_expr(damping)
        ),
        CadenceControlValueExpr::Delay {
            amount,
            time,
            feedback,
            damping,
        } => format!(
            "amount={}, time={}, feedback={}, damping={}",
            render_value_expr(amount),
            render_value_expr(time),
            render_value_expr(feedback),
            render_value_expr(damping)
        ),
        CadenceControlValueExpr::Compressor {
            threshold,
            ratio,
            attack,
            release,
        } => format!(
            "threshold={}, ratio={}, attack={}, release={}",
            render_value_expr(threshold),
            render_value_expr(ratio),
            render_value_expr(attack),
            render_value_expr(release)
        ),
        CadenceControlValueExpr::Pattern { expr } => {
            format!("pattern({})", render_pattern_expr(expr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_debug_render_is_deterministic() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 1, row: 0 },
                expr: CadencePatternExpr::Fast {
                    input: Box::new(CadencePatternExpr::Source {
                        source: CadenceSourceKind::Sample,
                        value: CadenceValueExpr::text("bd"),
                        site: None,
                    }),
                    factor: CadenceValueExpr::number(2.0),
                },
            }],
            functions: Vec::new(),
        };

        assert_eq!(program.render_debug(), vec!["speed(2, sample(\"bd\"))"]);
        assert_eq!(program.sample_selectors(), vec!["bd".to_string()]);
    }

    #[test]
    fn typed_synth_sources_do_not_register_as_sample_selectors() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 0, row: 0 },
                expr: CadencePatternExpr::Source {
                    source: CadenceSourceKind::Synth,
                    value: CadenceValueExpr::synth_source(CadenceSynthSource::Sine),
                    site: None,
                },
            }],
            functions: Vec::new(),
        };

        assert!(program.sample_selectors().is_empty());
    }

    #[test]
    fn mixed_source_program_collects_only_sample_selectors() {
        let program = CadenceProgram {
            outputs: vec![CadenceOutput {
                root: GridPos { col: 0, row: 0 },
                expr: CadencePatternExpr::CycleSlots {
                    branches: vec![
                        CadencePatternExpr::Source {
                            source: CadenceSourceKind::Synth,
                            value: CadenceValueExpr::synth_source(CadenceSynthSource::Sine),
                            site: None,
                        },
                        CadencePatternExpr::Source {
                            source: CadenceSourceKind::Sample,
                            value: CadenceValueExpr::text("bd"),
                            site: None,
                        },
                        CadencePatternExpr::Source {
                            source: CadenceSourceKind::Synth,
                            value: CadenceValueExpr::synth_source(CadenceSynthSource::Square),
                            site: None,
                        },
                    ],
                },
            }],
            functions: Vec::new(),
        };

        assert_eq!(program.sample_selectors(), vec!["bd".to_string()]);
        assert_eq!(
            program.render_debug(),
            vec!["polymeter(synth(sine), sample(\"bd\"), synth(square))"]
        );
    }
}
