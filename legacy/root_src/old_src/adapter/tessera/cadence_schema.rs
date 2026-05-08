use tessera::piece::{ParamInlineMode, ParamSchema, ParamValueKind};
use tessera::types::PortType;

const PATTERN_PORT: &str = "pattern";

pub fn pattern_port() -> PortType {
    PortType::new(PATTERN_PORT)
}

pub fn pattern_schema() -> ParamSchema {
    ParamSchema::ParamSchema::Custom {
        port_type: pattern_port(),
        value_kind: ParamValueKind::None,
        default: None,
        can_inline: false,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}
