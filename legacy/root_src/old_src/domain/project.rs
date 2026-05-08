//! Cadence-owned project and init-stage concepts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tessera::graph::{Graph, GraphOpRecord};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphTarget {
    #[default]
    Runtime,
    Trick {
        trick_id: String,
    },
}

pub type CadenceGraphTarget = GraphTarget;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleLoad {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

pub type CadenceSampleLoad = SampleLoad;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceTrickWorkspace {
    pub id: String,
    pub name: String,
    pub graph: Graph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceProjectWorkspace {
    pub runtime_graph: Graph,
    #[serde(default)]
    pub tricks: Vec<CadenceTrickWorkspace>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CadenceInitStage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cps_expr: Option<String>,
    #[serde(default)]
    pub sample_loads: Vec<SampleLoad>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceProjectDocument {
    pub schema_version: u32,
    pub name: String,
    pub workspace: CadenceProjectWorkspace,
    #[serde(default)]
    pub init_stage: CadenceInitStage,
}

impl CadenceProjectDocument {
    pub const SCHEMA_VERSION: u32 = 4;

    pub fn new(name: String, graph: Graph) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            name,
            workspace: CadenceProjectWorkspace {
                runtime_graph: graph,
                tricks: Vec::new(),
            },
            init_stage: CadenceInitStage::default(),
        }
    }

    pub fn runtime_graph(&self) -> &Graph {
        &self.workspace.runtime_graph
    }

    pub fn runtime_graph_mut(&mut self) -> &mut Graph {
        &mut self.workspace.runtime_graph
    }

    pub fn tricks(&self) -> &[CadenceTrickWorkspace] {
        self.workspace.tricks.as_slice()
    }

    pub fn tricks_mut(&mut self) -> &mut Vec<CadenceTrickWorkspace> {
        &mut self.workspace.tricks
    }

    pub fn trick(&self, trick_id: &str) -> Option<&CadenceTrickWorkspace> {
        self.workspace
            .tricks
            .iter()
            .find(|trick| trick.id == trick_id)
    }

    pub fn trick_mut(&mut self, trick_id: &str) -> Option<&mut CadenceTrickWorkspace> {
        self.workspace
            .tricks
            .iter_mut()
            .find(|trick| trick.id == trick_id)
    }

    pub fn graph(&self, target: &GraphTarget) -> Option<&Graph> {
        match target {
            GraphTarget::Runtime => Some(self.runtime_graph()),
            GraphTarget::Trick { trick_id } => self.trick(trick_id).map(|trick| &trick.graph),
        }
    }

    pub fn graph_mut(&mut self, target: &GraphTarget) -> Option<&mut Graph> {
        match target {
            GraphTarget::Runtime => Some(self.runtime_graph_mut()),
            GraphTarget::Trick { trick_id } => {
                self.trick_mut(trick_id).map(|trick| &mut trick.graph)
            }
        }
    }
}

pub fn sample_selector_matches(selector: &str, sample: &SampleLoad) -> bool {
    sample_selector_namespace(sample).contains(selector)
}

pub fn sample_selector_namespace(sample: &SampleLoad) -> std::collections::BTreeSet<String> {
    let mut selectors = std::collections::BTreeSet::new();
    selectors.insert(sample.id.clone());
    selectors.insert(sample.source.clone());
    selectors.extend(sample.aliases.keys().cloned());
    selectors.extend(sample.aliases.values().cloned());
    selectors
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitStageTrickSummary {
    pub id: String,
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitStageSnapshot {
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<SampleLoad>,
    pub tricks: Vec<InitStageTrickSummary>,
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

#[derive(Debug, Clone)]
pub struct TargetedGraphOpRecord {
    pub target: GraphTarget,
    pub record: GraphOpRecord,
}

impl From<&CadenceProjectDocument> for InitStageSnapshot {
    fn from(value: &CadenceProjectDocument) -> Self {
        Self {
            cps_expr: value.init_stage.cps_expr.clone(),
            sample_loads: value.init_stage.sample_loads.clone(),
            tricks: value
                .tricks()
                .iter()
                .map(|trick| InitStageTrickSummary {
                    id: trick.id.clone(),
                    name: trick.name.clone(),
                    node_count: trick.graph.nodes.len(),
                    edge_count: trick.graph.edges.len(),
                })
                .collect(),
        }
    }
}
