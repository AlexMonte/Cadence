#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
use std::{collections::BTreeSet, fmt};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
};

#[cfg(not(target_arch = "wasm32"))]
use cadence_core::infrastructure::audio::AudioControl;
#[cfg(test)]
use cadence_core::infrastructure::audio::AudioRenderer;
use cadence_core::infrastructure::{
    audio::SampleBuffer,
    playback::{
        PlaybackRuntime, PlaybackSettings, PlaybackState, PlaybackStatus, SampleLoadOptions,
    },
    score::Score,
    voice::Time,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::adapter::cadence_core::native_audio::CpalAudioBackend;
#[cfg(not(target_arch = "wasm32"))]
use crate::adapter::samples::{
    PathSampleCatalog, ResolvedSample, bundled_sample_root, legacy_sample_source, load_sample,
};
use crate::application::authoring::compile::CompiledProject;
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::project::CadenceSampleLoad;
#[cfg(target_arch = "wasm32")]
use crate::{adapter::cadence_core::web_audio::WebAudioBackend, web};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

const AUDIO_QUEUE_CAPACITY: usize = 1024;
const DEFAULT_CPM: f32 = 120.0;
#[derive(Debug)]
struct DecodedSampleLoad {
    name: String,
    sample: SampleBuffer,
    options: SampleLoadOptions,
}

#[cfg(not(target_arch = "wasm32"))]
enum DesktopRuntimeCommand {
    LoadSample {
        name: String,
        sample: SampleBuffer,
        options: SampleLoadOptions,
    },
    CommitScore {
        score: Score,
        cps: Time,
        reply: Sender<Result<(), String>>,
    },
    Stop {
        reply: Sender<Result<(), String>>,
    },
    Shutdown,
}

#[cfg(not(target_arch = "wasm32"))]
enum DesktopAudioBackend {
    Native {
        _backend: CpalAudioBackend,
    },
    #[cfg(test)]
    Test {
        _renderer: AudioRenderer,
    },
}

#[cfg(not(target_arch = "wasm32"))]
struct DesktopRuntimeHandle {
    _audio_backend: DesktopAudioBackend,
    command_tx: Sender<DesktopRuntimeCommand>,
    latest_status: Arc<RwLock<PlaybackStatus>>,
    control_thread: Option<JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DesktopRuntimeHandle {
    fn new() -> Result<Self, String> {
        let (audio_backend, audio) =
            CpalAudioBackend::new(AUDIO_QUEUE_CAPACITY).map_err(|error| error.to_string())?;
        Ok(Self::spawn(
            DesktopAudioBackend::Native {
                _backend: audio_backend,
            },
            audio,
        ))
    }

    fn spawn(audio_backend: DesktopAudioBackend, audio: AudioControl) -> Self {
        let settings = PlaybackSettings::default();
        let initial_status = PlaybackStatus {
            state: PlaybackState::Stopped,
            cycle_position: Time::ZERO,
            cps: settings.cps,
        };
        let latest_status = Arc::new(RwLock::new(initial_status));
        let (command_tx, command_rx) = mpsc::channel();
        let status_for_thread = Arc::clone(&latest_status);

        let control_thread = thread::spawn(move || {
            run_control_thread(settings, audio, command_rx, status_for_thread);
        });

        Self {
            _audio_backend: audio_backend,
            command_tx,
            latest_status,
            control_thread: Some(control_thread),
        }
    }

    fn load_sample(
        &self,
        name: String,
        sample: SampleBuffer,
        options: SampleLoadOptions,
    ) -> Result<(), String> {
        self.command_tx
            .send(DesktopRuntimeCommand::LoadSample {
                name,
                sample,
                options,
            })
            .map_err(|_| "desktop runtime command channel closed".to_string())
    }

    fn commit_score(&self, score: Score, cps: Time) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.command_tx
            .send(DesktopRuntimeCommand::CommitScore {
                score,
                cps,
                reply: reply_tx,
            })
            .map_err(|_| "desktop runtime command channel closed".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "desktop runtime reply channel closed".to_string())?
    }

    fn stop(&self) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.command_tx
            .send(DesktopRuntimeCommand::Stop { reply: reply_tx })
            .map_err(|_| "desktop runtime command channel closed".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "desktop runtime reply channel closed".to_string())?
    }

    fn status(&self) -> PlaybackStatus {
        match self.latest_status.read() {
            Ok(status) => *status,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    #[cfg(test)]
    fn for_tests(renderer: AudioRenderer, audio: AudioControl) -> Self {
        Self::spawn(
            DesktopAudioBackend::Test {
                _renderer: renderer,
            },
            audio,
        )
    }

    #[cfg(test)]
    fn shutdown_for_tests(&mut self) {
        let _ = self.command_tx.send(DesktopRuntimeCommand::Shutdown);
        if let Some(control_thread) = self.control_thread.take() {
            let _ = control_thread.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for DesktopRuntimeHandle {
    fn drop(&mut self) {
        let _ = self.command_tx.send(DesktopRuntimeCommand::Shutdown);
        if let Some(control_thread) = self.control_thread.take() {
            let _ = control_thread.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_control_thread(
    settings: PlaybackSettings,
    audio: AudioControl,
    command_rx: Receiver<DesktopRuntimeCommand>,
    latest_status: Arc<RwLock<PlaybackStatus>>,
) {
    let mut runtime = PlaybackRuntime::new(settings, audio);
    publish_status(&latest_status, runtime.status());

    let mut running = true;
    while running {
        let timeout = runtime.tick_interval_hint();
        match command_rx.recv_timeout(timeout) {
            Ok(command) => {
                running = apply_command(command, &mut runtime, &latest_status);
                while running {
                    match command_rx.try_recv() {
                        Ok(command) => {
                            running = apply_command(command, &mut runtime, &latest_status);
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            running = false;
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                running = false;
            }
        }

        if running {
            if let Err(error) = runtime.tick() {
                report_tick_error(error.to_string().as_str());
            }
            publish_status(&latest_status, runtime.status());
        }
    }

    let _ = runtime.stop();
    publish_status(&latest_status, runtime.status());
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_command(
    command: DesktopRuntimeCommand,
    runtime: &mut PlaybackRuntime,
    latest_status: &Arc<RwLock<PlaybackStatus>>,
) -> bool {
    match command {
        DesktopRuntimeCommand::LoadSample {
            name,
            sample,
            options,
        } => {
            runtime.load_sample_with_options(name, sample, options);
            true
        }
        DesktopRuntimeCommand::CommitScore { score, cps, reply } => {
            let result = runtime
                .set_cps(cps)
                .map_err(|error| error.to_string())
                .and_then(|()| {
                    if matches!(runtime.status().state, PlaybackState::Playing) {
                        runtime
                            .replace_score(score)
                            .map_err(|error| error.to_string())
                    } else {
                        runtime.play_score(score).map_err(|error| error.to_string())
                    }
                });
            publish_status(latest_status, runtime.status());
            let _ = reply.send(result);
            true
        }
        DesktopRuntimeCommand::Stop { reply } => {
            let result = runtime.stop().map_err(|error| error.to_string());
            publish_status(latest_status, runtime.status());
            let _ = reply.send(result);
            true
        }
        DesktopRuntimeCommand::Shutdown => {
            let _ = runtime.stop();
            publish_status(latest_status, runtime.status());
            false
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn publish_status(target: &Arc<RwLock<PlaybackStatus>>, status: PlaybackStatus) {
    match target.write() {
        Ok(mut current) => *current = status,
        Err(poisoned) => *poisoned.into_inner() = status,
    }
}

#[cfg(target_arch = "wasm32")]
struct DesktopRuntimeHandle {
    _audio_backend: WebAudioBackend,
    runtime: Rc<RefCell<PlaybackRuntime>>,
    latest_status: Rc<RefCell<PlaybackStatus>>,
    tick_interval_id: i32,
    #[allow(dead_code)]
    tick_closure: Closure<dyn FnMut()>,
}

#[cfg(target_arch = "wasm32")]
impl DesktopRuntimeHandle {
    fn new() -> Result<Self, String> {
        let (audio_backend, audio) =
            WebAudioBackend::new(AUDIO_QUEUE_CAPACITY).map_err(|error| error.to_string())?;
        let settings = PlaybackSettings::default();
        let runtime = Rc::new(RefCell::new(PlaybackRuntime::new(settings, audio)));
        let latest_status = Rc::new(RefCell::new(runtime.borrow().status()));
        let runtime_for_tick = Rc::clone(&runtime);
        let latest_for_tick = Rc::clone(&latest_status);
        let tick_closure = Closure::wrap(Box::new(move || {
            let mut runtime = runtime_for_tick.borrow_mut();
            if let Err(error) = runtime.tick() {
                report_tick_error(error.to_string().as_str());
            }
            *latest_for_tick.borrow_mut() = runtime.status();
        }) as Box<dyn FnMut()>);
        let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
        let tick_interval_id = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                tick_closure.as_ref().unchecked_ref(),
                16,
            )
            .map_err(|error| {
                error
                    .as_string()
                    .unwrap_or_else(|| "failed to schedule runtime tick".to_string())
            })?;

        Ok(Self {
            _audio_backend: audio_backend,
            runtime,
            latest_status,
            tick_interval_id,
            tick_closure,
        })
    }

    fn load_sample(
        &self,
        name: String,
        sample: SampleBuffer,
        options: SampleLoadOptions,
    ) -> Result<(), String> {
        self.runtime
            .borrow()
            .sample_bank()
            .load_with_options(name, sample, options);
        Ok(())
    }

    fn commit_score(&self, score: Score, cps: Time) -> Result<(), String> {
        let mut runtime = self.runtime.borrow_mut();
        runtime.set_cps(cps).map_err(|error| error.to_string())?;
        if matches!(runtime.status().state, PlaybackState::Playing) {
            runtime
                .replace_score(score)
                .map_err(|error| error.to_string())?;
        } else {
            runtime
                .play_score(score)
                .map_err(|error| error.to_string())?;
        }
        *self.latest_status.borrow_mut() = runtime.status();
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        let mut runtime = self.runtime.borrow_mut();
        runtime.stop().map_err(|error| error.to_string())?;
        *self.latest_status.borrow_mut() = runtime.status();
        Ok(())
    }

    fn status(&self) -> PlaybackStatus {
        *self.latest_status.borrow()
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for DesktopRuntimeHandle {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(self.tick_interval_id);
        }
    }
}

#[derive(Default)]
pub(crate) struct MusicRuntimeHost {
    #[cfg(not(target_arch = "wasm32"))]
    sample_root: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    catalog: Option<PathSampleCatalog>,
    loaded_samples: BTreeSet<String>,
    desktop: Option<DesktopRuntimeHandle>,
}

// Safety: the host is only accessed behind the app-store mutex on the native
// desktop path, so we do not move or use the underlying audio objects
// concurrently across threads.
unsafe impl Send for MusicRuntimeHost {}

impl fmt::Debug for MusicRuntimeHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("MusicRuntimeHost");
        #[cfg(not(target_arch = "wasm32"))]
        debug
            .field("sample_root", &self.sample_root)
            .field("catalog_loaded", &self.catalog.is_some());
        debug
            .field("loaded_samples", &self.loaded_samples.len())
            .field("desktop_ready", &self.desktop.is_some())
            .finish()
    }
}

impl MusicRuntimeHost {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn set_sample_root(&mut self, sample_root: Option<PathBuf>) {
        if self.sample_root == sample_root {
            return;
        }

        self.sample_root = sample_root;
        self.catalog = None;
        self.loaded_samples.clear();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_sample_root(&mut self, _sample_root: Option<std::path::PathBuf>) {
        self.loaded_samples.clear();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn preload_sample_loads(
        &mut self,
        sample_loads: &[CadenceSampleLoad],
        selectors: &[String],
    ) -> Result<(), String> {
        let pending = self.decode_manifest_samples(sample_loads, selectors)?;
        if pending.is_empty() {
            return Ok(());
        }

        let loaded_names = pending
            .iter()
            .map(|load| load.name.clone())
            .collect::<Vec<_>>();
        let result = {
            let desktop = self.ensure_runtime()?;
            for load in pending {
                desktop.load_sample(load.name, load.sample, load.options)?;
            }
            Ok::<(), String>(())
        };

        match result {
            Ok(()) => {
                self.loaded_samples.extend(loaded_names);
                Ok(())
            }
            Err(error) => {
                self.reset_runtime_bundle();
                Err(error)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn preload_sample_loads(
        &mut self,
        _sample_loads: &[crate::domain::project::CadenceSampleLoad],
        _selectors: &[String],
    ) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn play(
        &mut self,
        compiled: &CompiledProject,
        cpm: Option<f32>,
    ) -> Result<(), String> {
        let Some(score) = compiled.playback_score.clone() else {
            return Err("runtime graph did not produce any playable score output".to_string());
        };
        if compiled.output_count == 0 {
            return Err("runtime graph did not produce any score outputs".to_string());
        }

        let pending_loads = self.decode_missing_samples(compiled.sample_selectors.as_slice())?;
        let loaded_names = pending_loads
            .iter()
            .map(|load| load.name.clone())
            .collect::<Vec<_>>();
        let cps = select_cps(compiled.cps, cpm);

        let result = {
            let desktop = self.ensure_runtime()?;
            for load in pending_loads {
                desktop.load_sample(load.name, load.sample, load.options)?;
            }
            desktop.commit_score(score, cps)
        };

        match result {
            Ok(()) => {
                self.loaded_samples.extend(loaded_names);
                Ok(())
            }
            Err(error) => {
                self.reset_runtime_bundle();
                Err(error)
            }
        }
    }

    pub(crate) fn stop(&mut self) -> Result<(), String> {
        let result = match self.desktop.as_ref() {
            Some(desktop) => desktop.stop(),
            None => Ok(()),
        };
        if result.is_err() {
            self.reset_runtime_bundle();
        }
        result
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn decode_manifest_samples(
        &mut self,
        sample_loads: &[CadenceSampleLoad],
        selectors: &[String],
    ) -> Result<Vec<DecodedSampleLoad>, String> {
        let to_load = selectors
            .iter()
            .filter(|selector| !self.loaded_samples.contains((*selector).as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if to_load.is_empty() {
            return Ok(Vec::new());
        }

        let catalog = self.catalog().ok().cloned();
        let mut decoded = Vec::new();
        for selector in to_load {
            let Some(sample) = sample_loads
                .iter()
                .find(|sample| selector_matches_manifest_sample(selector.as_str(), sample))
            else {
                continue;
            };
            let ResolvedManifestSample { path, options } =
                resolve_manifest_sample_source(sample.source.as_str(), catalog.as_ref())?;
            let buffer = load_sample(path.as_path()).map_err(|error| {
                format!(
                    "failed to decode sample `{}` from {}: {error}",
                    sample.id,
                    path.display()
                )
            })?;
            decoded.push(DecodedSampleLoad {
                name: selector,
                sample: buffer,
                options,
            });
        }

        Ok(decoded)
    }

    pub(crate) fn status(&self) -> PlaybackStatus {
        self.desktop
            .as_ref()
            .map(DesktopRuntimeHandle::status)
            .unwrap_or_else(default_status)
    }

    fn ensure_runtime(&mut self) -> Result<&DesktopRuntimeHandle, String> {
        if self.desktop.is_none() {
            self.desktop = Some(DesktopRuntimeHandle::new()?);
        }
        self.desktop
            .as_ref()
            .ok_or_else(|| "runtime unavailable".to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn decode_missing_samples(
        &mut self,
        selectors: &[String],
    ) -> Result<Vec<DecodedSampleLoad>, String> {
        let selectors = selectors.to_vec();
        if selectors.is_empty() {
            return Ok(Vec::new());
        }

        let to_load = selectors
            .iter()
            .filter(|selector| !self.loaded_samples.contains((*selector).as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if to_load.is_empty() {
            return Ok(Vec::new());
        }

        let mut catalog: Option<PathSampleCatalog> = None;
        let mut decoded = Vec::new();
        for selector in &to_load {
            if catalog.is_none() {
                catalog = Some(self.catalog()?.clone());
            }
            let catalog = catalog
                .as_ref()
                .expect("sample catalog should be initialized before lookup");
            let resolved = catalog
                .resolve_name(selector)
                .map_err(|error| format!("failed to resolve sample `{selector}`: {error}"))?;
            let buffer = load_sample(&resolved.path)
                .map_err(|error| format!("failed to decode sample `{selector}`: {error}"))?;
            decoded.push(DecodedSampleLoad {
                name: selector.clone(),
                sample: buffer,
                options: SampleLoadOptions {
                    playback_limit: resolved.playback_limit,
                    choke_group: resolved.choke_group,
                    ..SampleLoadOptions::default()
                },
            });
        }

        Ok(decoded)
    }

    #[cfg(target_arch = "wasm32")]
    fn decode_missing_samples(
        &mut self,
        selectors: &[String],
    ) -> Result<Vec<DecodedSampleLoad>, String> {
        let selectors = selectors.to_vec();
        if selectors.is_empty() {
            return Ok(Vec::new());
        }

        let to_load = selectors
            .iter()
            .filter(|selector| !self.loaded_samples.contains((*selector).as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if to_load.is_empty() {
            return Ok(Vec::new());
        }

        let mut decoded = Vec::new();
        for selector in to_load {
            let Some((sample, options)) = web::resolve_cached_sample(selector.as_str()) else {
                return Err(format!(
                    "sample `{selector}` is not ready in web runtime; configure init-stage sample loads and run play again"
                ));
            };
            decoded.push(DecodedSampleLoad {
                name: selector,
                sample,
                options,
            });
        }
        Ok(decoded)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn catalog(&mut self) -> Result<&PathSampleCatalog, String> {
        if self.catalog.is_none() {
            let root = self
                .sample_root
                .clone()
                .or_else(default_sample_root)
                .ok_or_else(|| {
                    "sample root unavailable; set CADENCE_SAMPLE_ROOT or open a project next to a `samples/` directory"
                        .to_string()
                })?;
            let catalog = PathSampleCatalog::load_directory(&root).map_err(|error| {
                format!(
                    "failed to load sample catalog from {}: {error}",
                    root.display()
                )
            })?;
            self.sample_root = Some(root);
            self.catalog = Some(catalog);
        }
        self.catalog
            .as_ref()
            .ok_or_else(|| "sample catalog unavailable".to_string())
    }

    fn reset_runtime_bundle(&mut self) {
        self.desktop = None;
        self.loaded_samples.clear();
    }
}

fn default_status() -> PlaybackStatus {
    PlaybackStatus {
        state: PlaybackState::Stopped,
        cycle_position: Time::ZERO,
        cps: PlaybackSettings::default().cps,
    }
}

fn select_cps(project_cps: Option<Time>, cpm: Option<f32>) -> Time {
    cpm.map(cpm_to_cps)
        .or(project_cps)
        .unwrap_or_else(|| cpm_to_cps(DEFAULT_CPM))
}

fn cpm_to_cps(cpm: f32) -> Time {
    let clamped = cpm
        .is_finite()
        .then_some(cpm)
        .unwrap_or(DEFAULT_CPM)
        .clamp(1.0, 480.0);
    const SCALE: i64 = 1_000;
    Time::new((clamped as f64 * SCALE as f64 / 60.0).round() as i64, SCALE)
}

#[cfg(not(target_arch = "wasm32"))]
fn selector_matches_manifest_sample(selector: &str, sample: &CadenceSampleLoad) -> bool {
    crate::domain::project::sample_selector_matches(selector, sample)
}

#[cfg(not(target_arch = "wasm32"))]
fn default_sample_root() -> Option<PathBuf> {
    std::env::var_os("CADENCE_SAMPLE_ROOT")
        .map(PathBuf::from)
        .or_else(bundled_sample_root)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct ResolvedManifestSample {
    path: PathBuf,
    options: SampleLoadOptions,
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_manifest_sample_source(
    source: &str,
    catalog: Option<&PathSampleCatalog>,
) -> Result<ResolvedManifestSample, String> {
    let canonical = legacy_sample_source(source).unwrap_or(source);
    if let Some(resolved) = resolve_source_from_catalog(canonical, catalog)? {
        return Ok(ResolvedManifestSample {
            path: resolved.path,
            options: SampleLoadOptions {
                playback_limit: resolved.playback_limit,
                choke_group: resolved.choke_group,
                ..SampleLoadOptions::default()
            },
        });
    }

    Err(format!(
        "sample source `{source}` is not a bundled selector or a catalog entry"
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_source_from_catalog(
    source: &str,
    catalog: Option<&PathSampleCatalog>,
) -> Result<Option<ResolvedSample>, String> {
    let Some(catalog) = catalog else {
        return Ok(None);
    };
    let Some(selector) = catalog_lookup_name(source) else {
        return Ok(None);
    };
    catalog
        .resolve_name(selector.as_str())
        .map(Some)
        .map_err(|error| format!("failed to resolve sample source `{source}`: {error}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn catalog_lookup_name(source: &str) -> Option<String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        return None;
    }
    if let Some(stripped) = source.strip_prefix("/samples/") {
        let stem = stripped.split(['?', '#']).next().unwrap_or(stripped);
        let mut path = PathBuf::from(stem);
        path.set_extension("");
        return Some(path.to_string_lossy().replace('\\', "/"));
    }

    Some(source.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn report_tick_error(message: &str) {
    eprintln!("cadence playback tick failed: {message}");
}

#[cfg(target_arch = "wasm32")]
fn report_tick_error(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(
        format!("cadence playback tick failed: {message}").as_str(),
    ));
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use cadence_core::infrastructure::{
        audio::{AudioRenderer, AudioRendererSettings, Frame},
        score::Score,
        voice::{Intent, Repeat, Tile, Voice},
    };

    use super::*;
    use crate::{
        application::authoring::compile::CompiledProject,
        domain::{preview::PreviewDocument, program::CadenceProgram, project::CadenceSampleLoad},
    };

    fn sample_score(sample: &str) -> Score {
        Score::from(
            Voice::new(
                Time::ONE,
                vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample(sample)).unwrap()],
            )
            .unwrap()
            .with_repeat(Repeat::Forever),
        )
    }

    fn compiled_project(sample: &str) -> CompiledProject {
        CompiledProject {
            program: CadenceProgram::default(),
            diagnostics: Vec::new(),
            preview: PreviewDocument::default(),
            can_render: true,
            can_play: true,
            sample_selectors: vec![sample.to_string()],
            cps: None,
            output_count: 1,
            playback_score: Some(sample_score(sample)),
        }
    }

    fn compiled_synth_project(
        source: cadence_core::infrastructure::voice::BuiltInSynthSource,
    ) -> CompiledProject {
        let score = Score::from(
            Voice::new(
                Time::ONE,
                vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::synth(source)).unwrap()],
            )
            .unwrap()
            .with_repeat(Repeat::Forever),
        );

        CompiledProject {
            program: CadenceProgram::default(),
            diagnostics: Vec::new(),
            preview: PreviewDocument::default(),
            can_render: true,
            can_play: true,
            sample_selectors: Vec::new(),
            cps: None,
            output_count: 1,
            playback_score: Some(score),
        }
    }

    fn test_runtime_handle() -> DesktopRuntimeHandle {
        let (audio, renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
        DesktopRuntimeHandle::for_tests(renderer, audio)
    }

    fn temp_sample_root(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cadence-runtime-{label}-{suffix}"))
    }

    fn write_test_wav(path: &Path) {
        let samples = [0i16, i16::MAX / 4];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        let data_len = (samples.len() * std::mem::size_of::<i16>()) as u32;
        let riff_len = 36 + data_len;
        bytes.extend_from_slice(&riff_len.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8_000u32.to_le_bytes());
        bytes.extend_from_slice(&(8_000u32 * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn control_thread_commit_transitions_status_to_playing() {
        let runtime = test_runtime_handle();
        runtime
            .load_sample(
                "bd".to_string(),
                SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.5)]),
                SampleLoadOptions::default(),
            )
            .unwrap();

        runtime
            .commit_score(sample_score("bd"), Time::new(2, 1))
            .unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn control_thread_replace_preserves_transport_progression() {
        let runtime = test_runtime_handle();
        runtime
            .load_sample(
                "bd".to_string(),
                SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.5)]),
                SampleLoadOptions::default(),
            )
            .unwrap();
        runtime
            .load_sample(
                "sn".to_string(),
                SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.25)]),
                SampleLoadOptions::default(),
            )
            .unwrap();

        runtime
            .commit_score(sample_score("bd"), Time::new(2, 1))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(40));
        let before = runtime.status().cycle_position;

        runtime
            .commit_score(sample_score("sn"), Time::new(2, 1))
            .unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
        assert!(runtime.status().cycle_position >= before);
    }

    #[test]
    fn control_thread_stop_resets_status_to_stopped() {
        let runtime = test_runtime_handle();
        runtime
            .load_sample(
                "bd".to_string(),
                SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.5)]),
                SampleLoadOptions::default(),
            )
            .unwrap();
        runtime
            .commit_score(sample_score("bd"), Time::new(2, 1))
            .unwrap();

        runtime.stop().unwrap();

        let status = runtime.status();
        assert_eq!(status.state, PlaybackState::Stopped);
        assert_eq!(status.cycle_position, Time::ZERO);
    }

    #[test]
    fn status_defaults_to_stopped_before_runtime_init() {
        let host = MusicRuntimeHost::default();
        let status = host.status();

        assert_eq!(status.state, PlaybackState::Stopped);
        assert_eq!(status.cycle_position, Time::ZERO);
    }

    #[test]
    fn set_sample_root_clears_catalog_and_loaded_sample_dedupe() {
        let mut host = MusicRuntimeHost::default();
        host.catalog = Some(PathSampleCatalog::default());
        host.loaded_samples.insert("bd".to_string());

        host.set_sample_root(Some(PathBuf::from("/tmp/kit-a")));

        assert!(host.catalog.is_none());
        assert!(host.loaded_samples.is_empty());
    }

    #[test]
    fn manifest_selector_matching_accepts_ids_sources_and_aliases() {
        let sample = CadenceSampleLoad {
            id: "bd".to_string(),
            source: crate::adapter::samples::DEFAULT_KICK_SELECTOR.to_string(),
            aliases: BTreeMap::from([
                ("kick".to_string(), "bd".to_string()),
                ("ghost".to_string(), "bd".to_string()),
            ]),
        };

        assert!(selector_matches_manifest_sample("bd", &sample));
        assert!(selector_matches_manifest_sample(
            crate::adapter::samples::DEFAULT_KICK_SELECTOR,
            &sample
        ));
        assert!(selector_matches_manifest_sample("kick", &sample));
        assert!(selector_matches_manifest_sample("ghost", &sample));
        assert!(!selector_matches_manifest_sample("sn", &sample));
    }

    #[test]
    fn preload_sample_loads_registers_manifest_aliases_from_catalog_selector() {
        let sample_root = temp_sample_root("manifest-selector");
        fs::create_dir_all(&sample_root).unwrap();
        let sample_path = sample_root.join("main kick.wav");
        write_test_wav(&sample_path);

        let mut host = MusicRuntimeHost::default();
        host.sample_root = Some(sample_root.clone());
        let sample_loads = vec![CadenceSampleLoad {
            id: "bd".to_string(),
            source: "/kick/".to_string(),
            aliases: BTreeMap::from([
                ("kick".to_string(), "bd".to_string()),
                ("ghost".to_string(), "bd".to_string()),
            ]),
        }];

        host.preload_sample_loads(
            sample_loads.as_slice(),
            &["kick".to_string(), "ghost".to_string()],
        )
        .expect("manifest sample loads should preload");

        assert!(host.loaded_samples.contains("kick"));
        assert!(host.loaded_samples.contains("ghost"));
        assert!(host.desktop.is_some());

        let _ = fs::remove_file(sample_path);
        let _ = fs::remove_dir(sample_root);
    }

    #[test]
    fn play_dedupes_sample_loads_across_repeated_commits() {
        let sample_root = temp_sample_root("dedupe");
        fs::create_dir_all(&sample_root).unwrap();
        let sample_path = sample_root.join("bd.wav");
        write_test_wav(&sample_path);

        let mut host = MusicRuntimeHost::default();
        host.sample_root = Some(sample_root.clone());
        host.desktop = Some(test_runtime_handle());
        let compiled = compiled_project("bd");

        host.play(&compiled, None).unwrap();
        assert_eq!(host.loaded_samples.len(), 1);

        host.play(&compiled, None).unwrap();
        assert_eq!(host.loaded_samples.len(), 1);

        let _ = fs::remove_file(sample_path);
        let _ = fs::remove_dir(sample_root);
    }

    #[test]
    fn play_commits_builtin_sine_without_sample_root() {
        let mut host = MusicRuntimeHost::default();
        host.desktop = Some(test_runtime_handle());
        let compiled =
            compiled_synth_project(cadence_core::infrastructure::voice::BuiltInSynthSource::Sine);

        host.play(&compiled, None)
            .expect("builtin sine synth should play without a sample root");

        assert!(host.loaded_samples.is_empty());
    }

    #[test]
    fn command_channel_failure_drops_runtime_bundle() {
        let mut host = MusicRuntimeHost::default();
        host.desktop = Some(test_runtime_handle());
        host.desktop.as_mut().unwrap().shutdown_for_tests();
        let compiled =
            compiled_synth_project(cadence_core::infrastructure::voice::BuiltInSynthSource::Sine);

        let error = host.play(&compiled, None).expect_err("runtime should fail");

        assert!(error.contains("desktop runtime"));
        assert!(host.desktop.is_none());
        assert!(host.loaded_samples.is_empty());
    }
}
