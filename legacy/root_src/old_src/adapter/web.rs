use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    time::Duration,
};

use cadence_core::infrastructure::{
    audio::{Frame, SampleBuffer},
    playback::{ChokeGroup, SampleLoadOptions},
};
use js_sys::Uint8Array;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::{JsCast, prelude::*};

use crate::{
    adapter::{
        cadence_core::web_audio,
        samples::{
            PathSampleCatalog, bundled_web_sample_urls, legacy_sample_source, load_sample_bytes,
        },
        storage::{flush_storage, prepare_storage},
    },
    application::authoring::compile::compile_project,
    domain::project::CadenceSampleLoad,
    infrastructure::{api, dto::SharedAppState},
};

thread_local! {
    static WEB_RUNTIME_STATE: RefCell<WebRuntimeState> = RefCell::new(WebRuntimeState::default());
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProgramPayload {
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoad>,
    pub output_count: usize,
    pub debug_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeBootPhase {
    Idle,
    Booting,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBootStatus {
    pub phase: RuntimeBootPhase,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeSampleReadiness {
    pub ready: bool,
    pub attempted: usize,
    pub loaded: usize,
    pub failed: usize,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeSampleCacheStatus {
    pub name: String,
    pub installed: bool,
    pub available: bool,
    pub ready: bool,
    pub hits: usize,
    pub misses: usize,
    pub writes: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeInitSampleStatus {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone)]
struct CachedSample {
    sample: SampleBuffer,
    options: SampleLoadOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PersistedSampleBuffer {
    sample_rate: u32,
    interleaved: Vec<f32>,
    playback_limit_ms: Option<u64>,
    choke_group: Option<String>,
}

struct WebRuntimeState {
    boot: RuntimeBootStatus,
    sample_readiness: RuntimeSampleReadiness,
    sample_cache: RuntimeSampleCacheStatus,
    init_sample_status: Vec<RuntimeInitSampleStatus>,
    decoded_samples: BTreeMap<String, CachedSample>,
    sample_catalog: Option<PathSampleCatalog>,
}

impl Default for WebRuntimeState {
    fn default() -> Self {
        Self {
            boot: RuntimeBootStatus {
                phase: RuntimeBootPhase::Idle,
                detail: Some("web runtime idle".to_string()),
            },
            sample_readiness: RuntimeSampleReadiness {
                ready: false,
                attempted: 0,
                loaded: 0,
                failed: 0,
                failures: Vec::new(),
            },
            sample_cache: RuntimeSampleCacheStatus {
                name: "cadence-web-runtime-indexeddb".to_string(),
                installed: true,
                available: true,
                ready: false,
                hits: 0,
                misses: 0,
                writes: 0,
                last_error: None,
            },
            init_sample_status: Vec::new(),
            decoded_samples: BTreeMap::new(),
            sample_catalog: None,
        }
    }
}

#[wasm_bindgen(inline_js = r#"
export async function cadenceFetchBytes(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`failed to fetch ${url}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

let cadenceSampleCacheDbPromise = null;

function cadenceSampleCacheError(error, fallback) {
  if (!error) {
    return fallback;
  }
  if (typeof error === "string") {
    return error;
  }
  return String(error.message || error);
}

function cadenceSampleCacheTxDone(tx) {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onabort = () => reject(tx.error || new Error("IndexedDB transaction aborted."));
    tx.onerror = () => reject(tx.error || new Error("IndexedDB transaction failed."));
  });
}

function cadenceSampleCacheRequest(request, fallback) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result ?? null);
    request.onerror = () => reject(request.error || new Error(fallback));
  });
}

function cadenceOpenSampleCacheDb() {
  if (cadenceSampleCacheDbPromise) {
    return cadenceSampleCacheDbPromise;
  }

  cadenceSampleCacheDbPromise = new Promise((resolve, reject) => {
    if (typeof indexedDB === "undefined") {
      reject(new Error("IndexedDB unavailable in this browser."));
      return;
    }

    const request = indexedDB.open("cadence.sample.cache", 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains("samples")) {
        db.createObjectStore("samples");
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error("Failed to open sample cache."));
  });

  return cadenceSampleCacheDbPromise;
}

export async function cadenceSampleCacheGet(key) {
  const db = await cadenceOpenSampleCacheDb();
  const tx = db.transaction("samples", "readonly");
  const store = tx.objectStore("samples");
  const value = await cadenceSampleCacheRequest(
    store.get(String(key)),
    "Failed to read sample cache."
  );
  await cadenceSampleCacheTxDone(tx);
  return value;
}

export async function cadenceSampleCacheSet(key, value) {
  const db = await cadenceOpenSampleCacheDb();
  const tx = db.transaction("samples", "readwrite");
  tx.objectStore("samples").put(value, String(key));
  await cadenceSampleCacheTxDone(tx);
  return true;
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = cadenceFetchBytes)]
    async fn fetch_bytes_js(url: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = cadenceSampleCacheGet)]
    async fn sample_cache_get_js(key: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = cadenceSampleCacheSet)]
    async fn sample_cache_set_js(key: &str, value: JsValue) -> Result<JsValue, JsValue>;
}

pub async fn prepare_browser_storage() -> Result<(), String> {
    prepare_storage().await
}

pub async fn flush_browser_storage() -> Result<(), String> {
    flush_storage().await
}

pub async fn prime_audio_from_gesture(_state: &SharedAppState) -> Result<(), String> {
    set_boot(
        RuntimeBootPhase::Booting,
        Some("starting audio context".to_string()),
    );
    match web_audio::prime_audio_from_gesture().await {
        Ok(sample_rate) => {
            set_boot(
                RuntimeBootPhase::Ready,
                Some(format!("audio ready at {sample_rate} Hz")),
            );
            WEB_RUNTIME_STATE.with(|runtime| {
                let mut runtime = runtime.borrow_mut();
                runtime.sample_cache.installed = true;
                if runtime.sample_cache.last_error.is_none() {
                    runtime.sample_cache.available = true;
                }
            });
            Ok(())
        }
        Err(error) => {
            set_boot(RuntimeBootPhase::Error, Some(error.clone()));
            update_audio_flags(false, false, Some(error.clone()));
            Err(error)
        }
    }
}

pub async fn ensure_runtime_ready(state: &SharedAppState) -> Result<(), String> {
    set_boot(
        RuntimeBootPhase::Booting,
        Some("preparing web runtime".to_string()),
    );
    if let Err(error) = prepare_storage().await {
        set_boot(RuntimeBootPhase::Error, Some(error.clone()));
        update_audio_flags(false, false, Some(error.clone()));
        return Err(error);
    }
    sync_required_samples(state).await?;
    let sample_rate = match web_audio::ensure_audio_bridge_ready().await {
        Ok(sample_rate) => sample_rate,
        Err(error) => {
            set_boot(RuntimeBootPhase::Error, Some(error.clone()));
            update_audio_flags(false, false, Some(error.clone()));
            return Err(error);
        }
    };
    set_boot(
        RuntimeBootPhase::Ready,
        Some(format!("audio ready at {sample_rate} Hz")),
    );
    WEB_RUNTIME_STATE.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        runtime.sample_cache.installed = true;
        runtime.sample_cache.ready = runtime.sample_readiness.ready;
        if runtime.sample_cache.last_error.is_none() {
            runtime.sample_cache.available = true;
        }
    });
    Ok(())
}

pub fn runtime_boot_status(state: &SharedAppState) -> Result<RuntimeBootStatus, String> {
    refresh_manifest(state);
    sync_audio_errors();
    Ok(with_state(|state| state.boot.clone()))
}

pub fn runtime_sample_readiness(state: &SharedAppState) -> Result<RuntimeSampleReadiness, String> {
    refresh_manifest(state);
    sync_audio_errors();
    Ok(with_state(|state| state.sample_readiness.clone()))
}

pub fn runtime_sample_cache_status(
    state: &SharedAppState,
) -> Result<RuntimeSampleCacheStatus, String> {
    refresh_manifest(state);
    sync_audio_errors();
    Ok(with_state(|state| state.sample_cache.clone()))
}

pub fn runtime_init_sample_status(
    state: &SharedAppState,
) -> Result<Vec<RuntimeInitSampleStatus>, String> {
    refresh_manifest(state);
    Ok(with_state(|state| state.init_sample_status.clone()))
}

pub async fn run_cadence_program(
    state: &SharedAppState,
    _program: &RuntimeProgramPayload,
) -> Result<(), String> {
    ensure_runtime_ready(state).await
}

pub async fn stop_program(state: &SharedAppState) -> Result<(), String> {
    let _ = api::runtime_stop(state);
    Ok(())
}

pub fn get_runtime_audio_time(_state: &SharedAppState) -> Result<f64, String> {
    Ok(web_audio::audio_time())
}

pub(crate) fn resolve_cached_sample(selector: &str) -> Option<(SampleBuffer, SampleLoadOptions)> {
    with_state(|state| {
        state
            .decoded_samples
            .get(selector)
            .cloned()
            .map(|cached| (cached.sample, cached.options))
    })
}

fn refresh_manifest(state: &SharedAppState) {
    let sample_loads = sample_loads_for_state(state);
    let sample_loads = sample_loads.unwrap_or_default();

    WEB_RUNTIME_STATE.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let statuses = sample_loads
            .iter()
            .map(|sample| {
                let ready = runtime.decoded_samples.contains_key(sample.id.as_str());
                RuntimeInitSampleStatus {
                    id: sample.id.clone(),
                    source: sample.source.clone(),
                    aliases: sample.aliases.clone(),
                    status: if ready {
                        "ready".to_string()
                    } else {
                        "pending".to_string()
                    },
                    error: None,
                }
            })
            .collect::<Vec<_>>();
        let failures = statuses
            .iter()
            .filter_map(|status| status.error.clone())
            .collect::<Vec<_>>();
        runtime.sample_readiness = RuntimeSampleReadiness {
            ready: !statuses.is_empty()
                && failures.is_empty()
                && statuses.iter().all(|status| status.status == "ready"),
            attempted: statuses.len(),
            loaded: statuses
                .iter()
                .filter(|status| status.status == "ready")
                .count(),
            failed: failures.len(),
            failures,
        };
        runtime.init_sample_status = statuses;
        runtime.sample_cache.ready = runtime.sample_readiness.ready;
    });
}

async fn sync_sample_manifest(state: &SharedAppState, fetch_missing: bool) -> Result<(), String> {
    let _ = fetch_missing;
    refresh_manifest(state);
    Ok(())
}

#[derive(Debug, Clone)]
struct ResolvedWebSample {
    cache_key: String,
    fetch_url: String,
    options: SampleLoadOptions,
}

async fn sync_required_samples(state: &SharedAppState) -> Result<(), String> {
    let sample_loads = sample_loads_for_state(state)?;
    let selectors = required_sample_selectors(state)?;

    for selector in selectors {
        let already_ready =
            with_state(|runtime| runtime.decoded_samples.contains_key(selector.as_str()));
        if already_ready {
            WEB_RUNTIME_STATE.with(|runtime| runtime.borrow_mut().sample_cache.hits += 1);
            continue;
        }

        if let Some(sample) = sample_loads
            .iter()
            .find(|sample| selector_matches_manifest_sample(selector.as_str(), sample))
        {
            let resolved = resolve_manifest_sample(sample).await?;
            if let Some(cached) = load_cached_sample(resolved.cache_key.as_str()).await? {
                WEB_RUNTIME_STATE.with(|runtime| {
                    let mut runtime = runtime.borrow_mut();
                    runtime.sample_cache.hits += 1;
                    register_sample_aliases(&mut runtime.decoded_samples, sample, cached);
                });
                continue;
            }

            WEB_RUNTIME_STATE.with(|runtime| runtime.borrow_mut().sample_cache.misses += 1);
            let cached = fetch_and_decode_sample(
                sample.id.as_str(),
                resolved.fetch_url.as_str(),
                resolved.options,
            )
            .await?;
            WEB_RUNTIME_STATE.with(|runtime| {
                let mut runtime = runtime.borrow_mut();
                register_sample_aliases(&mut runtime.decoded_samples, sample, cached.clone());
            });
            match store_cached_sample(resolved.cache_key.as_str(), &cached).await {
                Ok(()) => {
                    WEB_RUNTIME_STATE.with(|runtime| runtime.borrow_mut().sample_cache.writes += 1)
                }
                Err(error) => record_sample_cache_error(error),
            }
            continue;
        }

        let resolved = resolve_catalog_selector(selector.as_str()).await?;
        if let Some(cached) = load_cached_sample(resolved.cache_key.as_str()).await? {
            WEB_RUNTIME_STATE.with(|runtime| {
                let mut runtime = runtime.borrow_mut();
                runtime.sample_cache.hits += 1;
                register_selector_cache(
                    &mut runtime.decoded_samples,
                    selector.as_str(),
                    &resolved,
                    cached,
                );
            });
            continue;
        }

        WEB_RUNTIME_STATE.with(|runtime| runtime.borrow_mut().sample_cache.misses += 1);
        let cached = fetch_and_decode_sample(
            selector.as_str(),
            resolved.fetch_url.as_str(),
            resolved.options.clone(),
        )
        .await?;
        WEB_RUNTIME_STATE.with(|runtime| {
            let mut runtime = runtime.borrow_mut();
            register_selector_cache(
                &mut runtime.decoded_samples,
                selector.as_str(),
                &resolved,
                cached.clone(),
            );
        });
        match store_cached_sample(resolved.cache_key.as_str(), &cached).await {
            Ok(()) => {
                WEB_RUNTIME_STATE.with(|runtime| runtime.borrow_mut().sample_cache.writes += 1)
            }
            Err(error) => record_sample_cache_error(error),
        }
    }

    sync_sample_manifest(state, false).await?;
    Ok(())
}

async fn fetch_and_decode_sample(
    sample_label: &str,
    fetch_url: &str,
    options: SampleLoadOptions,
) -> Result<CachedSample, String> {
    let value = fetch_bytes_js(fetch_url).await.map_err(js_error)?;
    let bytes = Uint8Array::new(&value).to_vec();
    let extension = sample_extension(fetch_url);
    let buffer = load_sample_bytes(bytes, extension.as_deref())
        .map_err(|error| format!("failed to decode sample `{sample_label}`: {error}"))?;

    Ok(CachedSample {
        sample: buffer,
        options,
    })
}

async fn ensure_sample_catalog_loaded() -> Result<(), String> {
    let already_loaded = with_state(|runtime| runtime.sample_catalog.is_some());
    if already_loaded {
        return Ok(());
    }

    let samples = bundled_web_sample_urls()
        .into_iter()
        .map(|(key, url)| (key, PathBuf::from(url)))
        .collect::<HashMap<_, _>>();
    WEB_RUNTIME_STATE.with(|runtime| {
        runtime.borrow_mut().sample_catalog = Some(PathSampleCatalog::new(samples));
    });
    Ok(())
}

async fn resolve_manifest_sample(sample: &CadenceSampleLoad) -> Result<ResolvedWebSample, String> {
    let source = legacy_sample_source(sample.source.as_str()).unwrap_or(sample.source.as_str());
    if let Some(direct_url) = direct_fetch_url(source) {
        return Ok(ResolvedWebSample {
            cache_key: direct_url.clone(),
            fetch_url: direct_url,
            options: SampleLoadOptions::default(),
        });
    }

    resolve_catalog_selector(source).await
}

async fn resolve_catalog_selector(selector: &str) -> Result<ResolvedWebSample, String> {
    ensure_sample_catalog_loaded().await?;
    let catalog = with_state(|runtime| runtime.sample_catalog.clone())
        .ok_or_else(|| "sample catalog unavailable in web runtime".to_string())?;
    let resolved = catalog
        .resolve_name(selector)
        .map_err(|error| format!("failed to resolve sample selector `{selector}`: {error}"))?;
    let fetch_url = resolved.path.to_string_lossy().replace('\\', "/");

    Ok(ResolvedWebSample {
        cache_key: fetch_url.clone(),
        fetch_url,
        options: SampleLoadOptions {
            playback_limit: resolved.playback_limit,
            choke_group: resolved.choke_group,
            ..SampleLoadOptions::default()
        },
    })
}

async fn load_cached_sample(source: &str) -> Result<Option<CachedSample>, String> {
    let value = sample_cache_get_js(source).await.map_err(js_error)?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }

    let persisted: PersistedSampleBuffer = from_value(value)
        .map_err(|error| format!("failed to deserialize sample cache: {error}"))?;
    Ok(Some(cached_sample_from_persisted(persisted)?))
}

async fn store_cached_sample(source: &str, cached: &CachedSample) -> Result<(), String> {
    let persisted = persisted_sample_from_cached(cached);
    let value = to_value(&persisted)
        .map_err(|error| format!("failed to serialize sample cache entry: {error}"))?;
    sample_cache_set_js(source, value)
        .await
        .map(|_| ())
        .map_err(js_error)
}

fn persisted_sample_from_cached(cached: &CachedSample) -> PersistedSampleBuffer {
    let interleaved = cached
        .sample
        .frames()
        .iter()
        .flat_map(|frame| [frame.left, frame.right])
        .collect::<Vec<_>>();

    PersistedSampleBuffer {
        sample_rate: cached.sample.sample_rate(),
        interleaved,
        playback_limit_ms: cached
            .options
            .playback_limit
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64),
        choke_group: choke_group_name(cached.options.choke_group),
    }
}

fn cached_sample_from_persisted(persisted: PersistedSampleBuffer) -> Result<CachedSample, String> {
    if persisted.interleaved.len() % 2 != 0 {
        return Err("persisted sample cache entry had an odd stereo frame count".to_string());
    }

    let frames = persisted
        .interleaved
        .chunks_exact(2)
        .map(|channels| Frame::new(channels[0], channels[1]))
        .collect::<Vec<_>>();

    Ok(CachedSample {
        sample: SampleBuffer::new(persisted.sample_rate, frames),
        options: SampleLoadOptions {
            playback_limit: persisted.playback_limit_ms.map(Duration::from_millis),
            choke_group: choke_group_from_name(persisted.choke_group.as_deref())?,
            ..SampleLoadOptions::default()
        },
    })
}

fn choke_group_name(group: Option<ChokeGroup>) -> Option<String> {
    match group {
        Some(ChokeGroup::Hat) => Some("hat".to_string()),
        None => None,
    }
}

fn choke_group_from_name(name: Option<&str>) -> Result<Option<ChokeGroup>, String> {
    match name {
        Some("hat") => Ok(Some(ChokeGroup::Hat)),
        Some(other) => Err(format!("unsupported persisted choke group `{other}`")),
        None => Ok(None),
    }
}

fn register_sample_aliases(
    decoded_samples: &mut BTreeMap<String, CachedSample>,
    sample: &CadenceSampleLoad,
    cached: CachedSample,
) {
    decoded_samples.insert(sample.id.clone(), cached.clone());
    decoded_samples.insert(sample.source.clone(), cached.clone());
    for (alias, target) in &sample.aliases {
        decoded_samples.insert(alias.clone(), cached.clone());
        decoded_samples.insert(target.clone(), cached.clone());
    }
}

fn register_selector_cache(
    decoded_samples: &mut BTreeMap<String, CachedSample>,
    selector: &str,
    resolved: &ResolvedWebSample,
    cached: CachedSample,
) {
    decoded_samples.insert(selector.to_string(), cached.clone());
    decoded_samples.insert(resolved.fetch_url.clone(), cached);
}

fn selector_matches_manifest_sample(selector: &str, sample: &CadenceSampleLoad) -> bool {
    sample.id == selector
        || sample.source == selector
        || sample.aliases.contains_key(selector)
        || sample.aliases.values().any(|value| value == selector)
}

fn sample_loads_for_state(state: &SharedAppState) -> Result<Vec<CadenceSampleLoad>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    Ok(store
        .current_project
        .as_ref()
        .map(|project| project.init_stage.sample_loads.clone())
        .unwrap_or_default())
}

fn required_sample_selectors(state: &SharedAppState) -> Result<Vec<String>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let selectors = store
        .current_project
        .as_ref()
        .map(|project| compile_project(project).sample_selectors)
        .unwrap_or_default();

    let mut seen = BTreeSet::new();
    Ok(selectors
        .into_iter()
        .filter(|selector| seen.insert(selector.clone()))
        .collect())
}

fn sample_extension(source: &str) -> Option<String> {
    let stem = source.split(['?', '#']).next().unwrap_or(source);
    Path::new(stem)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(ToOwned::to_owned)
}

fn direct_fetch_url(source: &str) -> Option<String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        return Some(source.to_string());
    }
    if sample_extension(source).is_some() && source.starts_with('/') {
        return Some(source.to_string());
    }
    None
}

fn set_boot(phase: RuntimeBootPhase, detail: Option<String>) {
    WEB_RUNTIME_STATE.with(|runtime| {
        runtime.borrow_mut().boot = RuntimeBootStatus { phase, detail };
    });
}

fn update_audio_flags(installed: bool, ready: bool, last_error: Option<String>) {
    WEB_RUNTIME_STATE.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        runtime.sample_cache.installed = installed;
        runtime.sample_cache.available = installed;
        runtime.sample_cache.ready = ready;
        runtime.sample_cache.last_error = last_error;
    });
}

fn record_sample_cache_error(error: String) {
    WEB_RUNTIME_STATE.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        runtime.sample_cache.last_error = Some(error);
        runtime.sample_cache.available = false;
    });
}

fn sync_audio_errors() {
    if let Some(error) = web_audio::take_last_error() {
        set_boot(RuntimeBootPhase::Error, Some(error.clone()));
        update_audio_flags(true, false, Some(error));
    }
}

fn with_state<T>(map: impl FnOnce(&WebRuntimeState) -> T) -> T {
    WEB_RUNTIME_STATE.with(|runtime| map(&runtime.borrow()))
}

fn js_error(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            error
                .dyn_ref::<js_sys::Error>()
                .map(|value| value.message().into())
        })
        .or_else(|| {
            js_sys::Reflect::get(&error, &JsValue::from_str("message"))
                .ok()
                .and_then(|value| value.as_string())
        })
        .unwrap_or_else(|| "web sample fetch failed".to_string())
}
