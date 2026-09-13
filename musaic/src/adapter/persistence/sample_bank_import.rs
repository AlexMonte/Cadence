//! Bounded, atomic import of a portable tuned-instrument/sample-bank package.
use std::{
    io::Read,
    path::{Component, Path},
};

use serde::Deserialize;

use crate::{
    application::session::MusaicProject,
    domain::project::samples::{
        SampleBankDefinition, SampleId, SampleImportOptions, SamplePitchZone, SampleVariant,
    },
};

use super::{
    PersistenceError, PreparedSample, adopt_prepared_sample, prepare_wav_bytes, prepare_wav_sample,
    set_sample_bank,
};

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_BANK_BYTES: u64 = 256 * 1024 * 1024;

/// A complete decoded bank; no project is changed until adoption succeeds.
#[derive(Debug, Clone)]
pub struct PreparedSampleBank {
    members: Vec<(PreparedSample, SampleVariant)>,
}

/// Result accepted by the existing background sample loader.
#[derive(Debug, Clone)]
pub enum PreparedSampleImport {
    Recording(PreparedSample),
    Bank(PreparedSampleBank),
}

impl From<PreparedSample> for PreparedSampleImport {
    fn from(sample: PreparedSample) -> Self {
        Self::Recording(sample)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BankFile {
    variants: Vec<BankMember>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BankMember {
    /// WAV path relative to the package manifest, confined to its directory.
    path: String,
    /// Optional assertion protecting loop frame coordinates across decoding.
    #[serde(default)]
    sample_rate: Option<u32>,
    #[serde(default)]
    options: SampleImportOptions,
    #[serde(default)]
    bank: Option<String>,
    #[serde(default)]
    pitch_zone: Option<SamplePitchZone>,
    #[serde(default)]
    playback_limit_ms: Option<u32>,
    #[serde(default)]
    hat_choke: bool,
}

fn invalid(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Sample(message.into())
}

/// Accepts ordinary WAV files or a `.musaic-bank.json` instrument package.
pub fn prepare_sample_import(
    path: impl AsRef<Path>,
) -> Result<PreparedSampleImport, PersistenceError> {
    let path = path.as_ref();
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        prepare_sample_bank(path).map(PreparedSampleImport::Bank)
    } else {
        prepare_wav_sample(path, Default::default()).map(Into::into)
    }
}

/// Decode every member and validate all hints before returning to the editor.
pub fn prepare_sample_bank(path: impl AsRef<Path>) -> Result<PreparedSampleBank, PersistenceError> {
    let path = path.as_ref();
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_MANIFEST_BYTES {
        return Err(invalid(
            "An instrument bank manifest must be a file of at most 1 MiB",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(invalid("Instrument bank manifest exceeds 1 MiB"));
    }
    let bank: BankFile = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("Invalid instrument bank manifest: {error}")))?;
    if bank.variants.is_empty() || bank.variants.len() > 256 {
        return Err(invalid("An instrument bank needs 1 to 256 WAV variants"));
    }
    let directory = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    let mut members = Vec::with_capacity(bank.variants.len());
    let mut total_bytes = 0_u64;
    let mut decoded_bytes = 0_u64;
    for member in bank.variants {
        let relative = Path::new(&member.path);
        if member.path.is_empty()
            || member.path.contains('\\')
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(invalid(
                "Bank WAV paths must stay inside the instrument package directory",
            ));
        }
        let source = directory.join(relative).canonicalize()?;
        if !source.starts_with(&directory) {
            return Err(invalid(
                "Bank WAV symlinks must stay inside the instrument package directory",
            ));
        }
        total_bytes = total_bytes.saturating_add(source.metadata()?.len());
        if total_bytes > MAX_BANK_BYTES {
            return Err(invalid(
                "An instrument bank is limited to 256 MiB of WAV audio",
            ));
        }
        let wav = super::wav::transcode_bank_audio(super::samples::read_wav_file(&source)?)?;
        let prepared = prepare_wav_bytes(
            source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| invalid("Invalid bank recording name"))?,
            wav,
            member.options,
        )?;
        if member
            .sample_rate
            .is_some_and(|rate| rate != prepared.sample_rate())
        {
            return Err(invalid(
                "Instrument sample rate differs from its loop metadata",
            ));
        }
        decoded_bytes = decoded_bytes.saturating_add(prepared.byte_length());
        if decoded_bytes > MAX_BANK_BYTES {
            return Err(invalid("Decoded instrument bank exceeds 256 MiB"));
        }
        members.push((
            prepared,
            SampleVariant {
                sample: SampleId(0),
                bank: member.bank,
                pitch_zone: member.pitch_zone,
                playback_limit_ms: member.playback_limit_ms,
                hat_choke: member.hat_choke,
            },
        ));
    }
    let prepared = PreparedSampleBank { members };
    // Reuse the single authoritative project validator for ranges, labels,
    // ownership, decoded metadata and all aggregate limits.
    adopt_prepared_sample_bank(&mut MusaicProject::new_empty(), prepared.clone())?;
    Ok(prepared)
}

/// Commit a complete bank as one sample-state replacement. Failure (including a
/// full destination project) leaves IDs, existing assets and dirty state intact.
pub fn adopt_prepared_sample_bank(
    project: &mut MusaicProject,
    prepared: PreparedSampleBank,
) -> Result<SampleId, PersistenceError> {
    let mut staging = MusaicProject::new_empty();
    staging.samples = project.samples.clone();
    let mut variants = Vec::with_capacity(prepared.members.len());
    for (sample, mut variant) in prepared.members {
        variant.sample = adopt_prepared_sample(&mut staging, sample)?;
        variants.push(variant);
    }
    let lead = variants
        .first()
        .ok_or_else(|| invalid("An instrument bank cannot be empty"))?
        .sample;
    set_sample_bank(&mut staging, lead, Some(SampleBankDefinition { variants }))?;
    project.samples = staging.samples;
    project.mark_dirty();
    Ok(lead)
}

pub fn adopt_sample_import(
    project: &mut MusaicProject,
    prepared: PreparedSampleImport,
) -> Result<SampleId, PersistenceError> {
    match prepared {
        PreparedSampleImport::Recording(sample) => adopt_prepared_sample(project, sample),
        PreparedSampleImport::Bank(bank) => adopt_prepared_sample_bank(project, bank),
    }
}
