use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::adapter::backend::CadenceSampleLoadDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProgramPayload {
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoadDto>,
    pub output_count: usize,
    pub debug_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeBootPhase {
    Idle,
    Booting,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBootStatus {
    pub phase: RuntimeBootPhase,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeSampleReadiness {
    pub ready: bool,
    pub attempted: usize,
    pub loaded: usize,
    pub failed: usize,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeSampleCacheStatus {
    pub name: String,
    pub installed: bool,
    pub available: bool,
    pub ready: bool,
    pub hits: usize,
    pub misses: usize,
    pub writes: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeInitSampleStatus {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    pub status: String,
    pub error: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
#[cfg(target_arch = "wasm32")]
pub use web::*;
