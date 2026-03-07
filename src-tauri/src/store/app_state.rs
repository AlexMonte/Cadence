use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::selection::SelectionState;
use crate::model::{CadenceProjectDocument, TargetedGraphOpRecord};

pub fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Default)]
pub struct RuntimeState {
    pub rev: u64,
    pub last_code: String,
    pub playing: bool,
    pub last_error: Option<String>,
    pub play_started_at_ms: Option<u64>,
    pub play_elapsed_ms: u64,
}

impl RuntimeState {
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

    pub fn elapsed_ms(&self) -> u64 {
        if self.playing {
            if let Some(started_at) = self.play_started_at_ms {
                return self
                    .play_elapsed_ms
                    .saturating_add(now_epoch_ms().saturating_sub(started_at));
            }
        }

        self.play_elapsed_ms
    }

    pub fn reset_playback_clock(&mut self) {
        self.play_started_at_ms = None;
        self.play_elapsed_ms = 0;
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub at_ms: u64,
    pub kind: String,
    pub message: String,
}

impl DiagnosticEntry {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            at_ms: now_epoch_ms(),
            kind: kind.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistorySnapshot {
    pub project: CadenceProjectDocument,
    pub current_path: Option<PathBuf>,
    pub selection: SelectionState,
    pub dirty: bool,
    pub last_saved_snapshot_hash: Option<String>,
}

#[derive(Debug)]
pub struct AppStore {
    pub current_project: Option<CadenceProjectDocument>,
    pub current_path: Option<PathBuf>,
    pub selection: SelectionState,
    pub runtime: RuntimeState,
    pub dirty: bool,
    pub last_saved_snapshot_hash: Option<String>,
    pub diagnostics: Vec<DiagnosticEntry>,
    pub mini_console_visible: bool,
    pub devtools_visible: bool,
    pub history_past: Vec<HistorySnapshot>,
    pub history_future: Vec<HistorySnapshot>,
    pub graph_history_past: Vec<TargetedGraphOpRecord>,
    pub graph_history_future: Vec<TargetedGraphOpRecord>,
    pub history_limit: usize,
}

impl AppStore {
    pub fn push_diagnostic(&mut self, kind: impl Into<String>, message: impl Into<String>) {
        self.diagnostics.push(DiagnosticEntry::new(kind, message));
        if self.diagnostics.len() > 500 {
            let drain = self.diagnostics.len() - 500;
            self.diagnostics.drain(0..drain);
        }
    }
}

impl Default for AppStore {
    fn default() -> Self {
        Self {
            current_project: None,
            current_path: None,
            selection: SelectionState::default(),
            runtime: RuntimeState::default(),
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
