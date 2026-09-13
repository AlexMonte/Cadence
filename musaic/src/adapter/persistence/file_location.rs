//! Show a file's location without opening its contents or invoking a shell.
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
fn existing_file(path: &Path) -> Result<PathBuf, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("Cannot locate {}: {error}", path.display()))?;
    if !path.is_file() {
        return Err("The saved file is no longer available at this location.".into());
    }
    Ok(path)
}
pub(crate) fn reveal(path: &Path) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Err("Use your browser's downloads list to show the downloaded file.".into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = existing_file(path)?;
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut command = std::process::Command::new("/usr/bin/open");
            command.arg("-R").arg(&path);
            command
        };
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut command = std::process::Command::new("explorer.exe");
            command.arg(path.parent().ok_or("This file has no containing folder")?);
            command
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let mut command = {
            let mut command = std::process::Command::new("xdg-open");
            command.arg(path.parent().ok_or("This file has no containing folder")?);
            command
        };
        let result = command
            .output()
            .map_err(|error| format!("Could not show the file location: {error}"))?;
        if result.status.success() {
            Ok(())
        } else {
            Err("The file manager could not show this location.".into())
        }
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn location_requires_a_file_and_preserves_path_characters_without_shell_parsing() {
        let root = std::env::temp_dir().join(format!("musaic-reveal-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("space ' quote $(literal).wav");
        std::fs::write(&path, b"file").unwrap();
        assert_eq!(existing_file(&path).unwrap().file_name(), path.file_name());
        assert!(existing_file(&root).is_err());
        std::fs::remove_file(&path).unwrap();
        assert!(existing_file(&path).is_err());
        std::fs::remove_dir(root).unwrap();
    }
}
