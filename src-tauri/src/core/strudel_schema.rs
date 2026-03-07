use serde_json::Value;

use tile_graph::piece::{ParamInlineMode, ParamSchema, ParamValueKind};
use tile_graph::types::PortType;

const PATTERN_PORT: &str = "pattern";
const RHYTHM_PORT: &str = "rhythm";
#[allow(dead_code)]
const TRIGGER_PORT: &str = "trigger";
#[allow(dead_code)]
const SIGNAL_PORT: &str = "signal";

pub fn pattern_port() -> PortType {
    PortType::new(PATTERN_PORT)
}

pub fn rhythm_port() -> PortType {
    PortType::new(RHYTHM_PORT)
}

#[allow(dead_code)]
pub fn trigger_port() -> PortType {
    PortType::new(TRIGGER_PORT)
}

#[allow(dead_code)]
pub fn signal_port() -> PortType {
    PortType::new(SIGNAL_PORT)
}

pub fn bool_port() -> PortType {
    PortType::bool()
}

pub fn number_port() -> PortType {
    PortType::number()
}

pub fn text_port() -> PortType {
    PortType::text()
}

pub fn pattern_schema() -> ParamSchema {
    ParamSchema::Custom {
        port_type: pattern_port(),
        value_kind: ParamValueKind::Text,
        default: None,
        can_inline: false,
        inline_mode: ParamInlineMode::Raw,
        min: None,
        max: None,
    }
}

pub fn rhythm_schema(default: impl Into<String>, can_inline: bool) -> ParamSchema {
    ParamSchema::Custom {
        port_type: rhythm_port(),
        value_kind: ParamValueKind::Text,
        default: Some(Value::String(default.into())),
        can_inline,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}

#[allow(dead_code)]
pub fn trigger_schema() -> ParamSchema {
    ParamSchema::Custom {
        port_type: trigger_port(),
        value_kind: ParamValueKind::None,
        default: None,
        can_inline: false,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}

#[allow(dead_code)]
pub fn signal_schema() -> ParamSchema {
    ParamSchema::Custom {
        port_type: signal_port(),
        value_kind: ParamValueKind::Number,
        default: None,
        can_inline: false,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}

pub fn json_schema(default: Option<Value>, can_inline: bool) -> ParamSchema {
    ParamSchema::Custom {
        port_type: PortType::any(),
        value_kind: ParamValueKind::Json,
        default,
        can_inline,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}

pub fn schema_for_port_type(
    port_type: &PortType,
    default: Option<Value>,
    can_inline: bool,
) -> ParamSchema {
    match port_type.as_str() {
        PATTERN_PORT => ParamSchema::Custom {
            port_type: pattern_port(),
            value_kind: ParamValueKind::Text,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Raw,
            min: None,
            max: None,
        },
        RHYTHM_PORT => ParamSchema::Custom {
            port_type: rhythm_port(),
            value_kind: ParamValueKind::Text,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
        TRIGGER_PORT => ParamSchema::Custom {
            port_type: trigger_port(),
            value_kind: ParamValueKind::None,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
        SIGNAL_PORT => ParamSchema::Custom {
            port_type: signal_port(),
            value_kind: ParamValueKind::Number,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
        "number" => ParamSchema::Custom {
            port_type: number_port(),
            value_kind: ParamValueKind::Number,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
        "bool" => ParamSchema::Custom {
            port_type: bool_port(),
            value_kind: ParamValueKind::Bool,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
        "text" => ParamSchema::Custom {
            port_type: text_port(),
            value_kind: ParamValueKind::Text,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
        _ => ParamSchema::Custom {
            port_type: port_type.clone(),
            value_kind: ParamValueKind::Json,
            default,
            can_inline,
            inline_mode: ParamInlineMode::Literal,
            min: None,
            max: None,
        },
    }
}
