//! Project-owned sample import and relative asset resolution.

use std::{io::Read, path::Path, sync::Arc};

use crate::{
    application::session::{LoadedProjectSample, MusaicProject, ProjectSamples},
    domain::project::samples::{
        SampleDiagnostic, SampleDiagnosticKind, SampleId, SampleImportOptions, SampleManifest,
        SampleMetadata,
    },
};

#[cfg(not(target_arch = "wasm32"))]
use super::atomic::atomic_write;
use super::{
    PersistenceError,
    wav::{MAX_WAV_BYTES, MAX_WAV_FRAMES, decode_wav},
};

const MAX_PROJECT_SAMPLE_BYTES: u64 = 256 * 1024 * 1024;

/// Fully decoded import, safe to send from a worker to the command dispatcher.
/// It does not contain or mutate an editor project.
#[derive(Debug, Clone)]
pub struct PreparedSample {
    source_name: String,
    sample: LoadedProjectSample,
    channels: u16,
    checksum: String,
    options: SampleImportOptions,
}

impl PreparedSample {
    pub(super) fn sample_rate(&self) -> u32 {
        self.sample.buffer.sample_rate()
    }
    pub(super) fn byte_length(&self) -> u64 {
        self.sample.wav_bytes.len() as u64
    }
}

fn sample_error(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Sample(message.into())
}

fn checksum(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn asset_path(id: SampleId, checksum: &str) -> String {
    format!("assets/samples/{:016x}-{checksum}.wav", id.0)
}

fn validate_options(options: &SampleImportOptions) -> Result<(), PersistenceError> {
    options.validate().map_err(sample_error)
}

pub(super) fn validate_manifest(manifest: &SampleManifest) -> Result<(), PersistenceError> {
    if manifest.next_id == 0 || manifest.samples.len() > 256 {
        return Err(sample_error(
            "Sample manifest has an invalid identity counter or exceeds 256 samples",
        ));
    }
    let mut total_bytes = 0_u64;
    for (id, sample) in &manifest.samples {
        if id.0 == 0 || id.0 >= manifest.next_id {
            return Err(sample_error(
                "Sample manifest contains an invalid or reused identity",
            ));
        }
        if sample.checksum.len() != 16
            || !sample
                .checksum
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(sample_error("Sample manifest contains an invalid checksum"));
        }
        if sample.relative_path != asset_path(*id, &sample.checksum) {
            return Err(sample_error(
                "Sample paths must name their project-owned assets/samples WAV file",
            ));
        }
        if sample.source_name.is_empty()
            || sample.source_name.len() > 1024
            || sample.source_name.contains(['/', '\\', '\0'])
            || !(1..=384_000).contains(&sample.sample_rate)
            || !(1..=2).contains(&sample.channels)
            || sample.frame_count == 0
            || sample.frame_count > MAX_WAV_FRAMES
            || sample.byte_length == 0
            || sample.byte_length > MAX_WAV_BYTES as u64
        {
            return Err(sample_error(
                "Sample manifest contains invalid WAV metadata",
            ));
        }
        validate_options(&sample.options)?;
        sample
            .options
            .validate_frame_count(sample.frame_count)
            .map_err(sample_error)?;
        total_bytes = total_bytes.saturating_add(sample.byte_length);
    }
    if total_bytes > MAX_PROJECT_SAMPLE_BYTES {
        return Err(sample_error(
            "Project-owned WAV assets are limited to 256 MiB total",
        ));
    }
    let mut members = std::collections::BTreeSet::new();
    for (lead, bank) in &manifest.banks {
        if bank.variants.is_empty() || bank.variants.len() > 256 || bank.variants[0].sample != *lead
        {
            return Err(sample_error(
                "A sample bank needs its lead recording first and at most 256 variants",
            ));
        }
        for variant in &bank.variants {
            if !manifest.samples.contains_key(&variant.sample) || !members.insert(variant.sample) {
                return Err(sample_error(
                    "Each bank variant must name an imported recording and belong to only one bank",
                ));
            }
            if variant.bank.as_ref().is_some_and(|name| {
                name.trim().is_empty()
                    || name.trim() != name
                    || name.len() > 64
                    || name.chars().any(char::is_control)
            }) {
                return Err(sample_error(
                    "Bank labels must contain 1 to 64 characters without surrounding spaces or control characters",
                ));
            }
            if variant.pitch_zone.as_ref().is_some_and(|zone| {
                !zone.low.is_finite()
                    || !zone.high.is_finite()
                    || zone.low < 0.0
                    || zone.high > 127.0
                    || zone.low > zone.high
            }) {
                return Err(sample_error(
                    "A sample pitch zone needs ordered MIDI pitches from 0 to 127",
                ));
            }
            if variant.playback_limit_ms == Some(0) {
                return Err(sample_error(
                    "A sample playback limit must be a positive number of milliseconds",
                ));
            }
        }
    }
    Ok(())
}

/// Bank membership is reference-only; imports keep their decoded ownership and
/// identities. Complete validation precedes the one manifest publication.
pub fn set_sample_bank(
    project: &mut MusaicProject,
    lead: SampleId,
    definition: Option<crate::domain::project::samples::SampleBankDefinition>,
) -> Result<Option<Option<crate::domain::project::samples::SampleBankDefinition>>, PersistenceError>
{
    if !project.samples.manifest.samples.contains_key(&lead) {
        return Err(sample_error("That lead sample is not in this project"));
    }
    let previous = project.samples.manifest.banks.get(&lead).cloned();
    if previous == definition {
        return Ok(None);
    }
    let mut manifest = project.samples.manifest.clone();
    match definition {
        Some(definition) => {
            manifest.banks.insert(lead, definition);
        }
        None => {
            manifest.banks.remove(&lead);
        }
    }
    validate_manifest(&manifest)?;
    project.samples.manifest = manifest;
    project.samples.revision = project.samples.revision.wrapping_add(1);
    project.mark_dirty();
    Ok(Some(previous))
}

/// Decode and validate the entire WAV before changing the active project.
/// Original bytes stay in the project cache until save writes its owned copy.
pub fn import_wav_bytes(
    project: &mut MusaicProject,
    source_name: &str,
    bytes: impl Into<Arc<[u8]>>,
    options: SampleImportOptions,
) -> Result<SampleId, PersistenceError> {
    adopt_prepared_sample(project, prepare_wav_bytes(source_name, bytes, options)?)
}

pub fn prepare_wav_bytes(
    source_name: &str,
    bytes: impl Into<Arc<[u8]>>,
    options: SampleImportOptions,
) -> Result<PreparedSample, PersistenceError> {
    validate_options(&options)?;
    if source_name.is_empty() || source_name.len() > 1024 || source_name.contains(['/', '\\', '\0'])
    {
        return Err(sample_error("Sample source must have a valid filename"));
    }
    let bytes = bytes.into();
    let decoded = decode_wav(bytes.clone())?;
    options
        .validate_frame_count(decoded.buffer.len() as u64)
        .map_err(sample_error)?;
    Ok(PreparedSample {
        source_name: source_name.to_owned(),
        channels: decoded.channels,
        checksum: checksum(&bytes),
        options,
        sample: LoadedProjectSample::new(bytes, decoded.buffer),
    })
}

pub fn adopt_prepared_sample(
    project: &mut MusaicProject,
    prepared: PreparedSample,
) -> Result<SampleId, PersistenceError> {
    let id = SampleId(project.samples.manifest.next_id);
    let next_id =
        id.0.checked_add(1)
            .ok_or_else(|| sample_error("Project sample identities are exhausted"))?;
    let metadata = SampleMetadata {
        source_name: prepared.source_name,
        relative_path: asset_path(id, &prepared.checksum),
        sample_rate: prepared.sample.buffer.sample_rate(),
        channels: prepared.channels,
        frame_count: prepared.sample.buffer.len() as u64,
        byte_length: prepared.sample.wav_bytes.len() as u64,
        checksum: prepared.checksum,
        options: prepared.options,
    };
    let mut manifest = project.samples.manifest.clone();
    manifest.next_id = next_id;
    manifest.samples.insert(id, metadata);
    validate_manifest(&manifest)?;
    project.samples.manifest = manifest;
    project.samples.loaded.insert(id, prepared.sample);
    project.samples.asset_changed(id);
    project.mark_dirty();
    Ok(id)
}

pub fn snapshot_sample(
    project: &MusaicProject,
    id: SampleId,
) -> Result<crate::application::session::SampleAssetSnapshot, PersistenceError> {
    Ok(crate::application::session::SampleAssetSnapshot {
        metadata: project
            .samples
            .manifest
            .samples
            .get(&id)
            .cloned()
            .ok_or_else(|| sample_error("That sample is not in this project"))?,
        loaded: project.samples.loaded.get(&id).cloned(),
        diagnostics: project
            .samples
            .diagnostics
            .iter()
            .filter(|item| item.sample == id)
            .cloned()
            .collect(),
    })
}

pub fn restore_sample_snapshot(
    project: &mut MusaicProject,
    id: SampleId,
    snapshot: &crate::application::session::SampleAssetSnapshot,
) -> Result<(), PersistenceError> {
    if !project.samples.manifest.samples.contains_key(&id) {
        return Err(sample_error("That sample is not in this project"));
    }
    let mut manifest = project.samples.manifest.clone();
    manifest.samples.insert(id, snapshot.metadata.clone());
    validate_manifest(&manifest)?;
    project.samples.manifest = manifest;
    match &snapshot.loaded {
        Some(loaded) => {
            project.samples.loaded.insert(id, loaded.clone());
        }
        None => {
            project.samples.loaded.remove(&id);
        }
    }
    project.samples.diagnostics.retain(|item| item.sample != id);
    project
        .samples
        .diagnostics
        .extend(snapshot.diagnostics.clone());
    project.samples.asset_changed(id);
    project.mark_dirty();
    Ok(())
}

/// Changes shared sample metadata without decoding or changing asset identity.
pub fn set_sample_options(
    project: &mut MusaicProject,
    id: SampleId,
    options: SampleImportOptions,
) -> Result<Option<SampleImportOptions>, PersistenceError> {
    validate_options(&options)?;
    let metadata = project
        .samples
        .manifest
        .samples
        .get_mut(&id)
        .ok_or_else(|| sample_error("That sample is not in this project"))?;
    options
        .validate_frame_count(metadata.frame_count)
        .map_err(sample_error)?;
    if metadata.options == options {
        return Ok(None);
    }
    let previous = std::mem::replace(&mut metadata.options, options);
    project.samples.revision = project.samples.revision.wrapping_add(1);
    project.mark_dirty();
    Ok(Some(previous))
}

/// Atomically adopts replacement audio under the existing SampleId. Metadata
/// options are read at adoption, preserving edits made while the file decoded.
pub fn relink_prepared_sample(
    project: &mut MusaicProject,
    id: SampleId,
    prepared: PreparedSample,
) -> Result<crate::application::session::SampleAssetSnapshot, PersistenceError> {
    let previous = snapshot_sample(project, id)?;
    let metadata = SampleMetadata {
        source_name: prepared.source_name,
        relative_path: asset_path(id, &prepared.checksum),
        sample_rate: prepared.sample.buffer.sample_rate(),
        channels: prepared.channels,
        frame_count: prepared.sample.buffer.len() as u64,
        byte_length: prepared.sample.wav_bytes.len() as u64,
        checksum: prepared.checksum,
        options: previous.metadata.options.clone(),
    };
    let mut manifest = project.samples.manifest.clone();
    manifest.samples.insert(id, metadata);
    validate_manifest(&manifest)?;
    project.samples.manifest = manifest;
    project.samples.loaded.insert(id, prepared.sample);
    project.samples.diagnostics.retain(|item| item.sample != id);
    project.samples.asset_changed(id);
    project.mark_dirty();
    Ok(previous)
}

pub(super) fn read_wav_file(path: &Path) -> Result<Arc<[u8]>, PersistenceError> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_WAV_BYTES as u64 {
        return Err(sample_error(
            "Sample must be a regular WAV file no larger than 64 MiB",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_WAV_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_WAV_BYTES {
        return Err(sample_error("WAV import is limited to 64 MiB per sample"));
    }
    Ok(bytes.into())
}

pub fn import_wav_sample(
    project: &mut MusaicProject,
    source_path: impl AsRef<Path>,
    options: SampleImportOptions,
) -> Result<SampleId, PersistenceError> {
    adopt_prepared_sample(project, prepare_wav_sample(source_path, options)?)
}

pub fn prepare_wav_sample(
    source_path: impl AsRef<Path>,
    options: SampleImportOptions,
) -> Result<PreparedSample, PersistenceError> {
    let source_path = source_path.as_ref();
    let source_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| sample_error("Sample source must have a valid filename"))?;
    let bytes = read_wav_file(source_path)?;
    prepare_wav_bytes(source_name, bytes, options)
}

pub(super) fn diagnose_missing_sample_bindings(project: &mut MusaicProject) {
    let mut missing = std::collections::BTreeSet::new();
    for node in project.document.graph.nodes() {
        let crate::domain::document::DocumentNodeKind::Sound(sound) = &node.kind else {
            continue;
        };
        if let crate::domain::instrument::InstrumentSource::Sample(id) = sound.definition.source
            && !project.samples.manifest.samples.contains_key(&id)
            && missing.insert(id)
        {
            project.samples.diagnostics.push(SampleDiagnostic {
                sample: id,
                source_name: id.runtime_name(),
                relative_path: String::new(),
                kind: SampleDiagnosticKind::Missing,
                message: format!(
                    "Instrument refers to {} which is absent from the project sample manifest",
                    id.runtime_name()
                ),
            });
        }
    }
}

fn diagnostic(
    id: SampleId,
    metadata: &SampleMetadata,
    kind: SampleDiagnosticKind,
    message: String,
) -> SampleDiagnostic {
    SampleDiagnostic {
        sample: id,
        source_name: metadata.source_name.clone(),
        relative_path: metadata.relative_path.clone(),
        kind,
        message,
    }
}

pub(super) fn from_manifest(manifest: SampleManifest) -> ProjectSamples {
    let diagnostics = manifest
        .samples
        .iter()
        .map(|(id, sample)| {
            diagnostic(
                *id,
                sample,
                SampleDiagnosticKind::Unresolved,
                "Sample asset needs its project folder; use load_project to resolve relative files"
                    .into(),
            )
        })
        .collect();
    ProjectSamples {
        manifest,
        diagnostics,
        ..Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn project_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_one(
    root: &Path,
    id: SampleId,
    metadata: &SampleMetadata,
) -> Result<LoadedProjectSample, PersistenceError> {
    let path = root.join(&metadata.relative_path);
    let resolved = path.canonicalize()?;
    if !resolved.starts_with(root) {
        return Err(sample_error(
            "Sample asset resolves outside the project folder",
        ));
    }
    let bytes = read_wav_file(&resolved)?;
    if bytes.len() as u64 != metadata.byte_length || checksum(&bytes) != metadata.checksum {
        return Err(sample_error(format!(
            "Sample {} differs from its saved WAV asset",
            id.runtime_name()
        )));
    }
    let decoded = decode_wav(bytes.clone())?;
    if decoded.buffer.sample_rate() != metadata.sample_rate
        || decoded.channels != metadata.channels
        || decoded.buffer.len() as u64 != metadata.frame_count
    {
        return Err(sample_error(
            "Decoded WAV does not match its sample metadata",
        ));
    }
    Ok(LoadedProjectSample::new(bytes, decoded.buffer))
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn resolve_project_samples(
    samples: &mut ProjectSamples,
    project_path: &Path,
) -> Result<(), PersistenceError> {
    let root = project_directory(project_path).canonicalize()?;
    samples.loaded.clear();
    samples.diagnostics.clear();
    for (id, metadata) in &samples.manifest.samples {
        match resolve_one(&root, *id, metadata) {
            Ok(sample) => {
                samples.loaded.insert(*id, sample);
            }
            Err(error) => {
                let kind = match &error {
                    PersistenceError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        SampleDiagnosticKind::Missing
                    }
                    _ => SampleDiagnosticKind::Invalid,
                };
                samples
                    .diagnostics
                    .push(diagnostic(*id, metadata, kind, error.to_string()));
            }
        }
    }
    samples.revision = samples.revision.wrapping_add(1);
    Ok(())
}

/// Immutable, content-checked WAV copies are written before the manifest commit.
/// A failed save cannot change assets referenced by the previously saved manifest.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn save_project_samples(
    project: &MusaicProject,
    project_path: &Path,
) -> Result<(), PersistenceError> {
    validate_manifest(&project.samples.manifest)?;
    for id in project.samples.manifest.samples.keys() {
        if !project.samples.loaded.contains_key(id) {
            return Err(sample_error(format!(
                "Cannot save: {} is missing or invalid; restore its WAV asset first",
                id.runtime_name()
            )));
        }
    }
    if project.samples.manifest.samples.is_empty() {
        return Ok(());
    }
    let root = project_directory(project_path).canonicalize()?;
    for directory in [root.join("assets"), root.join("assets/samples")] {
        match std::fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(sample_error(
                    "Project sample directories must be owned directories, not links",
                ));
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&directory)?
            }
            Err(error) => return Err(error.into()),
        }
    }
    for (id, metadata) in &project.samples.manifest.samples {
        let sample = &project.samples.loaded[id];
        let path = root.join(&metadata.relative_path);
        match std::fs::symlink_metadata(&path) {
            Ok(existing) => {
                if existing.file_type().is_symlink()
                    || !existing.is_file()
                    || read_wav_file(&path)?.as_ref() != sample.wav_bytes.as_ref()
                {
                    return Err(sample_error(format!(
                        "Refusing to replace a different sample asset at {}",
                        metadata.relative_path
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                atomic_write(&path, &sample.wav_bytes)?
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
