//! Reusable authored tile programs. Instances refer to definitions without copying code.
use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrickDefinition {
    pub name: String,
    pub source: NodeId,
    /// An upstream pattern node replaced by the instance input when called.
    pub input: Option<NodeId>,
}
