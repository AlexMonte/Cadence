pub mod combinators;
pub mod connector;
pub mod constants;
pub mod generators;
pub mod terminal;
pub mod transforms;

use std::collections::BTreeMap;

use serde_json::Value;
use tessera::ast::{Expr, ExprKind, Lit};
use tessera::piece::PieceInputs;
use tessera::piece::{ParamDef, PieceDef};
use tessera::types::{PieceCategory, PieceSemanticKind, PortType, TileSide};

pub use combinators::{CatPiece, StackPiece};
pub use connector::{ConnectorPiece, CrossConnectorPiece};

pub use constants::{MiniPiece, NumberPiece, TextPiece};
pub use generators::{NPiece, NotePiece, SoundPiece};
pub use terminal::OutputPiece;
pub use transforms::{
    ApplyPiece, BankPiece, ClipPiece, FastPiece, GainPiece, MaskPiece, PanPiece, ReleasePiece,
    RevPiece, RoomPiece, ScalePiece, SizePiece, SlowPiece, StructPiece, SustainPiece,
    TransposePiece,
};

pub(super) fn strudel_piece_def(
    id: &str,
    label: &str,
    category: PieceCategory,
    params: Vec<ParamDef>,
    output_type: Option<PortType>,
    output_side: Option<TileSide>,
    description: &str,
) -> PieceDef {
    PieceDef {
        id: id.into(),
        label: label.into(),
        category,
        semantic_kind: strudel_semantic_kind(category),
        namespace: "strudel".into(),
        params,
        output_type,
        output_side,
        description: Some(description.into()),
        tags: Vec::new(),
    }
}

fn strudel_semantic_kind(category: PieceCategory) -> PieceSemanticKind {
    match category {
        PieceCategory::Constant => PieceSemanticKind::Literal,
        PieceCategory::Output => PieceSemanticKind::Output,
        PieceCategory::Connector => PieceSemanticKind::Connector,
        PieceCategory::Generator | PieceCategory::Transform => PieceSemanticKind::Intrinsic,
        PieceCategory::Control => PieceSemanticKind::Construct,
        PieceCategory::Trick => PieceSemanticKind::Trick,
    }
}

pub(super) fn resolve_param_expr(
    def: &PieceDef,
    id: &str,
    inputs: &PieceInputs,
    inline_params: &BTreeMap<String, Value>,
) -> Option<Expr> {
    let param = def.params.iter().find(|param| param.id == id)?;

    inputs
        .get(id)
        .cloned()
        .or_else(|| {
            inline_params
                .get(id)
                .and_then(|value| param.schema.inline_expr(value))
        })
        .or_else(|| param.schema.default_expr())
}

pub(super) fn require_param_expr(
    def: &PieceDef,
    id: &str,
    inputs: &PieceInputs,
    inline_params: &BTreeMap<String, Value>,
    message: &str,
) -> Expr {
    resolve_param_expr(def, id, inputs, inline_params).unwrap_or_else(|| Expr::error(message))
}

pub(super) fn expr_string_value(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Lit {
            value: Lit::Str { value, .. },
        } => Some(value.as_str()),
        _ => None,
    }
}
