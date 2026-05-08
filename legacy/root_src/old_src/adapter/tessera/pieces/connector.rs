use std::collections::BTreeMap;

use tessera::piece::{ParamDef, ParamInlineMode, ParamSchema, ParamValueKind, Piece, PieceDef};
use tessera::types::{PieceCategory, PortType, TileSide};

use crate::adapter::tessera::pieces::cadence_piece_def_with_tags;

pub struct ConnectorPiece {
    def: PieceDef,
}

impl ConnectorPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "connector",
                "conn",
                PieceCategory::Connector,
                vec![ParamDef {
                    id: "target".into(),
                    label: "in".into(),
                    side: TileSide::LEFT,
                    schema: passthrough_schema(),
                    text_semantics: Default::default(),
                    variadic_group: None,
                    required: true,
                    role: Default::default(),
                }],
                Some(PortType::any()),
                Some(TileSide::RIGHT),
                "Pass-through connector. Routes a value from one side to the opposite side unchanged.",
                vec!["pipe", "cable"],
            ),
        }
    }
}

impl Piece for ConnectorPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn infer_output_type(
        &self,
        input_types: &BTreeMap<String, PortType>,
        _inline_params: &BTreeMap<String, serde_json::Value>,
    ) -> Option<PortType> {
        input_types
            .get("target")
            .cloned()
            .or_else(|| self.def.output_type.clone())
    }
}

pub struct ArgsConnectorPiece {
    def: PieceDef,
}

impl ArgsConnectorPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "args_connector",
                "args",
                PieceCategory::Control,
                vec![
                    ordered_input_param("arg1", "1", TileSide::LEFT),
                    ordered_input_param("arg2", "2", TileSide::TOP),
                    ordered_input_param("arg3", "3", TileSide::BOTTOM),
                    ordered_input_param("arg4", "4", TileSide::RIGHT),
                ],
                Some(PortType::new("control_bundle")),
                Some(TileSide::RIGHT),
                "Ordered bundle connector. Packs connected values into one control bundle in slot order.",
                vec!["args", "pack", "vector", "bundle"],
            ),
        }
    }
}

impl Piece for ArgsConnectorPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn infer_output_type(
        &self,
        _input_types: &BTreeMap<String, PortType>,
        _inline_params: &BTreeMap<String, serde_json::Value>,
    ) -> Option<PortType> {
        Some(PortType::new("control_bundle"))
    }
}

#[allow(dead_code)]
pub struct CrossConnectorPiece {
    def: PieceDef,
}

#[allow(dead_code)]
impl CrossConnectorPiece {
    pub fn new() -> Self {
        Self {
            def: cadence_piece_def_with_tags(
                "cross_connector",
                "x-conn",
                PieceCategory::Connector,
                vec![
                    ParamDef {
                        id: "h_in".into(),
                        label: "h".into(),
                        side: TileSide::LEFT,
                        schema: passthrough_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                    ParamDef {
                        id: "v_in".into(),
                        label: "v".into(),
                        side: TileSide::TOP,
                        schema: passthrough_schema(),
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                        role: Default::default(),
                    },
                ],
                Some(PortType::any()),
                None,
                "Cross connector. Routes two independent channels through the same tile.",
                vec!["cross_wire"],
            ),
        }
    }
}

impl Piece for CrossConnectorPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn infer_output_type(
        &self,
        input_types: &BTreeMap<String, PortType>,
        _inline_params: &BTreeMap<String, serde_json::Value>,
    ) -> Option<PortType> {
        input_types
            .get("h_in")
            .cloned()
            .or_else(|| input_types.get("v_in").cloned())
            .or_else(|| self.def.output_type.clone())
    }
}

fn passthrough_schema() -> ParamSchema {
    ParamSchema::Custom {
        port_type: PortType::any(),
        value_kind: ParamValueKind::None,
        default: None,
        can_inline: false,
        inline_mode: ParamInlineMode::Literal,
        min: None,
        max: None,
    }
}

fn ordered_input_param(id: &str, label: &str, side: TileSide) -> ParamDef {
    ParamDef {
        id: id.into(),
        label: label.into(),
        side,
        schema: passthrough_schema(),
        text_semantics: Default::default(),
        variadic_group: Some("ordered_args".into()),
        required: false,
        role: Default::default(),
    }
}

macro_rules! impl_default_from_new {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Default for $ty {
                fn default() -> Self {
                    Self::new()
                }
            }
        )*
    };
}

impl_default_from_new!(ConnectorPiece, CrossConnectorPiece, ArgsConnectorPiece);
