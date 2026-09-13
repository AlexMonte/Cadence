//! Persist small interaction preferences; never write them into a song.
use crate::application::editor::preferences::EditorPreferences;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let value = load()
        .and_then(|value| value.validate().map(|()| value))
        .unwrap_or_else(|error| {
            warn!("Could not read editor preferences: {error}");
            EditorPreferences::default()
        });
    app.insert_resource(value.clone())
        .insert_resource(SavedPreferences(value))
        .add_systems(PostUpdate, save_changes);
}
#[derive(Resource)]
struct SavedPreferences(EditorPreferences);
fn save_changes(current: Res<EditorPreferences>, mut saved: ResMut<SavedPreferences>) {
    if !current.is_changed() || *current == saved.0 {
        return;
    }
    // Record this attempt even on failure so a read-only config directory does
    // not cause a write or warning on every rendered frame.
    saved.0 = current.clone();
    if let Err(error) = save(&current) {
        warn!("Could not save editor preferences: {error}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn path() -> Option<std::path::PathBuf> {
    std::env::var_os("MUSAIC_PREFERENCES_PATH")
        .map(Into::into)
        .or_else(|| {
            directories::ProjectDirs::from("", "", "musaic")
                .map(|dirs| dirs.config_dir().join("editor-preferences.json"))
        })
}
#[cfg(not(target_arch = "wasm32"))]
fn load() -> Result<EditorPreferences, String> {
    let Some(path) = path() else {
        return Ok(EditorPreferences::default());
    };
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(EditorPreferences::default())
        }
        Err(error) => Err(error.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn save(value: &EditorPreferences) -> Result<(), String> {
    let path = path().ok_or("No configuration directory")?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    // Use the existing same-directory, atomic replacement path.
    crate::adapter::persistence::write_preferences(&path, &bytes).map_err(|error| error.to_string())
}
#[cfg(target_arch = "wasm32")]
fn load() -> Result<EditorPreferences, String> {
    let storage = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or("Browser storage unavailable")?;
    let value = storage
        .get_item("musaic.editor-preferences")
        .map_err(|_| "Could not read browser preferences")?;
    value
        .map(|json| serde_json::from_str(&json).map_err(|error| error.to_string()))
        .unwrap_or(Ok(EditorPreferences::default()))
}
#[cfg(target_arch = "wasm32")]
fn save(value: &EditorPreferences) -> Result<(), String> {
    let storage = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or("Browser storage unavailable")?;
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    storage
        .set_item("musaic.editor-preferences", &json)
        .map_err(|_| "Could not save browser preferences".into())
}

/// Read a bounded file and validate the whole replacement before changing preferences.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn import_keymap(
    path: &std::path::Path,
) -> Result<crate::application::editor::preferences::keymap::Keymap, String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(32_769)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    crate::application::editor::preferences::keymap::Keymap::import(&text)
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn export_keymap(
    path: &std::path::Path,
    keymap: &crate::application::editor::preferences::keymap::Keymap,
) -> Result<(), String> {
    let json = keymap.export()?;
    crate::adapter::persistence::write_preferences(path, json.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod keymap_files_tests {
    use super::*;
    use crate::application::editor::preferences::keymap::{Action, Chord, Keymap};
    #[test]
    fn keymap_files_preserve_bindings_and_bound_import_size() {
        let folder = std::env::temp_dir().join(format!(
            "musaic-keymap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("shortcuts.json");
        let mut map = Keymap::default();
        map.replace(
            Action::PlayPause,
            vec![Chord::from_input(KeyCode::F8, &ButtonInput::default())],
        )
        .unwrap();
        export_keymap(&path, &map).unwrap();
        assert_eq!(import_keymap(&path).unwrap(), map);
        // Atomic replacement exports the current map, including disabled character shortcuts.
        map.character_shortcuts = false;
        export_keymap(&path, &map).unwrap();
        assert_eq!(import_keymap(&path).unwrap(), map);
        std::fs::write(&path, " ".repeat(32_769)).unwrap();
        assert!(import_keymap(&path).is_err());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
