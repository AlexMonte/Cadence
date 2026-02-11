//! Persistence for project model/layout state using deterministic JSON files.

use crate::core::{LayoutData, ModelData, Project, default_layout};
use std::fs;
use std::path::{Path, PathBuf};

/// Error type for store operations
#[derive(Debug)]
pub enum StoreError {
    IoError(std::io::Error),
    SerializationError(serde_json::Error),
    ProjectNotFound,
}

impl From<std::io::Error> for StoreError {
    fn from(err: std::io::Error) -> Self {
        StoreError::IoError(err)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(err: serde_json::Error) -> Self {
        StoreError::SerializationError(err)
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::IoError(e) => write!(f, "IO error: {}", e),
            StoreError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            StoreError::ProjectNotFound => write!(f, "Project not found"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Helper to convert ModelData to array-based JSON for cleaner diffs
/// Serializes as sorted arrays instead of maps for better git readability
fn to_array_model(model: &ModelData) -> serde_json::Value {
    // Sort scopes by ID
    let mut scopes_vec: Vec<_> = model.scopes.values().collect();
    scopes_vec.sort_by_key(|s| s.id);

    // Sort nodes by ID
    let mut nodes_vec: Vec<_> = model.nodes.values().collect();
    nodes_vec.sort_by_key(|n| n.id);

    // Sort edges by ID
    let mut edges_vec: Vec<_> = model.edges.values().collect();
    edges_vec.sort_by_key(|e| e.id);

    serde_json::json!({
        "root_scope": model.root_scope,
        "scopes": scopes_vec,
        "nodes": nodes_vec,
        "edges": edges_vec,
    })
}

/// Helper to convert LayoutData to array-based JSON for cleaner diffs
fn to_array_layout(layout: &LayoutData) -> serde_json::Value {
    // Sort layouts by scope ID
    let mut layouts_vec: Vec<_> = layout.layouts.values().collect();
    layouts_vec.sort_by_key(|l| l.scope_id);

    serde_json::json!({
        "layouts": layouts_vec,
    })
}

/// Helper to convert array-based JSON back to ModelData
/// Deserializes from array format and reconstructs HashMaps
fn from_array_model(value: &serde_json::Value) -> Result<ModelData, StoreError> {
    use crate::core::{Edge, ModelNode, Scope};
    use std::collections::HashMap;

    let root_scope = serde_json::from_value(value["root_scope"].clone())
        .map_err(StoreError::SerializationError)?;

    // Deserialize scopes array into HashMap
    let scopes_array: Vec<Scope> =
        serde_json::from_value(value["scopes"].clone()).map_err(StoreError::SerializationError)?;
    let mut scopes = HashMap::new();
    for scope in scopes_array {
        scopes.insert(scope.id, scope);
    }

    // Deserialize nodes array into HashMap
    let nodes_array: Vec<ModelNode> =
        serde_json::from_value(value["nodes"].clone()).map_err(StoreError::SerializationError)?;
    let mut nodes = HashMap::new();
    for node in nodes_array {
        nodes.insert(node.id, node);
    }

    // Deserialize edges array into HashMap
    let edges_array: Vec<Edge> =
        serde_json::from_value(value["edges"].clone()).map_err(StoreError::SerializationError)?;
    let mut edges = HashMap::new();
    for edge in edges_array {
        edges.insert(edge.id, edge);
    }

    Ok(ModelData {
        root_scope,
        scopes,
        nodes,
        edges,
    })
}

/// Helper to convert array-based JSON back to LayoutData
fn from_array_layout(value: &serde_json::Value) -> Result<LayoutData, StoreError> {
    use crate::core::Layout;
    use std::collections::HashMap;

    // Deserialize layouts array into HashMap
    let layouts_array: Vec<Layout> =
        serde_json::from_value(value["layouts"].clone()).map_err(StoreError::SerializationError)?;
    let mut layouts = HashMap::new();
    for layout in layouts_array {
        layouts.insert(layout.scope_id, layout);
    }

    Ok(LayoutData { layouts })
}

/// Project store for persistence
pub struct ProjectStore;

impl ProjectStore {
    /// Save a project to disk in a directory
    /// Creates model.json (semantic content) and layout.json (UI state)
    pub fn save_project<P: AsRef<Path>>(project: &Project, directory: P) -> Result<(), StoreError> {
        let dir = directory.as_ref();
        fs::create_dir_all(dir)?;

        // Save model.json (semantic content + metadata)
        let model_with_meta = serde_json::json!({
            "name": project.name,
            "model": to_array_model(&project.model),
        });
        let model_path = dir.join("model.json");
        let model_json = serde_json::to_string_pretty(&model_with_meta)?;
        fs::write(model_path, model_json)?;

        // Save layout.json (UI state)
        let layout_path = dir.join("layout.json");
        let layout_json = serde_json::to_string_pretty(&to_array_layout(&project.layout))?;
        fs::write(layout_path, layout_json)?;

        Ok(())
    }

    /// Load a project from disk
    pub fn load_project<P: AsRef<Path>>(directory: P) -> Result<Project, StoreError> {
        let dir = directory.as_ref();
        let model_path = dir.join("model.json");
        let layout_path = dir.join("layout.json");

        if !model_path.exists() {
            return Err(StoreError::ProjectNotFound);
        }

        // Load model.json
        let model_json = fs::read_to_string(model_path)?;
        let model_value: serde_json::Value = serde_json::from_str(&model_json)?;

        let name = model_value
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("Untitled")
            .to_string();

        let model = from_array_model(&model_value["model"])?;

        // Load layout.json if it exists, otherwise use default
        let layout = if layout_path.exists() {
            let layout_json = fs::read_to_string(layout_path)?;
            let layout_value: serde_json::Value = serde_json::from_str(&layout_json)?;
            from_array_layout(&layout_value)?
        } else {
            // Create default layout
            let mut layouts = std::collections::HashMap::new();
            layouts.insert(model.root_scope, default_layout(model.root_scope));
            LayoutData { layouts }
        };

        Ok(Project {
            name,
            model,
            layout,
        })
    }

    /// Check if a project exists at the given directory
    pub fn project_exists<P: AsRef<Path>>(directory: P) -> bool {
        let dir = directory.as_ref();
        let model_path = dir.join("model.json");
        model_path.exists()
    }

    /// List project directories directly under `directory`.
    pub fn list_projects<P: AsRef<Path>>(directory: P) -> Result<Vec<PathBuf>, StoreError> {
        let mut projects = Vec::new();

        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }

            let path = entry.path();
            if Self::project_exists(&path) {
                projects.push(path);
            }
        }

        projects.sort();
        Ok(projects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Project;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_split_files() {
        let temp_dir = TempDir::new().unwrap();
        let project = Project::new("Test Project".to_string());

        // Save
        ProjectStore::save_project(&project, temp_dir.path()).unwrap();

        // Verify files exist
        assert!(temp_dir.path().join("model.json").exists());
        assert!(temp_dir.path().join("layout.json").exists());

        // Load
        let loaded = ProjectStore::load_project(temp_dir.path()).unwrap();

        assert_eq!(project.name, loaded.name);
        assert_eq!(project.model.root_scope, loaded.model.root_scope);
    }

    #[test]
    fn test_load_nonexistent_project() {
        let temp_dir = TempDir::new().unwrap();
        let result = ProjectStore::load_project(temp_dir.path());
        assert!(matches!(result, Err(StoreError::ProjectNotFound)));
    }

    #[test]
    fn test_load_without_layout_file_uses_default_root_layout() {
        let temp_dir = TempDir::new().unwrap();
        let project = Project::new("No Layout".to_string());
        ProjectStore::save_project(&project, temp_dir.path()).unwrap();
        fs::remove_file(temp_dir.path().join("layout.json")).unwrap();

        let loaded = ProjectStore::load_project(temp_dir.path()).unwrap();
        let root_layout = loaded
            .layout
            .layouts
            .get(&loaded.model.root_scope)
            .expect("default root layout should be created when layout.json is missing");
        assert_eq!(root_layout.zoom, 1.0);
        assert_eq!(root_layout.camera_pos, (0.0, 0.0));
        assert!(root_layout.node_positions.is_empty());
    }

    #[test]
    fn test_list_projects_in_directory() {
        let temp_dir = TempDir::new().unwrap();
        let project_a_dir = temp_dir.path().join("alpha");
        let project_b_dir = temp_dir.path().join("beta");
        let non_project_dir = temp_dir.path().join("not_a_project");

        fs::create_dir_all(&non_project_dir).unwrap();
        ProjectStore::save_project(&Project::new("Alpha".to_string()), &project_a_dir).unwrap();
        ProjectStore::save_project(&Project::new("Beta".to_string()), &project_b_dir).unwrap();

        let projects = ProjectStore::list_projects(temp_dir.path()).unwrap();

        assert_eq!(projects.len(), 2);
        assert!(projects.contains(&project_a_dir));
        assert!(projects.contains(&project_b_dir));
        assert!(!projects.contains(&non_project_dir));
    }
}
