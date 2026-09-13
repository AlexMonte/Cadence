//! Native recovery snapshots use the complete project format and owned media.
//! A single worker writes snapshots; the UI never serializes or copies audio.

use super::{PersistenceError, load_project, save_project};
use crate::application::session::MusaicProject;
use bevy::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn recovery_directory() -> Option<PathBuf> {
    // Allows an isolated native review or portable host to own its checkpoints.
    if let Some(directory) = std::env::var_os("MUSAIC_RECOVERY_DIR") {
        return Some(PathBuf::from(directory));
    }
    directories::ProjectDirs::from("", "", "musaic").map(|dirs| dirs.data_dir().join("recovery"))
}

#[derive(Debug)]
pub struct RecoveryEntry {
    pub path: PathBuf,
    pub name: String,
    pub modified: SystemTime,
}

/// List only snapshots produced by this adapter. Normal project files are never
/// overwritten or adopted as recovery destinations.
pub fn list_recoveries_in(directory: &Path) -> Vec<RecoveryEntry> {
    #[derive(serde::Deserialize)]
    struct Header {
        metadata: crate::domain::project::ProjectMetadata,
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_dir()
                || !entry.file_name().to_string_lossy().starts_with("session-")
            {
                return None;
            }
            let path = entry.path().join("recovery.musaic.json");
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
                return None;
            }
            Some((metadata.modified().ok()?, path))
        })
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    entries
        .into_iter()
        .take(8)
        .filter_map(|(modified, path)| {
            let header: Header = serde_json::from_reader(std::fs::File::open(&path).ok()?).ok()?;
            Some(RecoveryEntry {
                path,
                name: header.metadata.display_name,
                modified,
            })
        })
        .collect()
}

pub fn list_recoveries() -> Vec<RecoveryEntry> {
    recovery_directory()
        .map(|path| list_recoveries_in(&path))
        .unwrap_or_default()
}

pub fn save_recovery(path: &Path, project: &MusaicProject) -> Result<(), PersistenceError> {
    save_project(path, project)
}

/// A recovered project is an unsaved document, with decoded owned media ready
/// for Save As. Saving it cannot accidentally overwrite its recovery snapshot.
pub fn load_recovery(path: &Path) -> Result<MusaicProject, PersistenceError> {
    let mut project = load_project(path)?;
    project.metadata.file_path = None;
    project.metadata.dirty = true;
    Ok(project)
}

const MAX_OUTGOING_PROJECTS: usize = 8;
const DEBOUNCE: Duration = Duration::from_secs(5);

/// Captures outgoing authoring before the command dispatcher replaces it.
#[derive(Message)]
pub struct RecoveryProjectChanged {
    previous_generation: u64,
    generation: u64,
    outgoing: MusaicProject,
}

/// Reservation authority shared with project replacement. When recovery is not
/// registered, headless command handling still emits messages without disk work.
#[derive(Resource, Default)]
pub struct RecoveryLifecycle {
    enabled: bool,
    generation: u64,
    outgoing: BTreeSet<u64>,
}

impl RecoveryLifecycle {
    pub(crate) fn prepare_transition(
        &mut self,
        outgoing: &MusaicProject,
    ) -> Result<RecoveryProjectChanged, &'static str> {
        if self.enabled && outgoing.metadata.dirty && self.outgoing.len() >= MAX_OUTGOING_PROJECTS {
            return Err(
                "Recovery is still saving earlier projects. Wait for recovery to finish before replacing this unsaved project; your current edits are retained.",
            );
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or("Recovery generation limit reached")?;
        let previous_generation = self.generation;
        if self.enabled && outgoing.metadata.dirty {
            self.outgoing.insert(previous_generation);
        }
        self.generation = generation;
        Ok(RecoveryProjectChanged {
            previous_generation,
            generation,
            outgoing: outgoing.clone(),
        })
    }
}

#[derive(Clone)]
struct Snapshot {
    generation: u64,
    serial: u64,
    project: Arc<MusaicProject>,
    path: Option<PathBuf>,
    ready_at: Instant,
    failures: u32,
}

struct Completion {
    generation: u64,
    serial: u64,
    result: Result<(), String>,
}

#[derive(Resource)]
struct RecoveryWriter {
    root: Option<PathBuf>,
    session: String,
    generation: u64,
    next_serial: u64,
    queued: BTreeMap<u64, Snapshot>,
    in_flight: Option<Snapshot>,
    errors: BTreeMap<u64, String>,
    last_saved: Option<SystemTime>,
    sender: mpsc::Sender<Completion>,
    receiver: Mutex<mpsc::Receiver<Completion>>,
}

impl Default for RecoveryWriter {
    fn default() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self::new(
            recovery_directory(),
            format!("session-{unique}-{}", std::process::id()),
        )
    }
}

impl RecoveryWriter {
    fn new(root: Option<PathBuf>, session: String) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            root,
            session,
            generation: 0,
            next_serial: 0,
            queued: BTreeMap::new(),
            in_flight: None,
            errors: BTreeMap::new(),
            last_saved: None,
            sender,
            receiver: Mutex::new(receiver),
        }
    }

    fn capture(&mut self, generation: u64, project: MusaicProject, ready_at: Instant) {
        self.next_serial += 1;
        let existing = self.queued.get(&generation);
        let failures = existing.map_or(0, |snapshot| snapshot.failures);
        let ready_at = existing.map_or(ready_at, |snapshot| {
            if snapshot.failures == 0 {
                snapshot.ready_at.min(ready_at)
            } else {
                // New edits replace retry content, not its backoff deadline.
                // Otherwise continuous editing could postpone recovery forever.
                snapshot.ready_at
            }
        });
        let path = self.root.as_ref().map(|root| {
            root.join(format!("{}-{generation}", self.session))
                .join("recovery.musaic.json")
        });
        self.queued.insert(
            generation,
            Snapshot {
                generation,
                serial: self.next_serial,
                project: Arc::new(project),
                path,
                ready_at,
                failures,
            },
        );
    }

    fn replace_project(&mut self, change: RecoveryProjectChanged, now: Instant) {
        if change.outgoing.metadata.dirty {
            // Supersede any debounced/queued older version. An older in-flight
            // version may finish first, but cannot consume this latest snapshot.
            self.capture(change.previous_generation, change.outgoing, now);
        } else {
            self.queued.remove(&change.previous_generation);
            self.errors.remove(&change.previous_generation);
        }
        self.generation = change.generation;
        self.last_saved = None;
    }

    fn next_job(&mut self, now: Instant) -> Option<Snapshot> {
        if self.in_flight.is_some() {
            return None;
        }
        let generation = self
            .queued
            .iter()
            .find(|(_, snapshot)| snapshot.ready_at <= now)
            .map(|(generation, _)| *generation)?;
        let snapshot = self.queued.remove(&generation)?;
        self.in_flight = Some(snapshot.clone());
        Some(snapshot)
    }

    fn complete(
        &mut self,
        completion: Completion,
        now: Instant,
        lifecycle: &mut RecoveryLifecycle,
    ) {
        if !self.in_flight.as_ref().is_some_and(|snapshot| {
            snapshot.generation == completion.generation && snapshot.serial == completion.serial
        }) {
            return;
        }
        let mut snapshot = self.in_flight.take().expect("matched completion");
        match completion.result {
            Ok(()) => {
                self.errors.remove(&snapshot.generation);
                if snapshot.generation == self.generation {
                    self.last_saved = Some(SystemTime::now());
                }
                if snapshot.generation != self.generation
                    && !self.queued.contains_key(&snapshot.generation)
                {
                    lifecycle.outgoing.remove(&snapshot.generation);
                }
            }
            Err(error) => {
                self.errors.insert(
                    snapshot.generation,
                    format!("{}: {error}", snapshot.project.metadata.display_name),
                );
                snapshot.failures = snapshot.failures.saturating_add(1);
                let delay = Duration::from_secs(
                    5_u64
                        .saturating_mul(1_u64 << snapshot.failures.saturating_sub(1).min(4))
                        .min(60),
                );
                snapshot.ready_at = now + delay;
                match self.queued.get_mut(&snapshot.generation) {
                    Some(newer) => {
                        newer.failures = snapshot.failures;
                        newer.ready_at = newer.ready_at.max(snapshot.ready_at);
                    }
                    None => {
                        self.queued.insert(snapshot.generation, snapshot);
                    }
                }
            }
        }
    }

    fn status(&self) -> RecoveryStatus {
        RecoveryStatus {
            last_saved: self.last_saved,
            error: self.errors.values().next().cloned(),
            saving: self.in_flight.is_some(),
            pending_snapshots: self.queued.len(),
        }
    }
}

/// Status belongs to the current project; errors also name outgoing projects
/// still waiting for a successful recovery write.
#[derive(Resource, Default)]
pub struct RecoveryStatus {
    pub last_saved: Option<SystemTime>,
    pub error: Option<String>,
    pub saving: bool,
    pub pending_snapshots: usize,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<RecoveryWriter>()
        .init_resource::<RecoveryStatus>()
        .init_resource::<RecoveryLifecycle>()
        .add_message::<RecoveryProjectChanged>()
        .add_systems(
            Update,
            autosave.after(crate::infrastructure::app::MusaicSet::Commands),
        );
    app.world_mut().resource_mut::<RecoveryLifecycle>().enabled = true;
}

fn write_snapshot(snapshot: &Snapshot) -> Completion {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let path = snapshot
            .path
            .as_ref()
            .ok_or_else(|| "Recovery storage is unavailable".to_owned())?;
        save_recovery(path, &snapshot.project).map_err(|error| error.to_string())
    }))
    .unwrap_or_else(|_| {
        Err("Recovery writer stopped unexpectedly; the snapshot will be retried".into())
    });
    Completion {
        generation: snapshot.generation,
        serial: snapshot.serial,
        result,
    }
}

fn autosave(
    project: Res<MusaicProject>,
    mut changes: MessageReader<RecoveryProjectChanged>,
    mut writer: ResMut<RecoveryWriter>,
    mut lifecycle: ResMut<RecoveryLifecycle>,
    mut status: ResMut<RecoveryStatus>,
) {
    let now = Instant::now();
    // Apply transitions before completions so an old A save cannot mark B saved
    // or discard A's final edits captured by the same command batch.
    for change in changes.read() {
        writer.replace_project(
            RecoveryProjectChanged {
                previous_generation: change.previous_generation,
                generation: change.generation,
                outgoing: change.outgoing.clone(),
            },
            now,
        );
    }
    let finished = writer
        .receiver
        .lock()
        .ok()
        .and_then(|receiver| receiver.try_recv().ok());
    if let Some(completion) = finished {
        writer.complete(completion, now, &mut lifecycle);
    }
    if project.is_changed() {
        let generation = writer.generation;
        if project.metadata.dirty {
            writer.capture(generation, project.clone(), now + DEBOUNCE);
        } else {
            writer.queued.remove(&generation);
            writer.errors.remove(&generation);
        }
    }
    if let Some(snapshot) = writer.next_job(now) {
        let sender = writer.sender.clone();
        let generation = snapshot.generation;
        let serial = snapshot.serial;
        if let Err(error) = std::thread::Builder::new()
            .name("musaic-recovery".into())
            .spawn(move || {
                let _ = sender.send(write_snapshot(&snapshot));
            })
        {
            writer.complete(
                Completion {
                    generation,
                    serial,
                    result: Err(error.to_string()),
                },
                now,
                &mut lifecycle,
            );
        }
    }
    *status = writer.status();
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "musaic-recovery-state-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn project(name: &str, bpm: f64) -> MusaicProject {
        let mut project = MusaicProject::demo();
        project.metadata.display_name = name.into();
        project.metadata.dirty = true;
        project.document.playback.bpm = bpm;
        project
    }
    fn lifecycle() -> RecoveryLifecycle {
        RecoveryLifecycle {
            enabled: true,
            ..default()
        }
    }
    fn complete_ok(
        writer: &mut RecoveryWriter,
        job: &Snapshot,
        now: Instant,
        lifecycle: &mut RecoveryLifecycle,
    ) {
        writer.complete(
            Completion {
                generation: job.generation,
                serial: job.serial,
                result: Ok(()),
            },
            now,
            lifecycle,
        );
    }
    fn wav() -> Vec<u8> {
        let samples = [1200_i16, -2400, 3600, -4800];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&44_u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8_u32.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn recovering_then_switching_preserves_latest_a_and_b_in_separate_complete_snapshots() {
        let directory = TestDirectory::new();
        let now = Instant::now();
        let mut writer = RecoveryWriter::new(Some(directory.0.clone()), "session-fixture".into());
        let mut lifecycle = lifecycle();
        let mut a = project("A before final edit", 120.0);
        let id =
            super::super::import_wav_bytes(&mut a, "owned.wav", wav(), Default::default()).unwrap();
        let expected_frames = a.samples.decoded(id).unwrap().frames().to_vec();
        writer.capture(0, a, now);
        let initial = writer.next_job(now).unwrap();
        writer.complete(write_snapshot(&initial), now, &mut lifecycle);
        let a_path = initial.path.unwrap();
        let mut a = load_recovery(&a_path).unwrap();
        a.metadata.display_name = "A latest recovered edit".into();
        a.document.playback.bpm = 137.0;
        writer.capture(0, a.clone(), now + DEBOUNCE);
        let b = project("B latest edit", 91.0);
        writer.replace_project(lifecycle.prepare_transition(&a).unwrap(), now);
        writer.capture(1, b, now + DEBOUNCE);
        let outgoing = writer
            .next_job(now)
            .expect("switch must flush A without waiting five seconds");
        assert_eq!(outgoing.path.as_ref(), Some(&a_path));
        writer.complete(write_snapshot(&outgoing), now, &mut lifecycle);
        assert!(
            writer.status().last_saved.is_none(),
            "A's completion cannot mark B saved"
        );
        let current = writer.next_job(now + DEBOUNCE).unwrap();
        assert_ne!(current.path.as_ref(), Some(&a_path));
        writer.complete(write_snapshot(&current), now + DEBOUNCE, &mut lifecycle);
        let entries = list_recoveries_in(&directory.0);
        assert_eq!(entries.len(), 2);
        let recovered_a = load_recovery(&a_path).unwrap();
        let recovered_b = load_recovery(current.path.as_ref().unwrap()).unwrap();
        assert_eq!(recovered_a.metadata.display_name, "A latest recovered edit");
        assert_eq!(recovered_a.document.playback.bpm, 137.0);
        assert_eq!(
            recovered_a.samples.decoded(id).unwrap().frames(),
            expected_frames
        );
        assert_eq!(recovered_b.metadata.display_name, "B latest edit");
        assert_eq!(recovered_b.document.playback.bpm, 91.0);
        assert!(recovered_a.metadata.dirty && recovered_b.metadata.dirty);
        assert!(
            recovered_a.metadata.file_path.is_none() && recovered_b.metadata.file_path.is_none()
        );
        assert!(lifecycle.outgoing.is_empty());
    }

    #[test]
    fn replacement_while_busy_keeps_latest_outgoing_and_ignores_stale_completions() {
        let now = Instant::now();
        let mut writer = RecoveryWriter::new(None, "session-fixture".into());
        let mut lifecycle = lifecycle();
        writer.capture(0, project("A old", 120.0), now);
        let old = writer.next_job(now).unwrap();
        let latest_a = project("A latest", 145.0);
        for _ in 0..100 {
            writer.capture(0, latest_a.clone(), now + DEBOUNCE);
        }
        assert_eq!(
            writer.queued.len(),
            1,
            "edits coalesce while one worker is active"
        );
        writer.replace_project(lifecycle.prepare_transition(&latest_a).unwrap(), now);
        writer.capture(1, project("B", 100.0), now);
        assert!(
            writer.next_job(now).is_none(),
            "never start a second worker"
        );
        writer.complete(
            Completion {
                generation: 99,
                serial: old.serial,
                result: Ok(()),
            },
            now,
            &mut lifecycle,
        );
        assert!(writer.in_flight.is_some());
        complete_ok(&mut writer, &old, now, &mut lifecycle);
        assert!(
            lifecycle.outgoing.contains(&0),
            "an older A snapshot cannot release the latest obligation"
        );
        assert!(writer.status().last_saved.is_none());
        let latest = writer.next_job(now).unwrap();
        assert_eq!(latest.project.document.playback.bpm, 145.0);
        assert!(latest.serial > old.serial);
        complete_ok(&mut writer, &old, now, &mut lifecycle);
        assert_eq!(writer.in_flight.as_ref().unwrap().serial, latest.serial);
        complete_ok(&mut writer, &latest, now, &mut lifecycle);
        assert!(!lifecycle.outgoing.contains(&0));
        let b = writer.next_job(now).unwrap();
        assert_eq!(b.generation, 1);
        complete_ok(&mut writer, &b, now, &mut lifecycle);
        assert!(writer.status().last_saved.is_some());
    }

    #[test]
    fn failures_retry_without_edits_with_capped_backoff_and_keep_newer_snapshots() {
        let start = Instant::now();
        let mut now = start;
        let mut writer = RecoveryWriter::new(None, "session-fixture".into());
        let mut lifecycle = lifecycle();
        writer.capture(0, project("A", 120.0), now);
        for attempt in 0..10 {
            let job = writer.next_job(now).unwrap();
            if attempt == 0 {
                writer.capture(0, project("A latest", 155.0), now);
            }
            writer.complete(
                Completion {
                    generation: job.generation,
                    serial: job.serial,
                    result: Err("disk temporarily full".into()),
                },
                now,
                &mut lifecycle,
            );
            assert!(
                writer
                    .status()
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("disk temporarily full")
            );
            let retry_at = writer.queued[&0].ready_at;
            let expected = Duration::from_secs((5_u64 << attempt.min(4)).min(60));
            assert_eq!(retry_at.duration_since(now), expected);
            if attempt == 0 {
                writer.capture(0, project("A latest", 155.0), retry_at + DEBOUNCE);
                assert_eq!(
                    writer.queued[&0].ready_at, retry_at,
                    "continued edits cannot postpone an already scheduled retry"
                );
            }
            assert!(
                writer
                    .next_job(retry_at - Duration::from_millis(1))
                    .is_none()
            );
            assert_eq!(writer.queued[&0].project.document.playback.bpm, 155.0);
            now = retry_at;
        }
        let success = writer.next_job(now).unwrap();
        complete_ok(&mut writer, &success, now, &mut lifecycle);
        assert!(writer.queued.is_empty() && writer.status().error.is_none());
        assert!(writer.status().last_saved.is_some());
    }

    #[test]
    fn a_failed_outgoing_snapshot_does_not_block_b_and_stays_reserved_until_saved() {
        let now = Instant::now();
        let mut writer = RecoveryWriter::new(None, "session-fixture".into());
        let mut lifecycle = lifecycle();
        writer.replace_project(
            lifecycle.prepare_transition(&project("A", 120.0)).unwrap(),
            now,
        );
        writer.capture(1, project("B", 100.0), now);
        let a = writer.next_job(now).unwrap();
        writer.complete(
            Completion {
                generation: a.generation,
                serial: a.serial,
                result: Err("A failed".into()),
            },
            now,
            &mut lifecycle,
        );
        let b = writer.next_job(now).unwrap();
        assert_eq!(b.generation, 1);
        complete_ok(&mut writer, &b, now, &mut lifecycle);
        assert!(writer.status().error.as_ref().unwrap().contains("A failed"));
        assert!(lifecycle.outgoing.contains(&0));
        let retry = writer.next_job(now + DEBOUNCE).unwrap();
        complete_ok(&mut writer, &retry, now + DEBOUNCE, &mut lifecycle);
        assert!(lifecycle.outgoing.is_empty());
        assert!(writer.status().error.is_none());
    }

    fn command_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::infrastructure::app::AppState>()
            .init_state::<crate::infrastructure::app::TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((
                crate::application::editor::EditorPlugin,
                crate::application::pipeline::PlaybackPlugin,
            ));
        app.insert_state(crate::infrastructure::app::AppState::Editor);
        app
    }

    #[test]
    fn command_bus_captures_outgoing_project_without_a_persistence_plugin() {
        use crate::application::command::{EditorCommand, EditorCommandBus};
        let mut app = command_app();
        app.insert_resource(project("A latest", 139.0));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::AdoptProject {
                project: project("B", 101.0),
                path: None,
            }));
        app.update();
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .playback
                .bpm,
            101.0
        );
        let messages: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<RecoveryProjectChanged>>()
            .drain()
            .collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].outgoing.metadata.display_name, "A latest");
        assert_eq!(messages[0].outgoing.document.playback.bpm, 139.0);
        assert!(!app.world().contains_resource::<RecoveryWriter>());
    }

    #[test]
    fn full_recovery_backlog_rejects_replacement_before_current_edits_change() {
        use crate::application::command::{EditorCommand, EditorCommandBus};
        let mut app = command_app();
        app.insert_resource(project("A retained", 143.0));
        let mut lifecycle = lifecycle();
        for _ in 0..MAX_OUTGOING_PROJECTS {
            lifecycle
                .prepare_transition(&project("older", 120.0))
                .unwrap();
        }
        app.insert_resource(lifecycle);
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::AdoptProject {
                project: project("B", 101.0),
                path: None,
            }));
        app.update();
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .metadata
                .display_name,
            "A retained"
        );
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .playback
                .bpm,
            143.0
        );
        assert!(
            app.world()
                .resource::<Messages<RecoveryProjectChanged>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<crate::infrastructure::diagnostics::DiagnosticStore>()
                .items
                .iter()
                .any(|diagnostic| format!("{diagnostic:?}").contains("current edits are retained"))
        );
    }
}
