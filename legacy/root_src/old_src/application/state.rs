//! In-memory application state shared across all backend operations.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    adapter::cadence_core::MusicRuntimeHost,
    domain::{
        project::{CadenceGraphTarget, CadenceProjectDocument, TargetedGraphOpRecord},
        selection::SelectionState,
    },
};
use tessera::analysis::AnalysisCache;

/// Return a best-effort wall-clock timestamp used for diagnostics and timers.
pub fn now_epoch_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        let now = js_sys::Date::now();
        if now.is_finite() && now >= 0.0 {
            return now.round() as u64;
        }
        return 0;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeProgramState {
    #[default]
    None,
    Current,
    StaleLastGood,
}

#[derive(Debug, Default)]
/// Playback-specific state that lives alongside the current project.
pub struct RuntimeState {
    /// Monotonic revision of the last committed runtime program.
    pub rev: u64,
    /// Fingerprint of the last committed runtime program.
    pub last_program_fingerprint: String,
    /// Whether the loaded program reflects the current project or a stale last-good commit.
    pub program_state: RuntimeProgramState,
    /// Whether the UI/runtime currently considers playback active.
    pub playing: bool,
    /// Last runtime-level error surfaced to the UI.
    pub last_error: Option<String>,
    /// Start time for the current playback span, if any.
    pub play_started_at_ms: Option<u64>,
    /// Accumulated playback time across completed spans.
    pub play_elapsed_ms: u64,
}

impl RuntimeState {
    pub fn has_program(&self) -> bool {
        !matches!(self.program_state, RuntimeProgramState::None)
            && !self.last_program_fingerprint.is_empty()
    }

    pub fn clear_program(&mut self) {
        self.last_program_fingerprint.clear();
        self.program_state = RuntimeProgramState::None;
    }

    pub fn set_current_program(&mut self, fingerprint: String) {
        self.last_program_fingerprint = fingerprint;
        self.program_state = RuntimeProgramState::Current;
    }

    pub fn mark_program_stale(&mut self) {
        self.program_state = if self.last_program_fingerprint.is_empty() {
            RuntimeProgramState::None
        } else {
            RuntimeProgramState::StaleLastGood
        };
    }

    /// Toggle playback and keep the elapsed timer consistent across pauses/resumes.
    pub fn set_playing(&mut self, next_playing: bool) {
        if self.playing == next_playing {
            return;
        }

        let now = now_epoch_ms();
        if next_playing {
            self.play_started_at_ms = Some(now);
        } else if let Some(started_at) = self.play_started_at_ms.take() {
            self.play_elapsed_ms = self
                .play_elapsed_ms
                .saturating_add(now.saturating_sub(started_at));
        }
        self.playing = next_playing;
    }

    /// Return the total elapsed playback time including the active span.
    pub fn elapsed_ms(&self) -> u64 {
        if self.playing
            && let Some(started_at) = self.play_started_at_ms
        {
            return self
                .play_elapsed_ms
                .saturating_add(now_epoch_ms().saturating_sub(started_at));
        }

        self.play_elapsed_ms
    }

    /// Clear the playback timer without touching the current code revision.
    pub fn reset_playback_clock(&mut self) {
        self.play_started_at_ms = None;
        self.play_elapsed_ms = 0;
    }
}

#[derive(Debug, Clone)]
/// One entry shown in the mini console / diagnostics feed.
pub struct DiagnosticEntry {
    pub at_ms: u64,
    pub kind: String,
    pub message: String,
}

impl DiagnosticEntry {
    /// Build a new diagnostic stamped with the current time.
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            at_ms: now_epoch_ms(),
            kind: kind.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
/// Whole-app snapshot used for project-level undo/redo.
pub struct HistorySnapshot {
    pub project: CadenceProjectDocument,
    pub current_path: Option<String>,
    pub selection: SelectionState,
    pub dirty: bool,
    pub last_saved_snapshot_hash: Option<String>,
}

#[derive(Debug)]
/// Central mutable store for the Cadence backend.
///
/// The app intentionally keeps a single active project and a single runtime session in memory.
pub struct AppStore {
    /// Currently open project, if any.
    pub current_project: Option<CadenceProjectDocument>,
    /// Location of the active project on disk or in browser-local storage.
    pub current_path: Option<String>,
    /// Current editor selection/cursor state.
    pub selection: SelectionState,
    /// Playback/runtime bookkeeping.
    pub runtime: RuntimeState,
    /// Native Rust playback host backed by `cadence_core`.
    pub(crate) runtime_host: MusicRuntimeHost,
    /// Incremental analysis cache for the runtime graph.
    pub runtime_compile_cache: AnalysisCache,
    /// Incremental analysis caches for trick graphs, keyed by trick id.
    pub trick_compile_caches: BTreeMap<String, AnalysisCache>,
    /// Whether the in-memory project differs from the last saved fingerprint.
    pub dirty: bool,
    /// Hash of the last saved project payload.
    pub last_saved_snapshot_hash: Option<String>,
    /// Rolling diagnostic log exposed to the UI.
    pub diagnostics: Vec<DiagnosticEntry>,
    /// Mini console visibility remembered in backend state.
    pub mini_console_visible: bool,
    /// Devtools visibility remembered in backend state.
    pub devtools_visible: bool,
    /// Whole-project undo stack.
    pub history_past: Vec<HistorySnapshot>,
    /// Whole-project redo stack.
    pub history_future: Vec<HistorySnapshot>,
    /// Graph-only undo stack for granular editor mutations.
    pub graph_history_past: Vec<TargetedGraphOpRecord>,
    /// Graph-only redo stack for granular editor mutations.
    pub graph_history_future: Vec<TargetedGraphOpRecord>,
    /// Upper bound applied to both snapshot and graph histories.
    pub history_limit: usize,
}

impl AppStore {
    /// Append a diagnostic entry and trim the feed to a fixed rolling window.
    pub fn push_diagnostic(&mut self, kind: impl Into<String>, message: impl Into<String>) {
        self.diagnostics.push(DiagnosticEntry::new(kind, message));
        if self.diagnostics.len() > 500 {
            let drain = self.diagnostics.len() - 500;
            self.diagnostics.drain(0..drain);
        }
    }

    /// Return the incremental compile cache for the requested graph target.
    pub fn compile_cache_for_target_mut(
        &mut self,
        target: &CadenceGraphTarget,
    ) -> &mut AnalysisCache {
        match target {
            CadenceGraphTarget::Runtime => &mut self.runtime_compile_cache,
            CadenceGraphTarget::Trick { trick_id } => self
                .trick_compile_caches
                .entry(trick_id.clone())
                .or_default(),
        }
    }

    /// Clear all incremental compile caches.
    pub fn clear_compile_caches(&mut self) {
        self.runtime_compile_cache.clear();
        self.trick_compile_caches.clear();
    }

    /// Clear the cache associated with one graph target.
    pub fn clear_compile_cache_for_target(&mut self, target: &CadenceGraphTarget) {
        match target {
            CadenceGraphTarget::Runtime => self.runtime_compile_cache.clear(),
            CadenceGraphTarget::Trick { trick_id } => {
                self.trick_compile_caches.remove(trick_id);
            }
        }
    }

    /// Reset transient state when swapping to a new or different project.
    ///
    /// Call sites are still responsible for invoking [`clear_graph_history`] separately,
    /// since that helper takes `&mut AppStore` and cannot be called from `&mut self`.
    pub fn reset_on_project_swap(&mut self) {
        self.clear_compile_caches();
        self.current_path = None;
        self.selection = Default::default();
        let _ = self.runtime_host.stop();
        self.runtime.clear_program();
        self.runtime.last_error = None;
        self.runtime.reset_playback_clock();
        self.runtime.set_playing(false);
    }

    /// Drop trick caches whose ids no longer exist.
    pub fn retain_trick_compile_cache_ids<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = String>,
    {
        let keep = ids.into_iter().collect::<BTreeSet<_>>();
        self.trick_compile_caches
            .retain(|trick_id, _| keep.contains(trick_id));
    }
}

impl Default for AppStore {
    fn default() -> Self {
        Self {
            current_project: None,
            current_path: None,
            selection: SelectionState::default(),
            runtime: RuntimeState::default(),
            runtime_host: MusicRuntimeHost::default(),
            runtime_compile_cache: AnalysisCache::new(),
            trick_compile_caches: BTreeMap::new(),
            dirty: false,
            last_saved_snapshot_hash: None,
            diagnostics: Vec::new(),
            mini_console_visible: false,
            devtools_visible: false,
            history_past: Vec::new(),
            history_future: Vec::new(),
            graph_history_past: Vec::new(),
            graph_history_future: Vec::new(),
            history_limit: 100,
        }
    }
}
