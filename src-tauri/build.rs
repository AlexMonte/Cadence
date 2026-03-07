use std::path::{Path, PathBuf};
use std::process::Command;

fn ui_dir_from_manifest() -> PathBuf {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required for build.rs"),
    );
    manifest_dir
        .parent()
        .expect("src-tauri should be nested under workspace root")
        .join("ui")
}

fn npm_binary() -> &'static str {
    if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn run_ui_build(ui_dir: &Path) -> Result<(), String> {
    let status = Command::new(npm_binary())
        .arg("--prefix")
        .arg(ui_dir)
        .args(["run", "build"])
        .status()
        .map_err(|err| {
            format!(
                "failed to run npm build preflight ({err}). \
Install Node.js + npm, then run: npm --prefix {} run build",
                ui_dir.display()
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "frontend build failed with status {status}. \
Run: npm --prefix {} run build",
            ui_dir.display()
        ))
    }
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

fn allow_stale_dist_override() -> bool {
    std::env::var("CADENCE_ALLOW_STALE_DIST")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn stale_dist_input(ui_dir: &Path, dist_index: &Path) -> Option<PathBuf> {
    let dist_mtime = match dist_index.metadata().and_then(|meta| meta.modified()) {
        Ok(value) => value,
        Err(_) => return None,
    };

    let inputs = [
        ui_dir.join("index.html"),
        ui_dir.join("app.js"),
        ui_dir.join("app.css"),
    ];

    for input in inputs {
        let Ok(input_mtime) = input.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if input_mtime > dist_mtime {
            return Some(input);
        }
    }
    None
}

fn enforce_dist_freshness(ui_dir: &Path, dist_index: &Path) {
    let Some(input) = stale_dist_input(ui_dir, dist_index) else {
        return;
    };
    if uses_tauri_dev_server() || is_test_build() || allow_stale_dist_override() {
        println!(
            "cargo:warning=UI dist is stale ({} newer than {}), but build continues in dev/test mode",
            input.display(),
            dist_index.display()
        );
        return;
    }
    panic!(
        "UI dist is stale ({} newer than {}). \
Run `npm --prefix {} run build` or use `cargo tauri dev --config src-tauri/tauri.dev.conf.json`.",
        input.display(),
        dist_index.display(),
        ui_dir.display()
    );
}

fn main() {
    let ui_dir = ui_dir_from_manifest();
    let dist_index = ui_dir.join("dist").join("index.html");

    if !dist_index.exists() {
        println!(
            "cargo:warning=UI dist missing at {}. Running frontend build preflight...",
            dist_index.display()
        );
        if let Err(err) = run_ui_build(&ui_dir) {
            panic!("{err}");
        }
    }

    if !dist_index.exists() {
        panic!(
            "frontend dist still missing after preflight. expected {}",
            dist_index.display()
        );
    }

    enforce_dist_freshness(&ui_dir, &dist_index);
    tauri_build::build();
}
