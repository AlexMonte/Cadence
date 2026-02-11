use crate::core::{NodeId, ScopeId};
use serde::{Deserialize, Serialize};

/// Typed intermediate representation for one playable scope.
///
/// This keeps runtime codegen deterministic and allows future emitters
/// (Strudel text, validation, analysis) to share one canonical shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeProgram {
    pub scope_id: ScopeId,
    pub pattern: PatternOp,
    pub transforms: Vec<TransformOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternOp {
    pub source_node: NodeId,
    pub expr: PatternExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatternExpr {
    Sequence { notes: String },
    Sample { value: String },
    Mini { value: String },
    StepGrid(StepGridExpr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepGridExpr {
    pub rows: usize,
    pub cols: usize,
    pub bits: Vec<bool>,
    pub default_sample: String,
    pub row_samples: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocities: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accents: Option<Vec<bool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probabilities: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformOp {
    pub source_node: NodeId,
    pub expr: TransformExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransformExpr {
    Speed(f64),
    Gain(f64),
    Pan(f64),
}
