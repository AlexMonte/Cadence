use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use crate::code_expr::CodeExpr;
use crate::types::{PieceCategory, PortType, TileSide};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamValueKind {
    Number,
    Text,
    Bool,
    Json,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamInlineMode {
    Literal,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParamSchema {
    Number {
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
        can_inline: bool,
    },
    Text {
        default: String,
        can_inline: bool,
    },
    /// Dropdown selection from a fixed set of options.
    Enum {
        options: Vec<String>,
        default: String,
        can_inline: bool,
    },
    /// Boolean toggle.
    Bool {
        default: bool,
        can_inline: bool,
    },
    /// User-defined port typing and inline/default expression semantics.
    Custom {
        port_type: PortType,
        value_kind: ParamValueKind,
        default: Option<Value>,
        can_inline: bool,
        inline_mode: ParamInlineMode,
        min: Option<f64>,
        max: Option<f64>,
    },
}

impl ParamSchema {
    pub fn accepts(&self, port_type: &PortType) -> bool {
        match self {
            ParamSchema::Number { .. } => PortType::number().accepts(port_type),
            ParamSchema::Text { .. } => PortType::text().accepts(port_type),
            ParamSchema::Enum { .. } => PortType::text().accepts(port_type),
            ParamSchema::Bool { .. } => {
                PortType::bool().accepts(port_type) || PortType::number().accepts(port_type)
            }
            ParamSchema::Custom {
                port_type: expected,
                ..
            } => expected.accepts(port_type),
        }
    }

    pub fn can_inline(&self) -> bool {
        match self {
            ParamSchema::Number { can_inline, .. }
            | ParamSchema::Text { can_inline, .. }
            | ParamSchema::Enum { can_inline, .. }
            | ParamSchema::Bool { can_inline, .. } => *can_inline,
            ParamSchema::Custom { can_inline, .. } => *can_inline,
        }
    }

    pub fn default_expr(&self) -> Option<CodeExpr> {
        match self {
            ParamSchema::Number { default, .. } => Some(CodeExpr::Literal(Value::Number(
                Number::from_f64(*default).unwrap_or_else(|| Number::from(0)),
            ))),
            ParamSchema::Text { default, .. } => {
                Some(CodeExpr::Literal(Value::String(default.clone())))
            }
            ParamSchema::Enum { default, .. } => {
                Some(CodeExpr::Literal(Value::String(default.clone())))
            }
            ParamSchema::Bool { default, .. } => Some(CodeExpr::Literal(Value::Bool(*default))),
            ParamSchema::Custom {
                default,
                inline_mode,
                ..
            } => default
                .as_ref()
                .and_then(|value| value_to_expr(value, inline_mode)),
        }
    }

    pub fn expected_port_type(&self) -> PortType {
        match self {
            ParamSchema::Number { .. } => PortType::number(),
            ParamSchema::Text { .. } => PortType::text(),
            ParamSchema::Enum { .. } => PortType::text(),
            ParamSchema::Bool { .. } => PortType::bool(),
            ParamSchema::Custom { port_type, .. } => port_type.clone(),
        }
    }

    pub fn validate_inline_value(&self, value: &Value) -> bool {
        match self {
            ParamSchema::Number { min, max, .. } => validate_number(value, *min, *max),
            ParamSchema::Text { .. } => value.is_string(),
            ParamSchema::Enum { options, .. } => value
                .as_str()
                .map_or(false, |s| options.contains(&s.to_string())),
            ParamSchema::Bool { .. } => value.is_boolean(),
            ParamSchema::Custom {
                value_kind,
                min,
                max,
                ..
            } => match value_kind {
                ParamValueKind::Number => validate_number(value, *min, *max),
                ParamValueKind::Text => value.is_string(),
                ParamValueKind::Bool => value.is_boolean(),
                ParamValueKind::Json => true,
                ParamValueKind::None => false,
            },
        }
    }

    pub fn inline_expr(&self, value: &Value) -> Option<CodeExpr> {
        if !self.validate_inline_value(value) {
            return None;
        }
        match self {
            ParamSchema::Custom { inline_mode, .. } => value_to_expr(value, inline_mode),
            _ => Some(CodeExpr::Literal(value.clone())),
        }
    }
}

fn validate_number(value: &Value, min: Option<f64>, max: Option<f64>) -> bool {
    let Some(number) = value.as_f64() else {
        return false;
    };
    if let Some(min) = min {
        if number < min {
            return false;
        }
    }
    if let Some(max) = max {
        if number > max {
            return false;
        }
    }
    true
}

fn value_to_expr(value: &Value, inline_mode: &ParamInlineMode) -> Option<CodeExpr> {
    match inline_mode {
        ParamInlineMode::Literal => Some(CodeExpr::Literal(value.clone())),
        ParamInlineMode::Raw => value.as_str().map(|raw| CodeExpr::Raw(raw.to_string())),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: String,
    pub label: String,
    pub side: TileSide,
    pub schema: ParamSchema,
    /// Optional grouping key for variadic fan-in. Params that share this key are
    /// exposed as an ordered vector to pieces during compile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variadic_group: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieceDef {
    pub id: String,
    pub label: String,
    pub category: PieceCategory,
    #[serde(default)]
    pub params: Vec<ParamDef>,
    pub output_type: Option<PortType>,
    pub output_side: Option<TileSide>,
    pub description: Option<String>,
}

impl PieceDef {
    pub fn is_terminal(&self) -> bool {
        self.output_type.is_none() || matches!(self.category, PieceCategory::Output)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PieceInputs {
    #[serde(default)]
    pub scalar: BTreeMap<String, CodeExpr>,
    #[serde(default)]
    pub variadic: BTreeMap<String, Vec<CodeExpr>>,
}

impl PieceInputs {
    pub fn get(&self, key: &str) -> Option<&CodeExpr> {
        self.scalar.get(key)
    }

    pub fn get_variadic(&self, key: &str) -> Option<&Vec<CodeExpr>> {
        self.variadic.get(key)
    }
}

pub trait Piece: Send + Sync {
    fn def(&self) -> &PieceDef;

    fn compile(&self, inputs: &PieceInputs, inline_params: &BTreeMap<String, Value>) -> CodeExpr;

    fn initial_state(&self) -> Option<Value> {
        None
    }

    fn compile_stateful(
        &self,
        inputs: &PieceInputs,
        inline_params: &BTreeMap<String, Value>,
        state: &Value,
    ) -> (CodeExpr, Value) {
        (self.compile(inputs, inline_params), state.clone())
    }
}
