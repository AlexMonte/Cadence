#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use cadence_core::infrastructure::playback::ChokeGroup;
use thiserror::Error;

#[derive(Debug, Clone)]
pub(crate) struct PathSampleCatalog {
    catalog: Arc<SampleCatalog>,
}

#[derive(Debug)]
struct SampleCatalog {
    samples: HashMap<String, PathBuf>,
    entries: Vec<SampleEntry>,
    entries_by_key: HashMap<String, SampleEntry>,
    query_cursors: Mutex<HashMap<QueryCursorKey, usize>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSample {
    pub path: PathBuf,
    pub playback_limit: Option<Duration>,
    pub choke_group: Option<ChokeGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SampleCatalogEntry {
    pub key: String,
    pub library: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum SampleSelector {
    Exact(String),
    Query(SampleQuery),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SampleQuery {
    pub library: Option<String>,
    pub tags: Vec<String>,
    pub ordinal: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum ParseSampleSelectorError {
    #[error("sample selector was empty")]
    Empty,
    #[error("sample selector did not contain any tags")]
    MissingTags,
    #[error("invalid sample selector segment: {0}")]
    InvalidSegment(String),
    #[error("invalid sample selector ordinal: {0}")]
    InvalidOrdinal(String),
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Error)]
pub(crate) enum SampleCatalogLoadError {
    #[error("sample directory not found: {0}")]
    MissingDirectory(String),
    #[error("failed to read sample directory: {0}")]
    ReadDirectory(#[from] std::io::Error),
    #[error("duplicate sample name in directory: {0}")]
    DuplicateSample(String),
    #[error("no sample files found in directory: {0}")]
    NoSamplesFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SampleFamily {
    Kick,
    Snare,
    Hat,
    Vocal,
    EightOhEight,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QueryCursorKey {
    library: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct SampleEntry {
    key: String,
    library: Option<String>,
    tags: Vec<String>,
}

impl Default for PathSampleCatalog {
    fn default() -> Self {
        Self::new(HashMap::new())
    }
}

impl PathSampleCatalog {
    pub(crate) fn new(samples: HashMap<String, PathBuf>) -> Self {
        let entries = build_entries(&samples);
        let entries_by_key = entries
            .iter()
            .cloned()
            .map(|entry| (entry.key.clone(), entry))
            .collect();

        Self {
            catalog: Arc::new(SampleCatalog {
                samples,
                entries,
                entries_by_key,
                query_cursors: Mutex::new(HashMap::new()),
            }),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn load_directory(path: impl AsRef<Path>) -> Result<Self, SampleCatalogLoadError> {
        let directory = path.as_ref();
        if !directory.exists() {
            return Err(SampleCatalogLoadError::MissingDirectory(
                directory.display().to_string(),
            ));
        }

        let mut sample_paths = Vec::new();
        collect_sample_paths(directory, &mut sample_paths)?;
        let mut samples = HashMap::new();
        for path in sample_paths {
            let key = key_for_sample_path(directory, &path);
            if samples.contains_key(&key) {
                return Err(SampleCatalogLoadError::DuplicateSample(key));
            }
            samples.insert(key, path);
        }

        if samples.is_empty() {
            return Err(SampleCatalogLoadError::NoSamplesFound(
                directory.display().to_string(),
            ));
        }

        Ok(Self::new(samples))
    }

    pub(crate) fn entries(&self) -> Vec<SampleCatalogEntry> {
        self.catalog
            .entries
            .iter()
            .cloned()
            .map(|entry| SampleCatalogEntry {
                key: entry.key,
                library: entry.library,
                tags: entry.tags,
            })
            .collect()
    }

    pub(crate) fn resolve_name(
        &self,
        name: &str,
    ) -> Result<ResolvedSample, ParseSampleSelectorError> {
        let selector = if self.catalog.samples.contains_key(name) {
            SampleSelector::Exact(name.to_string())
        } else {
            parse_query_selector(name)?
        };

        self.resolve(&selector)
            .ok_or_else(|| ParseSampleSelectorError::InvalidSegment(name.to_string()))
    }

    pub(crate) fn resolve(&self, selector: &SampleSelector) -> Option<ResolvedSample> {
        match selector {
            SampleSelector::Exact(key) => {
                let path = self.catalog.samples.get(key).cloned()?;
                let entry = self.catalog.entries_by_key.get(key);
                let tags = entry.map(|entry| entry.tags.as_slice()).unwrap_or(&[]);
                Some(ResolvedSample {
                    path,
                    playback_limit: playback_limit_for_tags(tags),
                    choke_group: choke_group_for_tags(tags),
                })
            }
            SampleSelector::Query(query) => self.resolve_query(query),
        }
    }

    fn resolve_query(&self, query: &SampleQuery) -> Option<ResolvedSample> {
        let candidates = self
            .catalog
            .entries
            .iter()
            .filter(|entry| query_matches_entry(query, entry))
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            return None;
        }

        let entry = if let Some(ordinal) = query.ordinal {
            candidates.get(ordinal - 1)?
        } else {
            let query_key = QueryCursorKey {
                library: query.library.clone(),
                tags: query.tags.clone(),
            };
            let mut cursors = match self.catalog.query_cursors.lock() {
                Ok(cursors) => cursors,
                Err(poisoned) => poisoned.into_inner(),
            };
            let cursor = cursors.entry(query_key).or_insert(0);
            let selected = candidates[*cursor % candidates.len()];
            *cursor = (*cursor + 1) % candidates.len();
            selected
        };

        let path = self.catalog.samples.get(&entry.key).cloned()?;
        Some(ResolvedSample {
            path,
            playback_limit: playback_limit_for_tags(&query.tags),
            choke_group: choke_group_for_tags(&query.tags),
        })
    }
}

impl SampleSelector {
    fn tags<T, I>(tags: I) -> Self
    where
        T: Into<String>,
        I: IntoIterator<Item = T>,
    {
        Self::Query(SampleQuery {
            library: None,
            tags: normalize_tags(tags),
            ordinal: None,
        })
    }

    fn ordinal(mut self, ordinal: usize) -> Self {
        if let Self::Query(query) = &mut self {
            query.ordinal = Some(ordinal);
        }
        self
    }
}

impl SampleFamily {
    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "kick" | "kicks" => Some(Self::Kick),
            "snare" | "snares" => Some(Self::Snare),
            "hat" | "hats" | "hihat" | "hihats" => Some(Self::Hat),
            "vocal" | "vocals" => Some(Self::Vocal),
            "808" => Some(Self::EightOhEight),
            _ => None,
        }
    }

    fn playback_limit(self) -> Option<Duration> {
        match self {
            Self::Kick => Some(Duration::from_millis(250)),
            Self::Snare => Some(Duration::from_millis(220)),
            Self::Hat => Some(Duration::from_millis(110)),
            Self::Vocal => None,
            Self::EightOhEight => Some(Duration::from_millis(500)),
        }
    }

    fn choke_group(self) -> Option<ChokeGroup> {
        match self {
            Self::Hat => Some(ChokeGroup::Hat),
            _ => None,
        }
    }

    fn canonical_tag(self) -> &'static str {
        match self {
            Self::Kick => "kick",
            Self::Snare => "snare",
            Self::Hat => "hat",
            Self::Vocal => "vocal",
            Self::EightOhEight => "808",
        }
    }
}

fn parse_query_selector(selector: &str) -> Result<SampleSelector, ParseSampleSelectorError> {
    let mut segments = selector
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_ascii_lowercase())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return Err(ParseSampleSelectorError::Empty);
    }

    let ordinal = if let Some(last) = segments.last() {
        if let Some(ordinal_segment) = last.strip_prefix('@') {
            let ordinal_segment = ordinal_segment.to_string();
            if ordinal_segment.is_empty()
                || !ordinal_segment
                    .chars()
                    .all(|character| character.is_ascii_digit())
            {
                return Err(ParseSampleSelectorError::InvalidOrdinal(last.clone()));
            }
            segments.pop();
            let ordinal = ordinal_segment
                .parse::<usize>()
                .map_err(|_| ParseSampleSelectorError::InvalidOrdinal(ordinal_segment.clone()))?;
            if ordinal == 0 {
                return Err(ParseSampleSelectorError::InvalidOrdinal(ordinal_segment));
            }
            Some(ordinal)
        } else {
            None
        }
    } else {
        None
    };

    if segments.is_empty() {
        return Err(ParseSampleSelectorError::MissingTags);
    }

    if let Some(invalid) = segments.iter().find(|segment| {
        !segment.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    }) {
        return Err(ParseSampleSelectorError::InvalidSegment(invalid.clone()));
    }

    if segments.is_empty() {
        return Err(ParseSampleSelectorError::MissingTags);
    }

    let mut selector = SampleSelector::tags(segments);
    if let Some(ordinal) = ordinal {
        selector = selector.ordinal(ordinal);
    }
    Ok(selector)
}

fn normalize_tags<T, I>(tags: I) -> Vec<String>
where
    T: Into<String>,
    I: IntoIterator<Item = T>,
{
    let mut normalized = tags
        .into_iter()
        .map(|tag| normalize_segment(tag.into()))
        .map(|tag| {
            SampleFamily::from_tag(tag.as_str())
                .map(|family| family.canonical_tag().to_string())
                .unwrap_or(tag)
        })
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_segment(value: String) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_sample_paths(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_sample_paths(&path, files)?;
        } else if path.is_file() && is_audio_file(path.as_path()) {
            files.push(path);
        }
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn key_for_sample_path(root: &Path, path: &Path) -> String {
    let mut relative = path
        .strip_prefix(root)
        .expect("sample path should stay inside the root")
        .to_path_buf();
    relative.set_extension("");
    relative.to_string_lossy().replace('\\', "/")
}

fn build_entries(samples: &HashMap<String, PathBuf>) -> Vec<SampleEntry> {
    let mut keys = samples.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys.into_iter()
        .map(|key| SampleEntry {
            library: key
                .split_once('/')
                .map(|(library, _)| library.to_ascii_lowercase()),
            tags: normalized_tokens(&key),
            key,
        })
        .collect()
}

fn normalized_tokens(key: &str) -> Vec<String> {
    let normalized = key
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();

    let mut tokens = Vec::new();
    for token in normalized.split_whitespace() {
        tokens.push(token.to_string());
        if let Some(family) = SampleFamily::from_tag(token) {
            tokens.push(family.canonical_tag().to_string());
        }
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn query_matches_entry(query: &SampleQuery, entry: &SampleEntry) -> bool {
    let library_matches = match query.library.as_deref() {
        Some(library) => entry.library.as_deref() == Some(library),
        None => true,
    };
    if !library_matches {
        return false;
    }
    query
        .tags
        .iter()
        .all(|tag| entry.tags.iter().any(|entry_tag| entry_tag == tag))
}

fn playback_limit_for_tags(tags: &[String]) -> Option<Duration> {
    tags.iter()
        .filter_map(|tag| SampleFamily::from_tag(tag))
        .filter_map(SampleFamily::playback_limit)
        .min()
}

fn choke_group_for_tags(tags: &[String]) -> Option<ChokeGroup> {
    if tags
        .iter()
        .filter_map(|tag| SampleFamily::from_tag(tag))
        .any(|family| family.choke_group() == Some(ChokeGroup::Hat))
    {
        Some(ChokeGroup::Hat)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn parse_query_selector_keeps_numeric_tags() {
        let selector = parse_query_selector("/kick/808/").expect("selector should parse");
        let SampleSelector::Query(query) = selector else {
            panic!("selector should parse as a query");
        };

        assert_eq!(query.library, None);
        assert_eq!(query.tags, vec!["808".to_string(), "kick".to_string()]);
        assert_eq!(query.ordinal, None);
    }

    #[test]
    fn parse_query_selector_supports_explicit_ordinal_suffix() {
        let selector = parse_query_selector("/kick/@2/").expect("selector should parse");
        let SampleSelector::Query(query) = selector else {
            panic!("selector should parse as a query");
        };

        assert_eq!(query.tags, vec!["kick".to_string()]);
        assert_eq!(query.ordinal, Some(2));
    }

    #[test]
    fn numeric_tag_queries_resolve_against_catalog_entries() {
        let catalog = PathSampleCatalog::new(HashMap::from([
            (
                "kicks/heavy 808".to_string(),
                PathBuf::from("/samples/kicks/heavy%20808.wav"),
            ),
            (
                "kicks/punchy kick".to_string(),
                PathBuf::from("/samples/kicks/punchy%20kick.wav"),
            ),
        ]));

        let resolved = catalog
            .resolve_name("/kick/808/")
            .expect("numeric tag query should resolve");

        assert_eq!(
            resolved.path,
            PathBuf::from("/samples/kicks/heavy%20808.wav")
        );
    }

    #[test]
    fn tag_queries_match_nested_catalog_entries_without_root_library() {
        let catalog = PathSampleCatalog::new(HashMap::from([
            (
                "kits/house/deep kick".to_string(),
                PathBuf::from("/samples/kits/house/deep%20kick.wav"),
            ),
            (
                "kits/house/deep snare".to_string(),
                PathBuf::from("/samples/kits/house/deep%20snare.wav"),
            ),
        ]));

        let resolved = catalog
            .resolve_name("/kick/")
            .expect("tag query should resolve nested entries");

        assert_eq!(
            resolved.path,
            PathBuf::from("/samples/kits/house/deep%20kick.wav")
        );
    }

    #[test]
    fn load_directory_filters_out_non_audio_files() {
        let root = temp_catalog_dir("filters_non_audio");
        fs::create_dir_all(root.join("kit")).expect("create sample dir");
        fs::write(root.join("kit").join("kick.wav"), b"wav").expect("write wav");
        fs::write(root.join("kit").join("notes.txt"), b"txt").expect("write txt");
        fs::write(root.join(".DS_Store"), b"junk").expect("write ds_store");

        let catalog = PathSampleCatalog::load_directory(&root).expect("load directory");
        let entries = catalog.entries();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "kit/kick");

        let _ = fs::remove_dir_all(root);
    }

    fn temp_catalog_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("cadence-catalog-{label}-{suffix}"))
    }
}
