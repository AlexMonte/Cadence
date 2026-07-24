use std::fs;
use std::path::{Path, PathBuf};

const RECENT_FILE: &str = "recent_projects.json";

pub fn load_recent_projects() -> Vec<PathBuf> {
    recent_file_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn push_recent_project(path: impl AsRef<Path>) {
    let path = path.as_ref().to_path_buf();
    let mut recent = load_recent_projects();
    recent.retain(|p| p != &path);
    recent.insert(0, path);
    recent.truncate(12);
    if let Some(file) = recent_file_path() {
        if let Ok(json) = serde_json::to_string_pretty(&recent) {
            let _ = fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
            let _ = fs::write(file, json);
        }
    }
}

fn recent_file_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "musaic").map(|dirs| dirs.data_dir().join(RECENT_FILE))
}
