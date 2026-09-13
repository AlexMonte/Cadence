//! Real command/compile/preview scheduling with an in-memory audio renderer.
use super::*;
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        editor::{EditorPlugin, TimelinePanelState, transport::TransportClock},
        history::CommandHistory,
        session::MusaicProject,
    },
    infrastructure::app::{AppState, configure_pipeline_schedule},
};
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins((EditorPlugin, cadence::bevy::CadencePlugin));
    let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(8000, 1024)).unwrap();
    app.insert_non_send_resource(renderer)
        .insert_non_send_resource(PlaybackRuntime::new(PlaybackSettings::default(), audio));
    register_runtime(&mut app);
    crate::application::pipeline::lowering::register_lowering(&mut app);
    configure_pipeline_schedule(&mut app);
    app.insert_state(AppState::Editor);
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project: MusaicProject::demo(),
            path: None,
        },
    );
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .starts()
            .count(),
        12
    );
    app
}

fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    app.update();
}

#[test]
fn browsing_changes_exact_preview_and_sources_without_seeking_or_editing_music() {
    let mut app = app();
    let before =
        crate::adapter::persistence::export_project_bytes(app.world().resource::<MusaicProject>())
            .unwrap();
    let old_ids: Vec<_> = app
        .world()
        .resource::<RuntimePreviewSnapshot>()
        .events()
        .map(|e| e.id)
        .collect();
    for cycle in [1, 7, 0, TimelinePanelState::MAX_PREVIEW_CYCLE] {
        send(&mut app, EditorCommand::PreviewCycle { cycle: Some(cycle) });
        let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
        assert!(snapshot.browsing);
        assert_eq!(
            snapshot.window.unwrap().start(),
            CycleTime::new(i64::from(cycle), 1)
        );
        assert_eq!(snapshot.starts().count(), 12);
        for event in snapshot.starts() {
            assert!(event.visible_start >= snapshot.window.unwrap().start());
            assert!(event.visible_end <= snapshot.window.unwrap().end());
            assert!(event.source_id.is_some());
        }
        assert!(old_ids.iter().all(|id| snapshot.event(*id).is_none()));
        assert_eq!(
            app.world().resource::<TransportClock>().position,
            CycleTime::ZERO
        );
        assert_eq!(
            app.world()
                .non_send_resource::<PlaybackRuntime>()
                .status()
                .cycle_position,
            CycleTime::ZERO
        );
    }
    send(
        &mut app,
        EditorCommand::PreviewCycle {
            cycle: Some(u32::MAX),
        },
    );
    assert_eq!(
        app.world().resource::<TimelinePanelState>().preview_cycle,
        Some(TimelinePanelState::MAX_PREVIEW_CYCLE)
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
    assert_eq!(
        crate::adapter::persistence::export_project_bytes(app.world().resource::<MusaicProject>())
            .unwrap(),
        before
    );
}

#[test]
fn follow_uses_the_live_cycle_and_stop_and_new_project_reset_browsing() {
    let mut app = app();
    send(&mut app, EditorCommand::PreviewCycle { cycle: Some(7) });
    send(
        &mut app,
        EditorCommand::TransportSeek {
            position_cycles: 3.25,
        },
    );
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .window
            .unwrap()
            .start(),
        CycleTime::new(7, 1)
    );
    send(&mut app, EditorCommand::PreviewCycle { cycle: None });
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .window
            .unwrap()
            .start(),
        CycleTime::new(3, 1)
    );
    assert_eq!(
        app.world().resource::<TransportClock>().position,
        CycleTime::new(13, 4)
    );
    for command in [
        EditorCommand::TransportStop,
        EditorCommand::TransportPanic,
        EditorCommand::NewProject,
    ] {
        send(&mut app, EditorCommand::PreviewCycle { cycle: Some(7) });
        send(&mut app, command);
        let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
        assert_eq!(snapshot.window.unwrap().start(), CycleTime::ZERO);
        assert!(!snapshot.browsing);
        assert_eq!(
            app.world().resource::<TimelinePanelState>().preview_cycle,
            None
        );
    }
    // Empty projects still have a usable preview cursor.
    send(&mut app, EditorCommand::PreviewCycle { cycle: Some(9) });
    let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
    assert_eq!(snapshot.window.unwrap().start(), CycleTime::new(9, 1));
    assert_eq!(snapshot.events().count(), 0);
}

#[test]
fn audio_ticks_preserve_event_identity_until_a_cycle_boundary() {
    let mut app = app();
    send(&mut app, EditorCommand::TransportPlay);
    app.update();
    let ids: Vec<_> = app
        .world()
        .resource::<RuntimePreviewSnapshot>()
        .events()
        .map(|e| e.id)
        .collect();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .events()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        ids
    );
    send(&mut app, EditorCommand::PreviewCycle { cycle: Some(7) });
    let ids: Vec<_> = app
        .world()
        .resource::<RuntimePreviewSnapshot>()
        .events()
        .map(|e| e.id)
        .collect();
    // Move the audio runtime across a boundary without issuing a preview command.
    app.world_mut()
        .non_send_resource_mut::<PlaybackRuntime>()
        .seek(CycleTime::new(3, 1))
        .unwrap();
    app.update();
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .events()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        ids
    );
    send(&mut app, EditorCommand::PreviewCycle { cycle: None });
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .window
            .unwrap()
            .start(),
        CycleTime::new(3, 1)
    );
    assert!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .events()
            .all(|e| !ids.contains(&e.id))
    );
}

#[test]
fn browsing_retains_queued_revision_feedback_and_follow_previews_its_boundary() {
    use crate::domain::document::{AtomValue, DocumentNodeKind, NoteName};
    let mut app = app();
    send(&mut app, EditorCommand::TransportPlay);
    app.update();
    send(&mut app, EditorCommand::PreviewCycle { cycle: Some(7) });
    let node = app.world().resource::<MusaicProject>().document.graph.nodes()
        .find(|node| matches!(&node.kind, DocumentNodeKind::Atom(atom) if atom.atom == AtomValue::NoteName(NoteName::C)))
        .unwrap().id.clone();
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node,
            value: AtomValue::NoteName(NoteName::D),
        },
    );
    let pending = app
        .world()
        .non_send_resource::<PlaybackRuntime>()
        .pending_revision_cycle()
        .expect("edit waits for an audio boundary");
    let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
    assert_eq!(snapshot.window.unwrap().start(), CycleTime::new(7, 1));
    assert_eq!(snapshot.pending_cycle, Some(pending));
    assert!(snapshot.starts().any(|event| event.label == "D#4"));
    send(&mut app, EditorCommand::PreviewCycle { cycle: None });
    let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
    assert_eq!(snapshot.window.unwrap().start(), pending);
    assert_eq!(snapshot.pending_cycle, Some(pending));
    assert!(!snapshot.browsing);
}

#[test]
fn audio_boundary_clears_feedback_without_rebuilding_a_pinned_preview() {
    use crate::domain::document::{AtomValue, DocumentNodeKind, NoteName};
    let mut app = app();
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .feedback
            .summary(),
        "Edits accepted · ready to play"
    );
    send(&mut app, EditorCommand::TransportPlay);
    app.update();
    send(&mut app, EditorCommand::PreviewCycle { cycle: Some(7) });
    let node = app.world().resource::<MusaicProject>().document.graph.nodes()
        .find(|node| matches!(&node.kind, DocumentNodeKind::Atom(atom) if atom.atom == AtomValue::NoteName(NoteName::C)))
        .unwrap().id.clone();
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node,
            value: AtomValue::NoteName(NoteName::D),
        },
    );
    let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
    assert!(
        snapshot
            .feedback
            .summary()
            .starts_with("Queued · plays at cycle")
    );
    let ids: Vec<_> = snapshot.events().map(|event| event.id).collect();
    let window = snapshot.window;
    // Advance the actual audio callback, not an editor seek or a simulated DTO.
    for _ in 0..600 {
        let mut frames = [cadence::adapter::audio::Frame::from_mono(0.0); 64];
        app.world_mut()
            .non_send_resource_mut::<AudioRenderer>()
            .render(&mut frames);
        app.update();
        if app
            .world()
            .non_send_resource::<PlaybackRuntime>()
            .pending_revision_cycle()
            .is_none()
        {
            break;
        }
    }
    let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
    assert_eq!(snapshot.feedback.summary(), "Playing current version");
    assert_eq!(snapshot.pending_cycle, None);
    assert_eq!(snapshot.window, window);
    assert_eq!(
        snapshot.events().map(|event| event.id).collect::<Vec<_>>(),
        ids
    );
    send(&mut app, EditorCommand::TransportStop);
    app.update();
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .feedback
            .summary(),
        "Edits accepted · ready to play"
    );
}

#[test]
fn rejected_audio_edit_keeps_valid_music_and_feedback_recovers_on_correction_and_open() {
    use crate::domain::document::{AtomValue, DocumentNodeKind};
    let mut app = app();
    send(&mut app, EditorCommand::TransportPlay);
    app.update();
    let node = app.world().resource::<MusaicProject>().document.graph.nodes()
        .find(|node| matches!(&node.kind, DocumentNodeKind::Atom(atom) if atom.atom == AtomValue::Number(3)))
        .unwrap().id.clone();
    let sources = app
        .world()
        .resource::<RuntimeState>()
        .compiled
        .as_ref()
        .unwrap()
        .source_nodes
        .clone();
    let ir = app
        .world()
        .resource::<RuntimeState>()
        .compiled
        .as_ref()
        .unwrap()
        .tessera_ir
        .clone();
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: node.clone(),
            value: AtomValue::Number(1_000_000),
        },
    );
    let snapshot = app.world().resource::<RuntimePreviewSnapshot>();
    assert_eq!(
        snapshot.feedback.summary(),
        "Edit rejected · previous version is still playing"
    );
    assert!(!snapshot.feedback.detail.is_empty());
    let compiled = app
        .world()
        .resource::<RuntimeState>()
        .compiled
        .as_ref()
        .unwrap();
    assert_eq!(compiled.source_nodes, sources);
    assert_eq!(compiled.tessera_ir, ir);
    assert!(
        app.world()
            .non_send_resource::<PlaybackRuntime>()
            .pending_revision_cycle()
            .is_none()
    );
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: node.clone(),
            value: AtomValue::Number(3),
        },
    );
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .feedback
            .readiness,
        feedback::Readiness::Accepted
    );
    assert!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .feedback
            .detail
            .is_empty()
    );
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node,
            value: AtomValue::Number(1_000_000),
        },
    );
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .feedback
            .readiness,
        feedback::Readiness::Rejected
    );
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project: MusaicProject::demo(),
            path: None,
        },
    );
    app.update();
    assert_eq!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .feedback
            .summary(),
        "Edits accepted · ready to play"
    );
    assert!(
        app.world()
            .resource::<RuntimePreviewSnapshot>()
            .feedback
            .detail
            .is_empty()
    );
}
