//! Tauri command for exporting the current project as a standalone Strudel script.

use std::path::{Path, PathBuf};

use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};

use crate::commands::project_commands::{SharedAppState, active_project};
use crate::commands::runtime_commands::TerminalStrategy;
use crate::core::project_compile::compile_project;
use crate::errors::{AppError, AppResult};
use tessera::compiler::CompileMode;
use tessera::diagnostics::Diagnostic;

#[derive(Debug, Clone, Deserialize)]
/// Payload for exporting either compiled project code or an explicit override.
pub struct ExportSongArgs {
    pub path: String,
    pub cpm: Option<f32>,
    pub code_override: Option<String>,
    pub terminal_strategy: Option<TerminalStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Result of attempting to export a Strudel script to disk.
pub struct ExportSongResultDto {
    pub exported: bool,
    pub message: String,
    pub path: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
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
    format!("{}.strudel.js", sanitize_export_stem(project_name))
}

fn resolve_export_path(path: &str) -> AppResult<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("path cannot be empty".to_string()));
    }
    let mut out = PathBuf::from(trimmed);
    if out.extension().is_none() {
        out.set_extension("strudel.js");
    }
    Ok(out)
}

fn write_export(path: &Path, mut payload: String) -> AppResult<()> {
    if !payload.ends_with('\n') {
        payload.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, payload)?;
    Ok(())
}

#[tauri::command]
/// Open a native save dialog for exporting the current project as a Strudel script.
pub async fn export_pick_song_path(
    state: tauri::State<'_, SharedAppState>,
) -> Result<Option<String>, String> {
    let (initial_directory, file_name) = {
        let store = state
            .store
            .lock()
            .map_err(|_| "app state lock poisoned".to_string())?;
        let directory = store
            .current_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(PathBuf::from);
        let file_name = store
            .current_project
            .as_ref()
            .map(|project| default_export_file_name(project.name.as_str()))
            .unwrap_or_else(|| "project.strudel.js".to_string());
        (directory, file_name)
    };

    let mut dialog = AsyncFileDialog::new().add_filter("Strudel Script", &["js"]);
    if let Some(directory) = initial_directory.as_ref() {
        dialog = dialog.set_directory(directory);
    }
    dialog = dialog.set_file_name(file_name.as_str());

    Ok(dialog
        .save_file()
        .await
        .map(|handle| handle.path().display().to_string()))
}

#[tauri::command]
/// Export the active project or a caller-supplied override to a `.strudel.js` file.
pub fn export_song(
    state: tauri::State<'_, SharedAppState>,
    args: ExportSongArgs,
) -> Result<ExportSongResultDto, String> {
    let cpm = args.cpm;
    let strategy = args.terminal_strategy.unwrap_or(TerminalStrategy::Stack);
    let path = resolve_export_path(args.path.as_str()).map_err(|err| err.to_string())?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;

    let payload = if let Some(override_code) = args.code_override.as_deref().map(str::trim) {
        if override_code.is_empty() {
            return Ok(ExportSongResultDto {
                exported: false,
                message: "code_override cannot be empty".to_string(),
                path: None,
                diagnostics: Vec::new(),
            });
        }
        override_code.to_string()
    } else {
        let project = active_project(&store).map_err(|err| err.to_string())?;
        let compiled = compile_project(project, strategy, CompileMode::Runtime);
        if !compiled.can_play {
            return Ok(ExportSongResultDto {
                exported: false,
                message: format!(
                    "compile blocked by {} diagnostics",
                    compiled.diagnostics.len()
                ),
                path: None,
                diagnostics: compiled.diagnostics,
            });
        }
        compiled.full_code.unwrap_or_default()
    };

    // Prepend a setCpm() call if the caller specified a tempo.
    let payload = match cpm {
        Some(value) => format!("setCpm({value});\n{payload}"),
        None => payload,
    };

    write_export(path.as_path(), payload).map_err(|err| err.to_string())?;
    let message = format!("exported song to {}", path.display());
    store.push_diagnostic("export_song", message.clone());
    Ok(ExportSongResultDto {
        exported: true,
        message,
        path: Some(path.display().to_string()),
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
            "Cadence Demo.strudel.js"
        );
    }

    #[test]
    fn export_file_name_sanitizes_invalid_path_characters() {
        assert_eq!(
            default_export_file_name("Lead/Bass:Take"),
            "Lead-Bass-Take.strudel.js"
        );
        assert_eq!(default_export_file_name("   "), "project.strudel.js");
    }
}
