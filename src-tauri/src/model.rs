use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tile_graph::graph::{Graph, GraphOpRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceProjectDocument {
    pub schema_version: u32,
    pub name: String,
    pub graph: Graph,
    #[serde(default)]
    pub init_stage: CadenceInitStage,
}

impl CadenceProjectDocument {
    pub const SCHEMA_VERSION: u32 = 3;

    pub fn new(name: String, graph: Graph) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            name,
            graph,
            init_stage: CadenceInitStage::default(),
        }
    }

    pub fn graph(&self, target: &CadenceGraphTarget) -> Option<&Graph> {
        match target {
            CadenceGraphTarget::Runtime => Some(&self.graph),
            CadenceGraphTarget::Trick { trick_id } => self
                .init_stage
                .tricks
                .iter()
                .find(|trick| trick.id == *trick_id)
                .map(|trick| &trick.graph),
        }
    }

    pub fn graph_mut(&mut self, target: &CadenceGraphTarget) -> Option<&mut Graph> {
        match target {
            CadenceGraphTarget::Runtime => Some(&mut self.graph),
            CadenceGraphTarget::Trick { trick_id } => self
                .init_stage
                .tricks
                .iter_mut()
                .find(|trick| trick.id == *trick_id)
                .map(|trick| &mut trick.graph),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CadenceInitStage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cps_expr: Option<String>,
    #[serde(default)]
    pub sample_loads: Vec<CadenceSampleLoad>,
    #[serde(default)]
    pub tricks: Vec<CadenceTrickDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadenceSampleLoad {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceTrickDef {
    pub id: String,
    pub name: String,
    pub graph: Graph,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadenceGraphTarget {
    #[default]
    Runtime,
    Trick { trick_id: String },
}

#[derive(Debug, Clone)]
pub struct TargetedGraphOpRecord {
    pub target: CadenceGraphTarget,
    pub record: GraphOpRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum InitStageOp {
    SetCps {
        expr: Option<String>,
    },
    SampleLoadUpsert {
        id: String,
        source: String,
        #[serde(default)]
        aliases: BTreeMap<String, String>,
    },
    SampleLoadRemove {
        id: String,
    },
    TrickCreate {
        id: String,
        name: String,
        #[serde(default)]
        graph: Option<Graph>,
    },
    TrickRename {
        id: String,
        name: String,
    },
    TrickDelete {
        id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitStageSnapshotDto {
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoad>,
    pub tricks: Vec<InitStageTrickDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitStageTrickDto {
    pub id: String,
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

impl From<&CadenceInitStage> for InitStageSnapshotDto {
    fn from(value: &CadenceInitStage) -> Self {
        Self {
            cps_expr: value.cps_expr.clone(),
            sample_loads: value.sample_loads.clone(),
            tricks: value
                .tricks
                .iter()
                .map(|trick| InitStageTrickDto {
                    id: trick.id.clone(),
                    name: trick.name.clone(),
                    node_count: trick.graph.nodes.len(),
                    edge_count: trick.graph.edges.len(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitStageApplyArgs {
    pub ops: Vec<InitStageOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCompilePreviewDto {
    pub can_render: bool,
    pub can_play: bool,
    pub code: Option<String>,
    pub diagnostics: Vec<tile_graph::diagnostics::Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadenceTrickSignature {
    pub inputs: Vec<CadenceTrickInput>,
    pub output_pos: tile_graph::types::GridPos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadenceTrickInput {
    pub slot: u8,
    pub pos: tile_graph::types::GridPos,
    pub label: String,
    pub port_type: tile_graph::types::PortType,
    pub required: bool,
    pub is_receiver: bool,
    pub default_value: Option<Value>,
}

impl CadenceTrickInput {
    /// Returns the user-defined label sanitised into a valid JS identifier,
    /// falling back to `arg{slot}` when the label is empty or not usable.
    pub fn param_name(&self) -> String {
        let sanitized = sanitize_param_label(&self.label);
        if sanitized.is_empty() {
            format!("arg{}", self.slot)
        } else {
            sanitized
        }
    }
}

fn sanitize_param_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (index, ch) in trimmed.chars().enumerate() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            if index == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else if (ch == ' ' || ch == '-' || ch == '.') && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}
