use std::collections::BTreeMap;

use serde_json::Value;

use super::strudel_piece_def;
use tessera::ast::Expr;
use tessera::piece::{
    ParamDef, ParamInlineMode, ParamSchema, ParamValueKind, Piece, PieceDef, PieceInputs,
};
use tessera::types::{PieceCategory, PortType, TileSide};

pub struct ConnectorPiece {
    def: PieceDef,
}

impl ConnectorPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "connector",
                "conn",
                PieceCategory::Connector,
                vec![ParamDef {
                    id: "target".into(),
                    label: "in".into(),
                    side: TileSide::LEFT,
                    schema: ParamSchema::Custom {
                        port_type: PortType::any(),
                        value_kind: ParamValueKind::None,
                        default: None,
                        can_inline: false,
                        inline_mode: ParamInlineMode::Literal,
                        min: None,
                        max: None,
                    },
                    text_semantics: Default::default(),
                    variadic_group: None,
                    required: true,
                }],
                Some(PortType::any()),
                Some(TileSide::RIGHT),
                "Pass-through connector. Routes a value from one side to the opposite side unchanged.",
            ),
        }
    }
}

impl Piece for ConnectorPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> Expr {
        inputs
            .get("target")
            .cloned()
            .unwrap_or_else(|| Expr::error("missing connector input"))
    }
}

pub struct CrossConnectorPiece {
    def: PieceDef,
}

impl CrossConnectorPiece {
    pub fn new() -> Self {
        Self {
            def: strudel_piece_def(
                "cross_connector",
                "x-conn",
                PieceCategory::Connector,
                vec![
                    ParamDef {
                        id: "h_in".into(),
                        label: "h".into(),
                        side: TileSide::LEFT,
                        schema: ParamSchema::Custom {
                            port_type: PortType::any(),
                            value_kind: ParamValueKind::None,
                            default: None,
                            can_inline: false,
                            inline_mode: ParamInlineMode::Literal,
                            min: None,
                            max: None,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                    ParamDef {
                        id: "v_in".into(),
                        label: "v".into(),
                        side: TileSide::TOP,
                        schema: ParamSchema::Custom {
                            port_type: PortType::any(),
                            value_kind: ParamValueKind::None,
                            default: None,
                            can_inline: false,
                            inline_mode: ParamInlineMode::Literal,
                            min: None,
                            max: None,
                        },
                        text_semantics: Default::default(),
                        variadic_group: None,
                        required: false,
                    },
                ],
                Some(PortType::any()),
                None,
                "Cross connector. Routes two independent channels: horizontal (left→right) and vertical (top→bottom).",
            ),
        }
    }
}

impl Piece for CrossConnectorPiece {
    fn def(&self) -> &PieceDef {
        &self.def
    }

    fn compile(&self, inputs: &PieceInputs, _inline_params: &BTreeMap<String, Value>) -> Expr {
        inputs
            .get("h_in")
            .or_else(|| inputs.get("v_in"))
            .cloned()
            .unwrap_or_else(|| Expr::error("missing cross-connector input"))
    }

    fn compile_multi_output(
        &self,
        inputs: &PieceInputs,
        _inline_params: &BTreeMap<String, Value>,
    ) -> Option<BTreeMap<TileSide, Expr>> {
        let mut outputs = BTreeMap::new();
        if let Some(h) = inputs.get("h_in").cloned() {
            outputs.insert(TileSide::RIGHT, h);
        }
        if let Some(v) = inputs.get("v_in").cloned() {
            outputs.insert(TileSide::BOTTOM, v);
        }
        if outputs.is_empty() {
            None
        } else {
            Some(outputs)
        }
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

impl_default_from_new!(ConnectorPiece, CrossConnectorPiece);
