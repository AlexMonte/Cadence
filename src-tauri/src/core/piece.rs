use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use crate::core::code_expr::CodeExpr;
use crate::core::types::{PieceCategory, PortType, TileSide};

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
    Pattern {
        can_inline: bool,
    },
    Rhythm {
        default: String,
        can_inline: bool,
    },
}

impl ParamSchema {
    pub fn accepts(&self, port_type: &PortType) -> bool {
        match self {
            ParamSchema::Number { .. } => matches!(port_type, PortType::Number | PortType::Any),
            ParamSchema::Text { .. } => matches!(port_type, PortType::Text | PortType::Any),
            ParamSchema::Pattern { .. } => matches!(port_type, PortType::Pattern | PortType::Any),
            ParamSchema::Rhythm { .. } => matches!(port_type, PortType::Rhythm | PortType::Any),
        }
    }

    pub fn can_inline(&self) -> bool {
        match self {
            ParamSchema::Number { can_inline, .. }
            | ParamSchema::Text { can_inline, .. }
            | ParamSchema::Pattern { can_inline }
            | ParamSchema::Rhythm { can_inline, .. } => *can_inline,
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
            ParamSchema::Rhythm { default, .. } => {
                Some(CodeExpr::Literal(Value::String(default.clone())))
            }
            ParamSchema::Pattern { .. } => None,
        }
    }

    pub fn expected_port_type(&self) -> PortType {
        match self {
            ParamSchema::Number { .. } => PortType::Number,
            ParamSchema::Text { .. } => PortType::Text,
            ParamSchema::Pattern { .. } => PortType::Pattern,
            ParamSchema::Rhythm { .. } => PortType::Rhythm,
        }
    }

    pub fn validate_inline_value(&self, value: &Value) -> bool {
        match self {
            ParamSchema::Number { min, max, .. } => {
                let Some(number) = value.as_f64() else {
                    return false;
                };
                if let Some(min) = min {
                    if number < *min {
                        return false;
                    }
                }
                if let Some(max) = max {
                    if number > *max {
                        return false;
                    }
                }
                true
            }
            ParamSchema::Text { .. } | ParamSchema::Pattern { .. } | ParamSchema::Rhythm { .. } => {
                value.is_string()
            }
        }
    }

    pub fn inline_expr(&self, value: &Value) -> Option<CodeExpr> {
        if !self.validate_inline_value(value) {
            return None;
        }
        match self {
            ParamSchema::Pattern { .. } => value.as_str().map(|raw| CodeExpr::Raw(raw.to_string())),
            _ => Some(CodeExpr::Literal(value.clone())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: String,
    pub label: String,
    pub side: TileSide,
    pub schema: ParamSchema,
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

pub trait Piece: Send + Sync {
    fn def(&self) -> &PieceDef;
    fn compile(
        &self,
        inputs: &BTreeMap<String, CodeExpr>,
        inline_params: &BTreeMap<String, Value>,
    ) -> CodeExpr;
}
