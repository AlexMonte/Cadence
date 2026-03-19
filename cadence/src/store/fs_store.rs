use std::fs;
use std::path::Path;

use crate::errors::{AppError, AppResult};

pub fn load_project(path: &Path) -> AppResult<String> {
    if !path.is_file() {
        return Err(AppError::InvalidInput(format!(
            "project file not found: {}",
            path.display()
        )));
    }

    Ok(fs::read_to_string(path)?)
}

pub fn save_project(path: &Path, payload: &str) -> AppResult<()> {
    if payload.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "cannot save empty payload".to_string(),
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, payload)?;
    Ok(())
}
