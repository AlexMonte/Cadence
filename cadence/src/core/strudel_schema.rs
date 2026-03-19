use tessera::piece::{ParamInlineMode, ParamSchema, ParamValueKind};
use tessera::types::PortType;

const PATTERN_PORT: &str = "pattern";
const RHYTHM_PORT: &str = "rhythm";

pub fn pattern_port() -> PortType {
    PortType::new(PATTERN_PORT)
}

fn rhythm_port() -> PortType {
    PortType::new(RHYTHM_PORT)
}

pub fn pattern_schema() -> ParamSchema {
    ParamSchema::Custom {
        port_type: pattern_port(),
        value_kind: ParamValueKind::Text,
        default: None,
        can_inline: false,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}

pub fn rhythm_schema(default: impl Into<String>, can_inline: bool) -> ParamSchema {
    ParamSchema::Custom {
        port_type: rhythm_port(),
        value_kind: ParamValueKind::Text,
        default: Some(serde_json::Value::String(default.into())),
        can_inline,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}
