use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub display_name: String,
    pub file_path: Option<String>,
    pub dirty: bool,
}
