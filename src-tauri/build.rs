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

fn warn_if_dist_stale(ui_dir: &Path, dist_index: &Path) {
    let dist_mtime = match dist_index.metadata().and_then(|meta| meta.modified()) {
        Ok(value) => value,
        Err(_) => return,
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
            println!(
                "cargo:warning=UI dist may be stale ({} newer than {}). \
Run: npm --prefix {} run build",
                input.display(),
                dist_index.display(),
                ui_dir.display()
            );
            return;
        }
    }
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

    warn_if_dist_stale(&ui_dir, &dist_index);
    tauri_build::build();
}
