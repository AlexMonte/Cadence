use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn frontend_dir_from_manifest() -> PathBuf {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required for build.rs"),
    );
    manifest_dir
        .parent()
        .expect("cadence should be nested under workspace root")
        .join("cadence_frontend")
}

fn frontend_bundle_dir(frontend_dir: &Path) -> PathBuf {
    frontend_dir.join("dist").join("public")
}

fn uses_tauri_dev_server() -> bool {
    let Ok(mut config) = std::env::var("TAURI_CONFIG") else {
        return false;
    };
    config.retain(|ch| !ch.is_whitespace());
    let compact = config;
    if !compact.contains("\"devUrl\"") {
        return false;
    }
    !(compact.contains("\"devUrl\":null") || compact.contains("\"devUrl\":\"\""))
}

fn is_test_build() -> bool {
    matches!(
        std::env::var("PROFILE").ok().as_deref(),
        Some("test") | Some("bench")
    ) || std::env::var("CARGO_CFG_TEST").is_ok()
}

fn is_release_build() -> bool {
    matches!(std::env::var("PROFILE").ok().as_deref(), Some("release"))
}

fn allow_stale_dist_override() -> bool {
    std::env::var("CADENCE_ALLOW_STALE_DIST")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn generated_frontend_asset(path: &Path, frontend_dir: &Path) -> bool {
    path == frontend_dir.join("assets").join("tailwind.css")
}

fn latest_modified_time(path: &Path) -> Option<SystemTime> {
    let metadata = path.metadata().ok()?;
    let mut latest = metadata.modified().ok()?;

    if !metadata.is_dir() {
        return Some(latest);
    }

    let entries = fs::read_dir(path).ok()?;

    for entry in entries.flatten() {
        let child = entry.path();
        let hidden = child
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.starts_with('.'));
        if hidden {
            continue;
        }
        if let Some(child_latest) = latest_modified_time(&child) {
            latest = latest.max(child_latest);
        }
    }

    Some(latest)
}

fn stale_dist_input(frontend_dir: &Path, bundle_dir: &Path) -> Option<PathBuf> {
    let dist_mtime = latest_modified_time(bundle_dir)?;

    let inputs = [
        frontend_dir.join("Cargo.toml"),
        frontend_dir.join("Dioxus.toml"),
        frontend_dir.join("index.html"),
        frontend_dir.join("devtools.html"),
        frontend_dir.join("devtools.js"),
        frontend_dir.join("tailwind.css"),
        frontend_dir.join("package.json"),
        frontend_dir.join("package-lock.json"),
        frontend_dir.join("src"),
        frontend_dir.join("assets"),
    ];

    for input in inputs {
        if let Some(stale) = first_newer_path(&input, dist_mtime, frontend_dir) {
            return Some(stale);
        }
    }
    None
}

fn first_newer_path(path: &Path, dist_mtime: SystemTime, frontend_dir: &Path) -> Option<PathBuf> {
    if generated_frontend_asset(path, frontend_dir) {
        return None;
    }

    let Ok(metadata) = path.metadata() else {
        return None;
    };

    if metadata.is_file() {
        if metadata
            .modified()
            .ok()
            .is_some_and(|mtime| mtime > dist_mtime)
        {
            return Some(path.to_path_buf());
        }
        return None;
    }

    if !metadata.is_dir() {
        return None;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return None;
    };

    for entry in entries.flatten() {
        let child = entry.path();
        let hidden = child
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.starts_with('.'));
        if hidden {
            continue;
        }
        if let Some(stale) = first_newer_path(&child, dist_mtime, frontend_dir) {
            return Some(stale);
        }
    }

    None
}

fn enforce_dist_freshness(frontend_dir: &Path, bundle_dir: &Path, dist_index: &Path) {
    let Some(input) = stale_dist_input(frontend_dir, bundle_dir) else {
        return;
    };
    if !is_release_build()
        || uses_tauri_dev_server()
        || is_test_build()
        || allow_stale_dist_override()
    {
        println!(
            "cargo:warning=UI dist is stale ({} newer than {}), but build continues in dev/test mode",
            input.display(),
            dist_index.display()
        );
        return;
    }
    panic!(
        "UI dist is stale ({} newer than {}). \
Run `./dev build` from the workspace root to refresh the frontend bundle, or use `cargo tauri dev --config cadence/tauri.dev.conf.json` for live source edits.",
        input.display(),
        dist_index.display()
    );
}

fn copy_file_if_needed(source: &Path, target: &Path) -> Result<(), String> {
    let source_meta = source
        .metadata()
        .map_err(|err| format!("failed to stat {}: {err}", source.display()))?;

    let needs_copy = match target.metadata() {
        Ok(target_meta) => {
            let source_mtime = source_meta.modified().map_err(|err| {
                format!("failed to read {} modified time: {err}", source.display())
            })?;
            let target_mtime = target_meta.modified().map_err(|err| {
                format!("failed to read {} modified time: {err}", target.display())
            })?;
            source_mtime > target_mtime
        }
        Err(_) => true,
    };

    if !needs_copy {
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    fs::copy(source, target).map_err(|err| {
        format!(
            "failed to copy {} to {}: {err}",
            source.display(),
            target.display()
        )
    })?;

    Ok(())
}

fn sync_frontend_support_files(frontend_dir: &Path, bundle_dir: &Path) -> Result<(), String> {
    copy_file_if_needed(
        &frontend_dir.join("devtools.html"),
        &bundle_dir.join("devtools.html"),
    )?;
    copy_file_if_needed(
        &frontend_dir.join("devtools.js"),
        &bundle_dir.join("devtools.js"),
    )?;
    Ok(())
}

fn register_frontend_reruns(frontend_dir: &Path) {
    for path in [
        frontend_dir.join("Cargo.toml"),
        frontend_dir.join("Dioxus.toml"),
        frontend_dir.join("index.html"),
        frontend_dir.join("devtools.html"),
        frontend_dir.join("devtools.js"),
        frontend_dir.join("tailwind.css"),
        frontend_dir.join("package.json"),
        frontend_dir.join("package-lock.json"),
        frontend_dir.join("src"),
        frontend_dir.join("assets"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn normalize_bundle_paths(bundle_dir: &Path) -> Result<(), String> {
    let targets = [
        bundle_dir.join("index.html"),
        bundle_dir.join("wasm").join("cadence_frontend.js"),
    ];

    for target in targets {
        if !target.exists() {
            continue;
        }

        let original = fs::read_to_string(&target)
            .map_err(|err| format!("failed to read {}: {err}", target.display()))?;
        let normalized = original.replace("/./wasm/", "/wasm/");

        if normalized != original {
            fs::write(&target, normalized)
                .map_err(|err| format!("failed to write {}: {err}", target.display()))?;
        }
    }

    Ok(())
}

fn main() {
    let frontend_dir = frontend_dir_from_manifest();
    let bundle_dir = frontend_bundle_dir(&frontend_dir);
    let dist_index = bundle_dir.join("index.html");

    register_frontend_reruns(&frontend_dir);

    if !dist_index.exists() {
        if !is_release_build()
            || uses_tauri_dev_server()
            || is_test_build()
            || allow_stale_dist_override()
        {
            println!(
                "cargo:warning=UI dist missing at {}, but build continues in dev/test mode",
                dist_index.display()
            );
            tauri_build::build();
            return;
        }

        println!("cargo:warning=UI dist missing at {}", dist_index.display(),);
        panic!(
            "frontend dist missing. Run `./dev build` from the workspace root before building cadence."
        );
    }

    enforce_dist_freshness(&frontend_dir, &bundle_dir, &dist_index);
    normalize_bundle_paths(&bundle_dir).unwrap_or_else(|err| panic!("{err}"));
    sync_frontend_support_files(&frontend_dir, &bundle_dir).unwrap_or_else(|err| panic!("{err}"));
    tauri_build::build();
}
