//! Editor transport: the single authoritative clock plus transport command handling.
//!
//! While stopped, the editor owns the clock (seek, bpm edits). While playing,
//! Cadence is the clock master and the playback pipeline copies its cycle
//! position back into [`TransportClock`].

use bevy::prelude::*;
use cadence::domain::rational::Time as CycleTime;
use cadence::prelude::Span;

use crate::application::command::EditorCommand;
use crate::application::pipeline::runtime::RuntimeState;
use crate::application::session::MusaicProject;
use crate::infrastructure::app::TransportMode;

/// Authoritative editor transport clock shared by preview, timeline, and playback sync.
#[derive(Resource, Debug, Clone)]
pub struct TransportClock {
    pub position: CycleTime,
    pub bpm: f64,
    pub beats_per_cycle: u32,
    pub loop_region: Option<Span<CycleTime>>,
}

impl Default for TransportClock {
    fn default() -> Self {
        Self {
            position: CycleTime::ZERO,
            bpm: 120.0,
            beats_per_cycle: 4,
            loop_region: None,
        }
    }
}

impl TransportClock {
    pub fn seek(&mut self, position: CycleTime) {
        self.position = position;
    }

    pub fn set_bpm(&mut self, bpm: f64) {
        self.bpm = bpm.max(1.0);
    }
}

/// Keeps [`TransportClock`] aligned with document playback defaults.
pub fn sync_clock_from_document(
    project: Res<'_, MusaicProject>,
    mut clock: ResMut<'_, TransportClock>,
) {
    clock.set_bpm(project.document.playback.bpm);
    clock.beats_per_cycle = project.document.playback.beats_per_cycle;
}

pub fn handle_transport_command(
    command: &EditorCommand,
    transport_mode: &mut NextState<TransportMode>,
    clock: &mut TransportClock,
    runtime: &mut RuntimeState,
) -> bool {
    match command {
        EditorCommand::TransportPlay => {
            transport_mode.set(TransportMode::Playing);
            true
        }
        EditorCommand::TransportStop => {
            transport_mode.set(TransportMode::Stopped);
            clock.seek(CycleTime::ZERO);
            runtime.transport_request =
                Some(crate::application::pipeline::runtime::TransportRequest::Stop);
            runtime.mark_transport_changed();
            true
        }
        EditorCommand::TransportPause => {
            transport_mode.set(TransportMode::Paused);
            true
        }
        EditorCommand::TransportPanic => {
            transport_mode.set(TransportMode::Stopped);
            clock.seek(CycleTime::ZERO);
            runtime.transport_request =
                Some(crate::application::pipeline::runtime::TransportRequest::Panic);
            runtime.mark_transport_changed();
            true
        }
        EditorCommand::TransportToggle => false,
        EditorCommand::TransportSeek { position_cycles } => {
            if !position_cycles.is_finite()
                || *position_cycles < 0.0
                || *position_cycles > 1_000_000.0
            {
                return true;
            }
            let position = cycle_time_from_f64(*position_cycles);
            clock.seek(position);
            runtime.transport_request =
                Some(crate::application::pipeline::runtime::TransportRequest::Seek(position));
            runtime.mark_transport_changed();
            true
        }
        // SetBpm goes through execute_command (document + invalidation);
        // the bus then syncs TransportClock from the accepted result.
        _ => false,
    }
}

pub fn toggle_transport_mode(current: TransportMode) -> TransportMode {
    match current {
        TransportMode::Playing => TransportMode::Paused,
        TransportMode::Stopped | TransportMode::Paused => TransportMode::Playing,
    }
}

pub fn handle_transport_toggle(
    current: TransportMode,
    transport_mode: &mut NextState<TransportMode>,
) {
    transport_mode.set(toggle_transport_mode(current));
}

fn cycle_time_from_f64(cycles: f64) -> CycleTime {
    let scaled = (cycles * 1_000_000.0).round() as i64;
    CycleTime::new(scaled, 1_000_000)
}
