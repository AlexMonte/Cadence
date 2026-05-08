use std::{
    collections::BTreeSet,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required for build.rs"),
    );
    let source_samples = manifest_dir.join("resources").join("samples");
    let bundled_samples = manifest_dir.join("assets").join("generated_samples");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is required"));
    let catalog_rs = out_dir.join("web_sample_catalog.rs");

    println!("cargo:rerun-if-changed={}", source_samples.display());

    let mut assets = if source_samples.is_dir() {
        collect_sample_assets(source_samples.as_path()).unwrap_or_else(|error| panic!("{error}"))
    } else {
        Vec::new()
    };
    assets.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    sync_sample_bundle(&assets, bundled_samples.as_path())
        .unwrap_or_else(|error| panic!("{error}"));

    fs::write(&catalog_rs, render_web_sample_catalog(&assets))
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", catalog_rs.display()));
}

#[derive(Debug, Clone)]
struct SampleAsset {
    source: PathBuf,
    relative_path: PathBuf,
    key: String,
}

fn sync_sample_bundle(assets: &[SampleAsset], target_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|error| format!("failed to create {}: {error}", target_dir.display()))?;

    let mut expected_files = BTreeSet::new();
    for asset in assets {
        println!("cargo:rerun-if-changed={}", asset.source.display());
        let target = target_dir.join(&asset.relative_path);
        copy_file_if_needed(asset.source.as_path(), target.as_path())?;
        expected_files.insert(target);
    }

    prune_extra_files(target_dir, &expected_files)?;
    Ok(())
}

fn collect_sample_assets(source_dir: &Path) -> Result<Vec<SampleAsset>, String> {
    let mut files = Vec::new();
    collect_audio_files(source_dir, &mut files)
        .map_err(|error| format!("failed to read {}: {error}", source_dir.display()))?;

    Ok(files
        .into_iter()
        .map(|path| {
            let relative_path = path
                .strip_prefix(source_dir)
                .expect("sample paths should stay inside the source directory")
                .to_path_buf();
            let mut key_path = relative_path.clone();
            key_path.set_extension("");
            SampleAsset {
                source: path,
                relative_path,
                key: key_path.to_string_lossy().replace('\\', "/"),
            }
        })
        .collect())
}

fn collect_audio_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_audio_files(path.as_path(), files)?;
            continue;
        }
        if is_audio_file(path.as_path()) {
            files.push(path);
        }
    }

    Ok(())
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "wav" | "mp3" | "ogg" | "flac"
            )
        })
        .unwrap_or(false)
}

fn copy_file_if_needed(source: &Path, target: &Path) -> Result<(), String> {
    let source_meta = source
        .metadata()
        .map_err(|error| format!("failed to stat {}: {error}", source.display()))?;

    let needs_copy = match target.metadata() {
        Ok(target_meta) => {
            let source_mtime = source_meta.modified().map_err(|error| {
                format!("failed to read {} modified time: {error}", source.display())
            })?;
            let target_mtime = target_meta.modified().map_err(|error| {
                format!("failed to read {} modified time: {error}", target.display())
            })?;
            source_mtime > target_mtime || source_meta.len() != target_meta.len()
        }
        Err(_) => true,
    };

    if !needs_copy {
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    fs::copy(source, target).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;

    Ok(())
}

fn prune_extra_files(directory: &Path, expected_files: &BTreeSet<PathBuf>) -> Result<(), String> {
    if !directory.is_dir() {
        return Ok(());
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            prune_extra_files(path.as_path(), expected_files)?;
            let is_empty = fs::read_dir(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?
                .next()
                .is_none();
            if is_empty {
                fs::remove_dir(&path)
                    .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
            }
            continue;
        }
        if expected_files.contains(&path) {
            continue;
        }
        fs::remove_file(&path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
    }

    Ok(())
}

fn render_web_sample_catalog(assets: &[SampleAsset]) -> String {
    let mut module =
        String::from("pub(crate) static WEB_SAMPLE_ENTRIES: &[WebBundledSampleEntry] = &[\n");
    for asset in assets {
        let _ = writeln!(
            module,
            "    WebBundledSampleEntry {{ key: {:?}, web_path: {:?} }},",
            asset.key,
            sample_web_path(asset.relative_path.as_path()),
        );
    }
    module.push_str("];\n");
    module
}

fn sample_web_path(relative_path: &Path) -> String {
    relative_path
        .components()
        .map(|component| percent_encode(component.as_os_str().to_string_lossy().as_ref()))
        .collect::<Vec<_>>()
        .join("/")
}

fn percent_encode(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~' | b'(' | b')')
        {
            encoded.push(*byte as char);
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
