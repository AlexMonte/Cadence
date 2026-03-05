use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::commands::project_commands::{SharedAppState, active_project};
use crate::core::compiler::compile_graph;
use crate::core::diagnostics::Diagnostic;
use crate::core::piece_registry::PieceRegistry;
use crate::core::semantic::semantic_pass;
use crate::errors::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
pub struct ExportSongArgs {
    pub path: String,
    pub cpm: Option<f32>,
    pub code_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSongResultDto {
    pub exported: bool,
    pub message: String,
    pub path: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
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
pub fn export_song(
    state: tauri::State<'_, SharedAppState>,
    args: ExportSongArgs,
) -> Result<ExportSongResultDto, String> {
    let _ = args.cpm.unwrap_or(120.0);
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
        let registry = PieceRegistry::default_strudel();
        let sem = semantic_pass(&project.graph, &registry);
        if !sem.is_valid() {
            return Ok(ExportSongResultDto {
                exported: false,
                message: format!("compile blocked by {} diagnostics", sem.errors.len()),
                path: None,
                diagnostics: sem.errors,
            });
        }
        match compile_graph(&project.graph, &registry, &sem) {
            Ok(expr) => expr.render(),
            Err(errors) => {
                return Ok(ExportSongResultDto {
                    exported: false,
                    message: format!("compile blocked by {} diagnostics", errors.len()),
                    path: None,
                    diagnostics: errors,
                });
            }
        }
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
