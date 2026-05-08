//! Backend helper for exporting the current project as a debug snapshot.

use serde::{Deserialize, Serialize};

use crate::{
    adapter::storage::{file_name, parent_location, pick_export_path, save_text_export},
    application::{authoring::compile::compile_project, project::ops::active_project},
    infrastructure::dto::{DiagnosticDto, SharedAppState},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Result of attempting to export a debug snapshot to disk.
pub struct ExportSongResultDto {
    pub exported: bool,
    pub message: String,
    pub path: Option<String>,
    pub diagnostics: Vec<DiagnosticDto>,
}

fn sanitize_export_stem(project_name: &str) -> String {
    let mut stem = project_name
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '\0' => '-',
            _ => ch,
        })
        .collect::<String>();
    if stem.is_empty() {
        stem = "project".to_string();
    }
    stem
}

fn default_export_file_name(project_name: &str) -> String {
    format!("{}.cadence.txt", sanitize_export_stem(project_name))
}

fn resolve_export_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path cannot be empty".to_string());
    }
    let mut out = trimmed.to_string();
    if file_name(out.as_str())
        .map(|name| !name.contains('.'))
        .unwrap_or(true)
    {
        out.push_str(".cadence.txt");
    }
    Ok(out)
}

/// Open a platform save dialog for exporting the current project as a debug snapshot.
pub async fn export_pick_song_path(state: &SharedAppState) -> Result<Option<String>, String> {
    let (initial_directory, file_name) = {
        let store = state
            .store
            .lock()
            .map_err(|_| "app state lock poisoned".to_string())?;
        let directory = store
            .current_path
            .as_ref()
            .and_then(|path| parent_location(path.as_str()));
        let file_name = store
            .current_project
            .as_ref()
            .map(|project| default_export_file_name(project.name.as_str()))
            .unwrap_or_else(|| "project.cadence.txt".to_string());
        (directory, file_name)
    };
    pick_export_path(initial_directory.as_deref(), file_name.as_str()).await
}

/// Export the active project preview to a text file.
pub fn export_song(
    state: &SharedAppState,
    path: String,
    cpm: Option<f32>,
) -> Result<ExportSongResultDto, String> {
    let path = resolve_export_path(path.as_str()).map_err(|err| err.to_string())?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;

    let project = active_project(&store).map_err(|err| err.to_string())?;
    let compiled = compile_project(project);
    if !compiled.can_render {
        return Ok(ExportSongResultDto {
            exported: false,
            message: format!(
                "compile blocked by {} diagnostics",
                compiled.diagnostics.len()
            ),
            path: None,
            diagnostics: crate::adapter::tessera::encode_diagnostics(compiled.diagnostics)
                .map_err(|err| err.to_string())?,
        });
    }
    let payload = compiled.preview.debug_text.unwrap_or_default();

    let payload = match cpm {
        Some(value) => format!("cpm = {value}\n{payload}"),
        None => payload,
    };

    let written_path = save_text_export(path.as_str(), payload.as_str())?;
    let message = format!("exported song to {written_path}");
    store.push_diagnostic("export_song", message.clone());
    Ok(ExportSongResultDto {
        exported: true,
        message,
        path: Some(written_path),
        diagnostics: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::default_export_file_name;

    #[test]
    fn export_file_name_uses_project_name_and_extension() {
        assert_eq!(
            default_export_file_name("Cadence Demo"),
            "Cadence Demo.cadence.txt"
        );
    }

    #[test]
    fn export_file_name_sanitizes_invalid_path_characters() {
        assert_eq!(
            default_export_file_name("Lead/Bass:Take"),
            "Lead-Bass-Take.cadence.txt"
        );
        assert_eq!(default_export_file_name("   "), "project.cadence.txt");
    }
}
