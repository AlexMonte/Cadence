//! Pure command execution — applies an [`EditorCommand`](super::types::EditorCommand)
//! or [`EditorInverse`](super::types::EditorInverse) to document and editor state.

use super::types::{EditorCommand, EditorInverse};
use crate::application::editor::transaction::{
    self, EditorTransactionResult, Invalidation, TimelineSourceResolver,
};
use crate::application::editor::workspace::FocusTarget;
use crate::application::editor::{EditorAttention, SelectionState};
use crate::domain::DomainError;
use crate::domain::document::{DocumentQueries, MusaicDocument, apply_document_patch};

pub fn execute_command<R>(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    resolver: &R,
    command: &EditorCommand,
) -> Result<EditorTransactionResult, DomainError>
where
    R: TimelineSourceResolver,
{
    let result = match command {
        // ── Ephemeral attention/selection ──
        EditorCommand::Focus { target } => {
            let queries = DocumentQueries::new(document);
            match attention.focus(&queries, target.clone()) {
                Ok(()) => EditorTransactionResult::accepted(Invalidation::none()),
                Err(error) => EditorTransactionResult::rejected(error.to_string()),
            }
        }
        EditorCommand::SelectNode { node, mode } => {
            let queries = DocumentQueries::new(document);
            selection.select(node.clone(), *mode);
            match attention.focus(&queries, FocusTarget::Tile { node: node.clone() }) {
                Ok(()) => EditorTransactionResult::accepted(Invalidation::none()),
                Err(error) => EditorTransactionResult::rejected(error.to_string()),
            }
        }
        EditorCommand::ClearSelection => {
            let queries = DocumentQueries::new(document);
            selection.clear();
            attention
                .focus(&queries, FocusTarget::None)
                .expect("FocusTarget::None is always valid");
            EditorTransactionResult::accepted(Invalidation::none())
        }
        // ── Durable document mutations ──
        EditorCommand::SetAtomValue { node, value } => {
            transaction::set_atom_value(document, node, value.clone())
        }
        EditorCommand::MoveModifierGroup { owner, step } => {
            transaction::move_modifier_group(document, owner, *step)
        }
        EditorCommand::PlaceTile { target, tile } => {
            transaction::place_tile(document, attention, selection, *target, tile.clone())
        }
        EditorCommand::EnterContainer { container } => {
            transaction::enter_container(document, attention, selection, container)
        }
        EditorCommand::NavigateToSurface { surface } => {
            transaction::navigate_to_surface(document, attention, selection, *surface)
        }
        EditorCommand::ConnectTiles { from, to } => {
            transaction::connect_tiles(document, attention, selection, from.clone(), to.clone())
        }
        EditorCommand::BindOutputSide { node, side } => {
            transaction::bind_output_side(document, attention, node, *side)
        }
        EditorCommand::CycleConnection { from, to } => {
            transaction::cycle_connection(document, attention, from, to)
        }
        EditorCommand::DeleteSelection => {
            transaction::delete_selection(document, attention, selection)
        }
        EditorCommand::JumpToTimelineSource { event } => {
            transaction::jump_to_timeline_source(document, attention, selection, resolver, *event)
        }
        EditorCommand::SetBpm { bpm } => {
            set_tempo(document, *bpm, document.playback.beats_per_cycle)
        }
        EditorCommand::SetBeatsPerCycle { beats } => {
            set_tempo(document, document.playback.bpm, *beats)
        }

        // Handled by the dispatcher (session/tool/panel/project/view), never here.
        EditorCommand::SoundLibrary(_)
        | EditorCommand::SetSampleBank { .. }
        | EditorCommand::EditTiles(_)
        | EditorCommand::BeginSampleOptionsEdit { .. }
        | EditorCommand::EndSampleOptionsEdit
        | EditorCommand::SetSampleOptions { .. }
        | EditorCommand::RelinkSample { .. }
        | EditorCommand::ChooseAudioExport { .. }
        | EditorCommand::RevealProjectFile
        | EditorCommand::RevealLastExport
        | EditorCommand::ExportAudio { .. }
        | EditorCommand::AuditionSound { .. }
        | EditorCommand::BeginSoundEdit { .. }
        | EditorCommand::EndSoundEdit
        | EditorCommand::SetSound { .. }
        | EditorCommand::ImportSample { .. }
        | EditorCommand::ArmPlacementTool { .. }
        | EditorCommand::BeginAtomEdit { .. }
        | EditorCommand::EndAtomEdit
        | EditorCommand::CancelPlacement
        | EditorCommand::StartConnection { .. }
        | EditorCommand::AbortConnection
        | EditorCommand::ToggleDrawer
        | EditorCommand::SetDrawerOpen { .. }
        | EditorCommand::ToggleMinimap
        | EditorCommand::EnterTimelineMode
        | EditorCommand::EnterCompose
        | EditorCommand::SetViewSettings { .. }
        | EditorCommand::NewProject
        | EditorCommand::OpenProject { .. }
        | EditorCommand::AdoptProject { .. }
        | EditorCommand::SaveProject
        | EditorCommand::SaveProjectAs { .. }
        | EditorCommand::Undo
        | EditorCommand::Redo
        | EditorCommand::TransportPlay
        | EditorCommand::TransportPause
        | EditorCommand::TransportPanic
        | EditorCommand::TransportStop
        | EditorCommand::TransportToggle
        | EditorCommand::PreviewCycle { .. }
        | EditorCommand::TransportSeek { .. } => {
            return Err(DomainError::Message(format!(
                "command {command:?} must be handled by the dispatcher"
            )));
        }
    };

    Ok(result)
}

/// Applies a stored undo inverse. Public only to the command/history layer.
pub fn execute_inverse(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    inverse: &EditorInverse,
) -> Result<EditorTransactionResult, DomainError> {
    let result = match inverse {
        EditorInverse::RestoreSoundLibrary { .. }
        | EditorInverse::RestoreSampleBank { .. }
        | EditorInverse::RestoreTileEdit { .. }
        | EditorInverse::RestoreSampleOptions { .. }
        | EditorInverse::RestoreSampleAsset { .. }
        | EditorInverse::RestoreSound { .. } => {
            return Err(DomainError::Message(
                "Project-level history belongs to the project dispatcher".into(),
            ));
        }
        EditorInverse::RestoreStackOrder { surface, order } => {
            transaction::restore_stack_order(document, *surface, order)
        }
        EditorInverse::RestoreTempo {
            bpm,
            beats_per_cycle,
        } => set_tempo(document, *bpm, *beats_per_cycle),
        EditorInverse::RestoreAtomValue { node, value } => {
            transaction::set_atom_value(document, node, value.clone())
        }
        EditorInverse::DeleteNode { node } => {
            transaction::delete_node(document, attention, selection, node)
        }
        EditorInverse::RestoreSubtree { patch } => {
            match apply_document_patch(document, patch.clone()) {
                Ok(()) => EditorTransactionResult::accepted(Invalidation::document_replaced()),
                Err(message) => EditorTransactionResult::rejected(message),
            }
        }
        EditorInverse::DisconnectTiles { from, to } => {
            transaction::disconnect_tiles(document, from, to)
        }
        EditorInverse::RestoreConnections {
            bindings,
            relations,
        } => transaction::restore_connections(document, bindings, relations),
    };
    Ok(result)
}

fn set_tempo(document: &mut MusaicDocument, bpm: f64, beats: u32) -> EditorTransactionResult {
    if !bpm.is_finite() || !(1.0..=999.0).contains(&bpm) || !(1..=64).contains(&beats) {
        return EditorTransactionResult::rejected(
            "Tempo must be 1–999 BPM with 1–64 beats per cycle.",
        );
    }
    if document.playback.bpm == bpm && document.playback.beats_per_cycle == beats {
        return EditorTransactionResult::accepted(Invalidation::none());
    }
    let undo = EditorInverse::RestoreTempo {
        bpm: document.playback.bpm,
        beats_per_cycle: document.playback.beats_per_cycle,
    };
    document.playback.bpm = bpm;
    document.playback.beats_per_cycle = beats;
    document.bump_revision();
    EditorTransactionResult::accepted(Invalidation {
        runtime: true,
        save: true,
        ..Invalidation::default()
    })
    .with_undo(undo)
}
