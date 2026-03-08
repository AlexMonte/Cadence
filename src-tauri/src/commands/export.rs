use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::commands::project_commands::{SharedAppState, active_project};
use crate::commands::runtime_commands::TerminalStrategy;
use crate::core::project_compile::compile_project;
use crate::errors::{AppError, AppResult};
use tile_graph::compiler::CompileMode;
use tile_graph::diagnostics::Diagnostic;

#[derive(Debug, Clone, Deserialize)]
pub struct ExportSongArgs {
    pub path: String,
    pub cpm: Option<f32>,
    pub code_override: Option<String>,
    pub terminal_strategy: Option<TerminalStrategy>,
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
        let compiled = compile_project(project, strategy, CompileMode::Preview);
        if !compiled.can_render {
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
